//! 跨文件搜索当前缓冲,命中后回到原始文件。
use super::WorldeditApp;
use crate::theme::{self, *};

impl WorldeditApp {
    pub(super) fn project_search(&mut self, ctx: &egui::Context) {
        if !self.search_open {
            return;
        }
        let mut open = true;
        let mut target = None;
        egui::Window::new("搜索整个世界")
            .open(&mut open)
            .collapsible(false)
            .default_width(620.0)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 90.0])
            .show(ctx, |ui| {
                let input = ui.add(
                    egui::TextEdit::singleline(&mut self.project_query)
                        .desired_width(f32::INFINITY)
                        .hint_text("搜索正文、人物或 ID…"),
                );
                if self.search_focus {
                    input.request_focus();
                    self.search_focus = false;
                }
                let results = self.project.search(&self.project_query);
                let objects = self
                    .snapshot
                    .as_ref()
                    .map(|s| {
                        s.result
                            .analysis
                            .catalog
                            .search_objects(&self.project_query)
                    })
                    .unwrap_or_default();
                if !self.project_query.trim().is_empty() {
                    ui.label(theme::muted(format!(
                        "{} 个对象 · 名称、ID 或别名匹配，同名分别列出",
                        objects.len()
                    )));
                    egui::ScrollArea::vertical()
                        .id_salt("object-search")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for object in objects {
                                if ui
                                    .link(format!(
                                        "{} · {} ({})",
                                        super::catalog::kind_label(&object.target.kind),
                                        object.display,
                                        object.target.id
                                    ))
                                    .clicked()
                                {
                                    self.open_reading(object.target);
                                }
                            }
                        });
                }
                ui.label(theme::muted(format!(
                    "{} 个命中行 · 包含未保存的内容 · 区分大小写",
                    results.len()
                )));
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(410.0)
                    .show(ui, |ui| {
                        if results.is_empty() && !self.project_query.is_empty() {
                            ui.label(theme::muted("没有找到匹配内容"));
                        }
                        for hit in results.iter().take(300) {
                            let file = hit
                                .file
                                .strip_prefix(&self.project.root)
                                .unwrap_or(&hit.file);
                            ui.push_id((&hit.file, hit.line), |ui| {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{} : {}",
                                                file.display(),
                                                hit.line
                                            ))
                                            .color(ACCENT),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    target = Some((
                                        hit.file.to_string_lossy().into_owned(),
                                        hit.line,
                                        hit.column,
                                    ));
                                }
                                ui.label(
                                    egui::RichText::new(crate::visual::truncated(
                                        hit.preview.trim(),
                                        120,
                                    ))
                                    .size(13.0),
                                );
                                ui.add_space(6.0);
                            });
                        }
                        if results.len() > 300 {
                            ui.label(theme::muted("显示前 300 行,请缩小搜索范围"));
                        }
                    });
            });
        self.search_open = open;
        if let Some((file, line, column)) = target {
            self.jump_to_file(&file, line, column);
            self.search_open = false;
        }
    }
}
