use super::super::super::WorldeditApp;
use crate::theme;
use std::path::Path;

impl WorldeditApp {
    pub(super) fn readonly_source_document(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        path: &Path,
        relative: &str,
    ) {
        let Ok(document) = self.project.authoring_document(path) else {
            return;
        };
        let raw = document.bytes().to_vec();
        let read_only = document.is_read_only();
        let mut text = match String::from_utf8(raw.clone()) {
            Ok(text) => text,
            Err(_) => {
                self.page_heading(ui, relative, "展示文档原始字节（只读预览）");
                ui.label(theme::muted(
                                "原始文档不是有效 UTF-8，本视图只读，原字节保持不变；请通过外部编辑器或另存入口修复。",
                            ));
                let mut preview = String::from_utf8_lossy(&raw).into_owned();
                ui.add(
                    egui::TextEdit::multiline(&mut preview)
                        .code_editor()
                        .font(egui::FontId::monospace(14.0))
                        .desired_width(ui.available_width().max(500.0))
                        .desired_rows(36)
                        .interactive(false),
                );
                return;
            }
        };
        let id = egui::Id::new(("authoring-source", &path));
        let target = self.jump.take();
        let mut changed = false;
        self.page_heading(
            ui,
            relative,
            "展示文档原文 · 可修复损坏 JSON；Ctrl+S 保存全部文件",
        );
        ui.label(theme::muted(
            "地图和其他展示文档由 core 保留原始字节；结构化命令仍从地图画布提交。",
        ));
        ui.add_enabled_ui(!read_only, |ui| {
            let mut output = egui::TextEdit::multiline(&mut text)
                .id(id)
                .code_editor()
                .font(egui::FontId::monospace(14.0))
                .desired_width(ui.available_width().max(500.0))
                .desired_rows(36)
                .show(ui);
            changed = output.response.changed();
            if let Some((line, column)) = target {
                let offset = text
                    .split_inclusive('\n')
                    .take(line.saturating_sub(1) as usize)
                    .map(|line| line.chars().count())
                    .sum::<usize>()
                    + column.saturating_sub(1) as usize;
                let cursor = egui::text::CCursor::new(offset.min(text.chars().count()));
                output
                    .state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(cursor)));
                output.state.store(ctx, id);
                output.response.request_focus();
                let rect = output
                    .galley
                    .pos_from_cursor(cursor)
                    .translate(output.galley_pos.to_vec2());
                ui.scroll_to_rect(rect, Some(egui::Align::Center));
            }
        });
        if read_only {
            ui.label(theme::muted("此展示文档格式或能力未知，只读查看。"));
        }
        if changed {
            let before = self.project.clone();
            match self.project.set_authoring_document(path, text.into_bytes()) {
                Ok(()) => {
                    self.remember(before);
                    self.recompile();
                }
                Err(error) => self.io_error = Some(error),
            }
        }
    }
}
