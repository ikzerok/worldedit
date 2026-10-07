//! 当前稿目录只按来源代次编译；文字与过滤分页交由 core。
use super::*;
use crate::app::object_picker::CandidatePage;
use worldline_core::catalog::{Catalog, CatalogObject};
use worldline_core::object_search::{ObjectSearchFilter, ObjectSearchPage};

#[derive(Clone, Debug, PartialEq, Eq)]
struct CatalogBasis {
    root: PathBuf,
    version: u64,
    applied_available: bool,
    baseline: String,
    drafts: Vec<(PathBuf, String, u64, String)>,
}
#[derive(Default)]
pub(super) struct ObjectCatalogCache {
    basis: Option<CatalogBasis>,
    catalog: Option<Catalog>,
    warning: Option<String>,
    applied: bool,
    #[cfg(test)]
    pub compilations: usize,
}
pub(in crate::app) struct ObjectPageView {
    pub page: Result<ObjectSearchPage, String>,
    pub warning: Option<String>,
    pub applied: bool,
    pub stale: bool,
}

impl WorldeditApp {
    fn refresh_object_catalog(&mut self) {
        let buffers = self.manuscript.writing_buffers();
        let mut drafts: Vec<_> = buffers
            .iter()
            .map(|buffer| {
                (
                    buffer.path().to_path_buf(),
                    buffer.baseline().to_owned(),
                    buffer.generation(),
                    crate::app::writing_workspace::fingerprint(buffer.source()),
                )
            })
            .collect();
        drafts.sort();
        let basis = CatalogBasis {
            root: self.project.root.clone(),
            version: self.version,
            applied_available: self.snapshot.is_some(),
            baseline: self.project.content_baseline(),
            drafts,
        };
        if self.search_state.object_catalog.basis.as_ref() == Some(&basis) {
            return;
        }
        let result = self.project.compile_writing_drafts(&buffers);
        let cache = &mut self.search_state.object_catalog;
        cache.basis = Some(basis);
        #[cfg(test)]
        {
            cache.compilations += 1;
        }
        match result {
            Ok(result) => {
                cache.catalog = Some(result.analysis.catalog);
                cache.warning = None;
                cache.applied = false;
            }
            Err(error) => {
                cache.applied = true;
                cache.catalog = self
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.result.analysis.catalog.clone());
                cache.warning = Some(format!(
                    "已应用目录（已应用版本）；当前草稿暂不可解析：{error}。源码查找仍包含草稿。{}",
                    if self
                        .snapshot
                        .as_ref()
                        .is_some_and(|snapshot| snapshot.result.has_errors())
                    {
                        " 此已应用目录也含解析错误，总数仅代表已解析对象。"
                    } else {
                        ""
                    }
                ));
            }
        }
    }

    pub(in crate::app) fn current_object_page(
        &mut self,
        page: &mut CandidatePage,
        query: &str,
        filter: &ObjectSearchFilter,
    ) -> ObjectPageView {
        self.refresh_object_catalog();
        let cache = &self.search_state.object_catalog;
        let revision = format!("{:?}", cache.basis);
        let stale = cache
            .catalog
            .as_ref()
            .is_some_and(|catalog| page.refresh(catalog, query, filter, &revision));
        let result = if cache.catalog.is_some() {
            page.result
                .clone()
                .unwrap_or_else(|| Err("对象目录尚未就绪".into()))
        } else {
            page.result = None;
            Err("对象目录尚未就绪；未复用旧候选".into())
        };
        ObjectPageView {
            page: result,
            warning: cache.warning.clone(),
            applied: cache.applied,
            stale,
        }
    }

    pub(super) fn search_objects_in_current_drafts(&mut self) -> ObjectPageView {
        let mut page = std::mem::take(&mut self.search_state.object_page);
        let query = self.project_query.clone();
        let view = self.current_object_page(
            &mut page,
            &query,
            &crate::app::object_picker::filter(&[], None),
        );
        self.search_state.object_page = page;
        view
    }

    pub(in crate::app) fn navigate_object_candidate(
        &mut self,
        ctx: &egui::Context,
        object: &CatalogObject,
        applied: bool,
        inspect: bool,
    ) -> bool {
        let result = self.guard_and_navigate_object(ctx, object, applied, inspect);
        if let Err(error) = result {
            self.search_state.error = Some(error.clone());
            self.message = Some(error);
            false
        } else {
            true
        }
    }

    fn guard_and_navigate_object(
        &mut self,
        ctx: &egui::Context,
        object: &CatalogObject,
        applied: bool,
        inspect: bool,
    ) -> Result<(), String> {
        if self.ime_composing || self.command_palette.ime || self.command_palette.ime_frame {
            return Err("输入法组合尚未完成，未离开当前位置".into());
        }
        let current = if applied {
            Some(
                self.project
                    .compile_object_search_snapshot()
                    .analysis
                    .catalog,
            )
        } else {
            Some(
                self.project
                    .compile_writing_drafts(&self.manuscript.writing_buffers())?
                    .analysis
                    .catalog,
            )
        };
        let valid = current
            .as_ref()
            .and_then(|catalog| catalog.object(&object.target))
            .is_some_and(|current| current.file == object.file && current.line == object.line);
        if !valid {
            return Err("对象定位依据已变化，请重新查找；当前稿已保留".into());
        }
        let path = PathBuf::from(&object.file);
        let path = if path.is_absolute() {
            path
        } else {
            self.project.root.join(path)
        };
        self.project
            .verify_source_navigation(&path, self.project.document(&path)?)?;
        if applied {
            self.open_reading(object.target.clone());
            self.message = Some("正在旁查已应用目录；未使用其行号跳入当前草稿".into());
        } else if let Some(buffer) = self
            .manuscript
            .writing_buffers()
            .into_iter()
            .find(|buffer| buffer.path() == path && buffer.is_changed())
        {
            let preview = self.project.preview_source_jump(
                &path,
                buffer.source(),
                &format!("{}:1", object.line),
            )?;
            let range = self
                .project
                .resolve_source_jump(&preview, buffer.source())?;
            self.go_author_source_position(
                ctx,
                &SearchMatch {
                    path,
                    range,
                    line: object.line,
                    column: 1,
                    preview: String::new(),
                    context: None,
                    identity: None,
                    replaceable: false,
                    draft: true,
                },
                true,
            )?;
        } else if inspect {
            self.navigate_object(object);
        } else {
            self.open_reading(object.target.clone());
        }
        self.search_state.error = None;
        Ok(())
    }

    pub(super) fn navigate_search_object(
        &mut self,
        ctx: &egui::Context,
        object: &CatalogObject,
        applied: bool,
    ) {
        self.navigate_object_candidate(ctx, object, applied, false);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
