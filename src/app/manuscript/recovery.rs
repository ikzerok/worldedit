//! 编排撤销后仍可查看和复制唯一文件草稿，不自动刷新其基线。
use crate::theme;

impl super::super::WorldeditApp {
    pub(super) fn manuscript_orphaned_drafts(&mut self, ui: &mut egui::Ui) {
        if self.manuscript.writing_view.has_retained_input() {
            ui.separator();
            let appearance = self.personal.appearance();
            self.manuscript.writing_view.draw_orphaned_compositions(
                ui,
                &self.manuscript.writing_buffers,
                super::super::writing_workspace::Typography {
                    compact: true,
                    size: appearance.body_size,
                    spacing: appearance.line_spacing,
                    width: appearance.reading_width,
                    source_size: appearance.source_size,
                },
            );
            self.manuscript.writing_view.draw_retained_input(ui);
        }
        let count = self
            .manuscript
            .writing_buffers
            .values()
            .filter(|buffer| buffer.is_changed())
            .count();
        if count == 0 {
            return;
        }
        ui.add_space(theme::SPACE_XL);
        theme::panel_header(
            ui,
            "保留的正文草稿",
            "部分编排或来源无法在当前入口确认，未应用文字仍保留；先复制核对，再继续创作",
        );
        let mut discard = None;
        for (path, buffer) in self
            .manuscript
            .writing_buffers
            .iter()
            .filter(|(_, buffer)| buffer.is_changed())
        {
            egui::CollapsingHeader::new(theme::relative_source(&self.project.root, path))
                .id_salt(("orphaned-writing-buffer", path))
                .show(ui, |ui| {
                    let mut source = buffer.source().to_owned();
                    egui::ScrollArea::vertical()
                        .id_salt(("orphaned-writing-source", path))
                        .max_height(260.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut source)
                                    .font(theme::source_font(
                                        self.personal.appearance().source_size,
                                    ))
                                    .interactive(false)
                                    .desired_width(f32::INFINITY),
                            );
                        });
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("复制完整正文草稿").clicked() {
                            ui.ctx().copy_text(source);
                        }
                        if ui.button("丢弃这份保留草稿").clicked() {
                            self.manuscript.orphan_discard_confirm = Some(path.clone());
                        }
                    });
                    if self.manuscript.orphan_discard_confirm.as_ref() == Some(path) {
                        ui.label("将丢弃这份未应用输入，当前工程文件保持原样。");
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("确认丢弃保留草稿").clicked() {
                                discard = Some(path.clone());
                            }
                            if ui.button("继续保留草稿").clicked() {
                                self.manuscript.orphan_discard_confirm = None;
                            }
                        });
                    }
                });
        }
        if let Some(path) = discard {
            self.discard_manuscript_body(&path);
            self.manuscript.orphan_discard_confirm = None;
        }
    }
}
