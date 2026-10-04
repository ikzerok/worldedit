use super::*;
use worldline_core::catalog::CatalogObject;
impl WorldeditApp {
    pub(super) fn search_objects_in_current_drafts(&self) -> (Vec<CatalogObject>, Option<String>) {
        match self
            .project
            .compile_writing_drafts(&self.manuscript.writing_buffers())
        {
            Ok(result) => (
                result.analysis.catalog.search_objects(&self.project_query),
                None,
            ),
            Err(error) => (
                self.snapshot
                    .as_ref()
                    .map(|snapshot| {
                        snapshot
                            .result
                            .analysis
                            .catalog
                            .search_objects(&self.project_query)
                    })
                    .unwrap_or_default(),
                Some(format!(
                    "目录来自已应用版本；当前草稿暂不可解析：{error}。下方源码命中仍包含草稿。"
                )),
            ),
        }
    }
    pub(super) fn navigate_search_object(
        &mut self,
        ctx: &egui::Context,
        object: &CatalogObject,
        applied_catalog: bool,
    ) {
        if applied_catalog {
            self.open_reading(object.target.clone());
            return;
        }
        let current = self
            .project
            .compile_writing_drafts(&self.manuscript.writing_buffers());
        let valid = current
            .as_ref()
            .ok()
            .and_then(|result| result.analysis.catalog.object(&object.target))
            .is_some_and(|current| current.file == object.file && current.line == object.line);
        if !valid {
            self.search_state.error = Some("对象定位依据已变化，请重新查找；当前稿已保留".into());
            return;
        }
        let path = PathBuf::from(&object.file);
        let path = if path.is_absolute() {
            path
        } else {
            self.project.root.join(path)
        };
        if let Some(buffer) = self
            .manuscript
            .writing_buffers()
            .into_iter()
            .find(|b| b.path() == path && b.is_changed())
        {
            let start = buffer
                .source()
                .split_inclusive('\n')
                .take(object.line.saturating_sub(1) as usize)
                .map(str::len)
                .sum();
            self.go_search_position(
                ctx,
                &SearchMatch {
                    path,
                    range: start..start,
                    line: object.line,
                    column: 1,
                    preview: String::new(),
                    context: None,
                    identity: None,
                    replaceable: false,
                    draft: true,
                },
            );
        } else {
            self.open_reading(object.target.clone());
        }
    }
}
