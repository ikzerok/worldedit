//! 次级技术信息仍可辨别、查阅和复制，绝不按 basename 合并来源。
use super::muted;
use std::path::Path;

pub fn relative_source(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

pub fn technical_value(ui: &mut egui::Ui, label: &str, value: &str) {
    let response = ui.add(egui::Label::new(muted(format!("{label}：{value}"))).wrap());
    copy_menu(response, value);
}

pub fn source_path(ui: &mut egui::Ui, root: &Path, path: &Path) {
    let full = path.display().to_string();
    ui.horizontal_wrapped(|ui| {
        let response = ui
            .add(egui::Label::new(muted(relative_source(root, path))).wrap())
            .on_hover_text(&full);
        copy_menu(response, &full);
        if ui.small_button("复制来源").clicked() {
            ui.ctx().copy_text(full.clone());
        }
    });
}

/// 保持稿件首屏高度；完整值在 hover 与右键复制中可达。
pub fn source_caption(ui: &mut egui::Ui, root: &Path, path: &Path) {
    let full = path.display().to_string();
    let response = ui
        .add(egui::Label::new(muted(relative_source(root, path))).truncate())
        .on_hover_text(&full);
    copy_menu(response, &full);
}

fn copy_menu(response: egui::Response, value: &str) {
    response.context_menu(|ui| {
        if ui.button("复制完整值").clicked() {
            ui.ctx().copy_text(value.to_owned());
            ui.close();
        }
    });
}
