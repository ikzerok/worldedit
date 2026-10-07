use super::*;
use crate::theme;

pub(super) fn preview_text(preview: &SourceJumpPreview) -> String {
    let context = &preview.context;
    if context.text.is_empty() {
        return "▏（空行）".into();
    }
    let split = preview
        .position
        .byte_offset
        .saturating_sub(context.byte_range.start);
    // 字节边界和上下文完全来自 core；这里只在已确认的目标前后绘制光标符号。
    let (before, after) = context
        .text
        .get(..split)
        .zip(context.text.get(split..))
        .unwrap_or(("", context.text.as_str()));
    format!(
        "{}{}▏{}{}",
        if context.truncated_start { "…" } else { "" },
        before,
        after,
        if context.truncated_end { "…" } else { "" }
    )
}
impl WorldeditApp {
    pub(in crate::app) fn source_jump_window(&mut self, ctx: &egui::Context) {
        if !self.source_jump.open {
            return;
        }
        if self.tab != Tab::Edit || self.source_jump.path.as_ref() != Some(&self.active_file) {
            self.source_jump.open = false;
            return;
        }
        let mut refreshed = self.refresh_source_jump(false)
            || self.source_jump.projection_serial != self.source_jump.rendered_serial;
        let top = self.edit_layer_is_top("source-jump");
        let blocked = self.source_jump_blocked(ctx);
        let screen = ctx.screen_rect();
        let width = (screen.width() - 64.0).clamp(240.0, 560.0);
        let mut open = true;
        let mut close = false;
        let mut navigate = false;
        egui::Window::new("跳转到行列")
            .id(egui::Id::new("source-jump-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(width)
            .min_width(width)
            .max_width(width)
            .max_height((screen.height() - 96.0).max(120.0))
            .anchor(egui::Align2::CENTER_TOP, [0.0, 48.0])
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.add(
                    egui::Label::new(theme::muted(theme::relative_source(
                        &self.project.root,
                        &self.active_file,
                    )))
                    .truncate(),
                )
                .on_hover_text(theme::relative_source(
                    &self.project.root,
                    &self.active_file,
                ));
                let enter = top
                    && !blocked
                    && ctx.memory(|memory| memory.has_focus(query_id()))
                    && ui.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                    });
                let query = ui.add(
                    egui::TextEdit::singleline(&mut self.source_jump.query)
                        .id(query_id())
                        .desired_width(f32::INFINITY)
                        .hint_text("输入行号或行:列，例如 12 或 12:8"),
                );
                if self.source_jump.focus_query {
                    query.request_focus();
                    if let Some(mut state) = egui::TextEdit::load_state(ctx, query_id()) {
                        let end = self.source_jump.query.chars().count();
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::two(
                                egui::text::CCursor::new(0),
                                egui::text::CCursor::new(end),
                            )));
                        state.store(ctx, query_id());
                    }
                    self.source_jump.focus_query = false;
                }
                refreshed |= self.refresh_source_jump(false) || query.changed();
                if !refreshed
                    && ctx.input(|input| {
                        input
                            .events
                            .iter()
                            .any(|event| matches!(event, egui::Event::MouseWheel { .. }))
                    })
                {
                    self.source_jump.preview_ignore_momentum = false;
                }
                ui.add(
                    egui::Label::new(theme::muted(
                        "列按 Unicode 标量计数；Tab 为 1 列，行尾插入点可定位",
                    ))
                    .wrap(),
                );
                let ready = top
                    && !blocked
                    && !refreshed
                    && self
                        .source_jump
                        .review
                        .as_ref()
                        .is_some_and(|review| review.preview.is_ok());
                ui.horizontal_wrapped(|ui| {
                    navigate =
                        crate::theme::add_enabled(ui, ready, egui::Button::new("定位并收起"))
                            .clicked()
                            || (ready && enter);
                    if ui.button("取消").clicked() {
                        close = true;
                    }
                    if crate::theme::add_enabled(ui, !blocked, egui::Button::new("重新预览"))
                        .clicked()
                    {
                        self.refresh_source_jump(true);
                        refreshed = true;
                        navigate = false;
                    }
                    ui.label(theme::muted("Enter 定位 · Esc 取消"));
                });
                if blocked {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new("输入法组合或未提交稿尚未完成，定位暂不可用")
                                .color(theme::GOLD()),
                        )
                        .wrap(),
                    );
                } else if let Some(notice) = &self.source_jump.notice {
                    ui.add(egui::Label::new(theme::muted(notice)).wrap());
                }
                let Some(review) = &self.source_jump.review else {
                    return;
                };
                match &review.preview {
                    Err(error) => {
                        ui.add(
                            egui::Label::new(egui::RichText::new(error).color(theme::GOLD()))
                                .wrap(),
                        );
                    }
                    Ok(preview) => {
                        ui.add(
                            egui::Label::new(format!(
                                "行范围 1–{} · 第 {} 行列范围 1–{}",
                                preview.line_count, preview.position.line, preview.max_column
                            ))
                            .wrap(),
                        );
                        ui.strong(format!(
                            "目标：第 {} 行 · 第 {} 列",
                            preview.position.line, preview.position.column
                        ));
                        if preview.context.truncated_start || preview.context.truncated_end {
                            ui.label(theme::muted(format!(
                                "长行局部预览 · 从第 {} 列开始",
                                preview.context.start_column
                            )));
                        }
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            let salt = egui::Id::new("source-jump-context");
                            if refreshed {
                                // 新目标优先于旧预览的拖动速度与定位动画。
                                egui::scroll_area::State::default()
                                    .store(ctx, ui.make_persistent_id(salt));
                            }
                            egui::ScrollArea::vertical()
                                .id_salt(salt)
                                .animated(false)
                                .scroll_source(egui::scroll_area::ScrollSource {
                                    // egui 仍会逐帧送出旧滚轮的平滑余量；直到下一次真实滚轮
                                    // 事件才恢复该入口，不让余量把新目标立即推回旧位置。
                                    mouse_wheel: !self.source_jump.preview_ignore_momentum,
                                    ..egui::scroll_area::ScrollSource::ALL
                                })
                                .max_height(
                                    (screen.height() - ui.cursor().top() - 64.0).clamp(48.0, 120.0),
                                )
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    let (position, galley, _) = egui::Label::new(
                                        egui::RichText::new(preview_text(preview))
                                            .monospace()
                                            .size(self.personal.appearance().body_size),
                                    )
                                    .wrap()
                                    .layout_in_ui(ui);
                                    let marker = preview.position.column
                                        - preview.context.start_column
                                        + usize::from(preview.context.truncated_start);
                                    let target = galley
                                        .pos_from_cursor(egui::text::CCursor::new(marker))
                                        .translate(position.to_vec2());
                                    ui.painter().galley(
                                        position,
                                        galley,
                                        ui.visuals().text_color(),
                                    );
                                    // 仅新预览自动显示目标；作者之后手动滚动不会被每帧抢回。
                                    if refreshed {
                                        ui.scroll_to_rect(target, Some(egui::Align::Center));
                                    }
                                });
                        });
                    }
                }
            });
        // 同帧输入、来源重建或刷新只能显示新预览，不能消费旧 Enter/点击。
        self.source_jump.rendered_serial = self.source_jump.projection_serial;
        if !open || close {
            self.close_source_jump(ctx);
        } else if navigate && !refreshed {
            self.navigate_source_jump(ctx);
        }
    }
}
