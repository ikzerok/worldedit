//! 作者Back与来源编辑器共享真实文档身份；不对有损预览建立文本坐标。
use super::WorldeditApp;
use std::path::Path;

impl WorldeditApp {
    /// `.wl`和有效UTF-8展示文档都使用原文；未知格式仍保持core的只读保护。
    pub(in crate::app) fn source_position_document(&self, path: &Path) -> Option<(egui::Id, &str)> {
        if let Ok(text) = self.project.document(path) {
            return Some((egui::Id::new(("source", path)), text));
        }
        let document = self.project.authoring_document(path).ok()?;
        if document.is_deleted() {
            return None;
        }
        let text = std::str::from_utf8(document.bytes()).ok()?;
        Some((egui::Id::new(("authoring-source", path)), text))
    }
}
