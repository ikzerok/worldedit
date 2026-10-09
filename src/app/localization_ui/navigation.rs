//! Read-only provenance checks for displayed core results; source ranges remain core-owned.
use super::{jobs::AcceptedKind, plans::Preview, LocalizationUiState};
use crate::app::{Tab, WorldeditApp};
use worldline_core::localization::{
    localization_source_hit, LocalizationCatalogEntry, LocalizationCatalogPage,
    LocalizationDiagnostic, LocalizationExchangeEntry, LocalizationExportPlan, LocalizationIdPlan,
    LocalizationImportPlan, LocalizationSource,
};

#[derive(Clone, Copy)]
pub(super) enum Container<'a> {
    Catalog(&'a LocalizationCatalogPage),
    Export(&'a LocalizationExportPlan),
    Import(&'a LocalizationImportPlan),
    Edits(&'a LocalizationImportPlan),
    Id(&'a LocalizationIdPlan),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Owner {
    Catalog,
    Export,
    Import,
    Edits,
    Id,
}
#[derive(Clone, PartialEq, Eq)]
struct Receipt {
    owner: Owner,
    baseline: String,
    digest: String,
    locale: Option<String>,
}
#[derive(Clone)]
enum Evidence {
    Catalog(Box<LocalizationCatalogEntry>),
    Exchange(Box<LocalizationExchangeEntry>),
    Diagnostic(Box<LocalizationDiagnostic>),
}
#[derive(Clone)]
pub(in crate::app) struct Request {
    pub(in crate::app) source: LocalizationSource,
    receipt: Receipt,
    evidence: Evidence,
}

impl Container<'_> {
    fn receipt(self) -> Receipt {
        let (owner, baseline, digest, locale) = match self {
            Self::Catalog(page) => (
                Owner::Catalog,
                &page.content_baseline,
                &page.source_baseline,
                page.target_locale.clone(),
            ),
            Self::Export(plan) => (
                Owner::Export,
                &plan.content_baseline,
                &plan.plan_digest,
                None,
            ),
            Self::Import(plan) => (
                Owner::Import,
                &plan.content_baseline,
                &plan.plan_digest,
                None,
            ),
            Self::Edits(plan) => (
                Owner::Edits,
                &plan.content_baseline,
                &plan.plan_digest,
                None,
            ),
            Self::Id(plan) => (Owner::Id, &plan.content_baseline, &plan.plan_digest, None),
        };
        Receipt {
            owner,
            baseline: baseline.clone(),
            digest: digest.clone(),
            locale,
        }
    }
}

impl Request {
    pub(super) fn catalog_entry(
        page: &LocalizationCatalogPage,
        entry: &LocalizationCatalogEntry,
    ) -> Option<Self> {
        Some(Self {
            source: entry.source.clone()?,
            receipt: Container::Catalog(page).receipt(),
            evidence: Evidence::Catalog(Box::new(entry.clone())),
        })
    }
    pub(super) fn export_entry(
        plan: &LocalizationExportPlan,
        entry: &LocalizationExchangeEntry,
    ) -> Self {
        Self {
            source: entry.source.clone(),
            receipt: Container::Export(plan).receipt(),
            evidence: Evidence::Exchange(Box::new(entry.clone())),
        }
    }
    pub(super) fn import_entry(
        plan: &LocalizationImportPlan,
        entry: &LocalizationExchangeEntry,
    ) -> Self {
        Self {
            source: entry.source.clone(),
            receipt: Container::Import(plan).receipt(),
            evidence: Evidence::Exchange(Box::new(entry.clone())),
        }
    }
    pub(super) fn diagnostic(
        container: Container<'_>,
        diagnostic: &LocalizationDiagnostic,
    ) -> Option<Self> {
        Some(Self {
            source: diagnostic.source.clone()?,
            receipt: container.receipt(),
            evidence: Evidence::Diagnostic(Box::new(diagnostic.clone())),
        })
    }

    fn verify(
        &self,
        state: &LocalizationUiState,
        root: &std::path::Path,
        version: u64,
    ) -> Result<(), String> {
        let (kind, container) = match self.receipt.owner {
            Owner::Catalog => (
                AcceptedKind::Catalog,
                state.workbench.page.as_ref().map(Container::Catalog),
            ),
            Owner::Export => (
                AcceptedKind::Export,
                state.export_plan.as_ref().map(Container::Export),
            ),
            Owner::Import => (
                AcceptedKind::Import,
                state.import_plan.as_ref().map(Container::Import),
            ),
            Owner::Edits => (
                AcceptedKind::Preview,
                match &state.workbench.preview {
                    Some(Preview::Edits { plan, .. }) => Some(Container::Edits(plan)),
                    _ => None,
                },
            ),
            Owner::Id => (
                AcceptedKind::Preview,
                match &state.workbench.preview {
                    Some(Preview::Id { plan, .. }) => Some(Container::Id(plan)),
                    _ => None,
                },
            ),
        };
        if !state.jobs.accepted_matches(kind, root, version) {
            return Err(
                "此来源没有当前工作区与已应用版本的核对结果；请重新预览，输入已保留".into(),
            );
        }
        let container = container.ok_or("来源所属预览已关闭或替换；请重新打开，输入已保留")?;
        if self.receipt != container.receipt() {
            return Err("来源所属目录或计划已更新；请点击当前结果，输入已保留".into());
        }
        let valid = match (&self.evidence, container) {
            (Evidence::Catalog(entry), Container::Catalog(page)) => {
                entry.source.as_ref() == Some(&self.source) && page.entries.contains(entry)
            }
            (Evidence::Exchange(entry), Container::Export(plan)) => {
                entry.source == self.source && plan.exchange.entries.contains(entry)
            }
            (Evidence::Exchange(entry), Container::Import(plan)) => {
                // A package's source fields are untrusted until core validates this selected entry.
                // Never authenticate them by finding an unrelated real unit at the same line.
                if !plan.can_apply || !plan.affected_ids.contains(&entry.id) {
                    return Err(
                        "交换包此项来源尚未通过核心核对；请使用核心诊断的定位链接，原输入已保留"
                            .into(),
                    );
                }
                entry.source == self.source
                    && state
                        .import_exchange
                        .as_ref()
                        .is_some_and(|exchange| exchange.entries.contains(entry))
            }
            (Evidence::Diagnostic(diagnostic), container) => {
                let diagnostics = match container {
                    Container::Catalog(page) => &page.diagnostics,
                    Container::Export(plan) => &plan.diagnostics,
                    Container::Import(plan) | Container::Edits(plan) => &plan.diagnostics,
                    Container::Id(plan) => &plan.diagnostics,
                };
                diagnostic.source.as_ref() == Some(&self.source) && diagnostics.contains(diagnostic)
            }
            _ => false,
        };
        if !valid {
            return Err("此来源不属于当前显示的精确条目或核心诊断；当前位置与输入已保留".into());
        }
        // Unlike import diagnostics, ID diagnostics can echo an invalid assignment's source.
        // Authenticate that assignment against the accepted current core catalog, not a line match.
        if self.receipt.owner == Owner::Id {
            let Some(Preview::Id { draft, .. }) = &state.workbench.preview else {
                return Err("稳定 ID 预览已经关闭；原输入已保留".into());
            };
            let current = state
                .jobs
                .accepted_matches(AcceptedKind::Catalog, root, version)
                && state.workbench.page.as_ref().is_some_and(|page| {
                    page.content_baseline == self.receipt.baseline
                        && draft.assignments.iter().any(|assignment| {
                            assignment.source == self.source
                                && page.entries.iter().any(|entry| {
                                    entry.source.as_ref() == Some(&assignment.source)
                                        && entry.id == assignment.expected_id
                                        && entry.source_revision.as_ref()
                                            == Some(&assignment.source_revision)
                                })
                        })
                });
            if !current {
                return Err(
                    "稳定 ID 草稿来源尚未匹配当前目录的精确身份；请重新核对，原输入已保留".into(),
                );
            }
        }
        Ok(())
    }
}

impl WorldeditApp {
    pub(in crate::app) fn open_localization_source(
        &mut self,
        ctx: &egui::Context,
        request: Request,
    ) {
        let result = self
            .localization_source_hit(&request)
            .and_then(|hit| self.go_author_source_position(ctx, &hit, true));
        match result {
            Ok(()) => {
                self.message =
                    Some("已定位源文的完整语句或选择头；Alt+Left 返回本地化工作台".into());
            }
            Err(error) => {
                self.localization_ui.status = Some(Err(error.clone()));
                self.message = Some(error);
            }
        }
    }

    fn localization_source_hit(
        &self,
        request: &Request,
    ) -> Result<worldline_core::search_replace::SearchMatch, String> {
        if self.tab != Tab::Localization {
            return Err("本地化来源已离开当前视图；当前位置与输入已保留".into());
        }
        request.verify(&self.localization_ui, &self.project.root, self.version)?;
        if request.receipt.baseline != self.project.content_baseline() {
            return Err("来源核对基线已过期；请重新预览，原输入已保留".into());
        }
        let snapshot = self.snapshot.as_ref().ok_or("当前没有可确认的编译来源")?;
        if snapshot.result.has_errors()
            || snapshot.result.options != self.project.compile_options()
            || snapshot.result.sources != self.project.sources()
        {
            return Err("当前编译来源与已应用稿不一致；请修正或重新编译后定位".into());
        }
        if self.ime_composing || self.command_palette.ime || self.command_palette.ime_frame {
            return Err("请先完成输入法组合；当前位置与输入已保留".into());
        }
        if !self.unapplied_play_inputs().is_empty() {
            return Err("存在未应用执行草稿；请先应用或撤销这些输入，再定位源文".into());
        }
        self.project.verify_review_navigation()?;
        let hit =
            localization_source_hit(&snapshot.result, &self.project.root, &request.source, false)?;
        self.project
            .verify_source_navigation(&hit.path, self.project.document(&hit.path)?)?;
        Ok(hit)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
