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
        // JSON文档复用作者位置与滚动，不沿用只适用于.wl galley的软换行锚点。
        self.personal.source_view = None;
        let Ok(document) = self.project.authoring_document(path) else {
            return;
        };
        let raw = document.bytes().to_vec();
        let read_only = document.is_read_only();
        let editor_width = ui.available_width().max(500.);
        let mut text = match String::from_utf8(raw.clone()) {
            Ok(text) => text,
            Err(_) => {
                self.personal.restore_source = false;
                self.personal.source_scroll = [0., 0.];
                self.page_heading(ui, relative, "展示文档原始字节（只读预览）");
                ui.label(theme::muted("原始文档不是有效 UTF-8，本视图只读，原字节保持不变；请通过外部编辑器或另存入口修复。"));
                let mut preview = String::from_utf8_lossy(&raw).into_owned();
                egui::ScrollArea::both()
                    .id_salt(("authoring-bytes-scroll", path))
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut preview)
                                .code_editor()
                                .font(egui::FontId::monospace(14.))
                                .desired_width(editor_width)
                                .desired_rows(36)
                                .interactive(false),
                        );
                    });
                return;
            }
        };
        let id = egui::Id::new(("authoring-source", path));
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
        if read_only {
            ui.label(theme::muted("此展示文档格式或能力未知，只读查看。"));
        }
        let salt = egui::Id::new(("authoring-source-scroll", path));
        let mut scroll = egui::ScrollArea::both()
            .id_salt(salt)
            .auto_shrink([false, false]);
        if self.personal.restore_source && target.is_none() {
            let offset = egui::vec2(
                self.personal.source_scroll[0],
                self.personal.source_scroll[1],
            );
            let mut state = egui::scroll_area::State::default();
            state.offset = offset;
            state.store(ctx, ui.make_persistent_id(salt));
            scroll = scroll.scroll_offset(offset).animated(false);
        }
        self.personal.restore_source = false;
        let output = scroll.show(ui, |ui| {
            ui.add_enabled_ui(!read_only, |ui| {
                let mut output = egui::TextEdit::multiline(&mut text)
                    .id(id)
                    .code_editor()
                    .font(egui::FontId::monospace(14.))
                    .desired_width(editor_width)
                    .desired_rows(36)
                    .show(ui);
                changed = output.response.changed();
                if let Some((line, column)) = target {
                    // 旧入口兼容；工程问题的Document精度永不产生此jump。
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
        });
        self.personal.source_scroll = [output.state.offset.x, output.state.offset.y];
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
