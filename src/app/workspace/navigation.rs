use super::super::WorldeditApp;
use crate::app::Tab;
use std::path::{Path, PathBuf};
use worldline_core::project::Project;

pub(in crate::app) fn workspace_source_path(project: &Project, path: &Path) -> PathBuf {
    let web_rooted = path.starts_with(&project.root)
        && matches!(
            path.components().next(),
            Some(std::path::Component::RootDir)
        )
        && matches!(
            project.root.components().next(),
            Some(std::path::Component::RootDir)
        );
    if path.is_absolute() || web_rooted {
        return path.to_path_buf();
    }
    let path = project.root.join(path);
    // Browser-mounted source paths have no host filesystem path to canonicalize.
    std::path::absolute(&path).unwrap_or(path)
}
impl WorldeditApp {
    pub(in crate::app) fn jump_to_file(&mut self, file: &str, line: u32, column: u32) {
        let path = PathBuf::from(file);
        let known_source = self.project.documents.contains_key(&path)
            || self
                .project
                .authoring_document(&path)
                .is_ok_and(|document| !document.is_deleted());
        if known_source {
            self.active_file = path;
            self.tab = Tab::Edit;
            self.jump = Some((line, column));
        }
    }
}
