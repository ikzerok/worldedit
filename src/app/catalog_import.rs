//! CSV 快照的作者工作台；解析、差异、能力与整批事务只消费 core DTO。
mod input;
mod job;
mod mapping;
mod review;
mod view;

use super::{Tab, WorldeditApp};
use std::path::PathBuf;
use worldline_core::catalog_import::{
    CatalogColumnMapping, CatalogCsvTable, CatalogImportPlan, CatalogImportRequest,
};

#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum Step {
    #[default]
    Mapping,
    Review,
}

#[derive(Default)]
pub(super) struct ImportState {
    pub(super) source_name: String,
    csv: String,
    table: Option<CatalogCsvTable>,
    columns: Vec<Option<CatalogColumnMapping>>,
    destination: PathBuf,
    request: Option<CatalogImportRequest>,
    plan: Option<CatalogImportPlan>,
    job: Option<job::ImportJob>,
    generation: u64,
    step: Step,
    selected_row: usize,
    acknowledged: bool,
    submitted: bool,
    applied_baseline: Option<String>,
    stale: bool,
    discard_confirm: bool,
    replacement: Option<(String, String)>,
    error: Option<String>,
    status: Option<String>,
}

impl ImportState {
    pub(super) fn has_unsubmitted_work(&self) -> bool {
        (!self.source_name.is_empty() || self.table.is_some()) && !self.submitted
            || self.replacement.is_some()
    }

    pub(super) fn input_signature(&self) -> String {
        serde_json::json!([self.csv, self.columns, self.destination, self.generation]).to_string()
    }

    pub(super) fn discard(&mut self) {
        // Native tasks retain their single slot until they exit; never fan out canceled compiles.
        let mut job = self.job.take();
        if let Some(job) = &mut job {
            job.cancel();
        }
        let generation = self.generation.wrapping_add(1);
        *self = Self {
            job,
            generation,
            ..Default::default()
        };
    }

    fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.acknowledged = false;
        self.submitted = false;
        self.stale = self.plan.is_some();
        self.error = None;
        if let Some(job) = &mut self.job {
            job.cancel();
        }
    }

    pub(super) fn set_failure(&mut self, error: String) {
        self.acknowledged = false;
        self.error = Some(error);
    }

    fn refresh_baseline(&mut self, app: &WorldeditApp) {
        if self.submitted {
            if self
                .applied_baseline
                .as_ref()
                .is_some_and(|baseline| baseline != &app.project.content_baseline())
            {
                self.stale = true;
                self.acknowledged = false;
                self.status = Some("工程已变化；保留的批次结果仅供参考，请刷新预览。".into());
            }
            return;
        }
        if self
            .request
            .as_ref()
            .is_some_and(|request| request.expected_baseline != app.project.content_baseline())
        {
            self.stale = true;
            self.acknowledged = false;
        }
    }

    fn can_apply(&self, app: &WorldeditApp) -> bool {
        self.job.is_none()
            && self.replacement.is_none()
            && !self.discard_confirm
            && !self.stale
            && !self.submitted
            && self.acknowledged
            && app.catalog_import_blockers().is_empty()
            && self.plan.as_ref().is_some_and(|plan| {
                plan.can_apply && !plan.changed_files.is_empty() && plan.error_count == 0
            })
    }

    fn preview(&mut self, app: &WorldeditApp, ctx: &egui::Context) {
        if self.job.is_some() {
            return;
        }
        let blockers = app.catalog_import_blockers();
        if !blockers.is_empty() {
            self.error = Some(format!(
                "请先处理未应用输入：{}。输入已保留。",
                blockers.join("、")
            ));
            return;
        }
        if self.table.is_none() {
            self.start_parse(ctx);
            return;
        }
        let request = CatalogImportRequest {
            schema_version: 1,
            expected_baseline: app.project.content_baseline(),
            csv: self.csv.clone(),
            destination: self.destination.clone(),
            // Omitted mappings intentionally reach core as missing columns, never inferred Ignore.
            columns: self.columns.iter().flatten().cloned().collect(),
        };
        self.acknowledged = false;
        self.stale = true;
        self.error = None;
        self.submitted = false;
        match job::ImportJob::preview(&app.project, request.clone(), self.generation, ctx) {
            Ok(job) => {
                self.request = Some(request);
                self.job = Some(job);
                self.status = Some("正在后台检查整批资料，可取消；尚未修改工程。".into());
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn apply(&mut self, app: &mut WorldeditApp) {
        self.refresh_baseline(app);
        if !self.can_apply(app) {
            self.error = Some("当前预览不可应用；请核对草稿、错误、基线与整批确认。".into());
            return;
        }
        let (Some(request), Some(plan)) = (&self.request, &self.plan) else {
            return;
        };
        let before = app.project.clone();
        match app.project.apply_catalog_import(request, &plan.plan_digest) {
            Ok(result) => {
                if !result.changed_files.is_empty() {
                    // The apply guard proved these open forms have no unsubmitted edits.
                    let character = app
                        .character_editor
                        .as_ref()
                        .and_then(|form| form.original.clone());
                    let entity = app
                        .entity_editor
                        .as_ref()
                        .and_then(|form| form.original.clone());
                    app.remember(before);
                    app.recompile();
                    if let Some(id) = character {
                        app.character_editor = None;
                        app.select_character(&id);
                    }
                    if let Some(id) = entity {
                        app.entity_editor = None;
                        app.edit_entity(Some(&id));
                    }
                    app.manuscript.rebase_clean(&app.project);
                }
                self.applied_baseline = Some(result.new_baseline);
                self.plan = Some(result.plan);
                self.acknowledged = false;
                self.submitted = true;
                self.stale = false;
                self.error = None;
                self.status =
                    Some("整批已应用到内存；可一次撤销，或保存全部。原 CSV 未改变。".into());
                app.message = self.status.clone();
                app.io_error = None;
            }
            Err(error) => {
                self.stale = true;
                self.acknowledged = false;
                self.error = Some(format!("整批未应用，工程未改变：{error}"));
            }
        }
    }
}

impl WorldeditApp {
    pub(super) fn open_catalog_import(&mut self) {
        self.remember_author_position();
        self.tab = Tab::CatalogImport;
    }

    fn catalog_import_blockers(&self) -> Vec<&'static str> {
        let mut blockers: Vec<_> = self
            .dirty_draft_names()
            .into_iter()
            .chain(self.frame_dirty_drafts.iter().copied())
            .filter(|kind| *kind != "世界资料导入")
            .collect();
        if self.command_palette.ime || self.command_palette.ime_frame {
            blockers.push("输入法组合");
        }
        blockers.sort_unstable();
        blockers.dedup();
        blockers
    }

    pub(super) fn catalog_import_tab(&mut self, ctx: &egui::Context) {
        let mut state = std::mem::take(&mut self.catalog_import);
        state.poll(self, ctx);
        // A successfully applied preview remains navigable but never silently reapplies.
        state.refresh_baseline(self);
        state.render(self, ctx);
        self.catalog_import = state;
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
