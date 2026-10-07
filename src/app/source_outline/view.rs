use super::*;
use crate::theme;

fn row_text(
    ui: &egui::Ui,
    entry: &SourceOutlineEntry,
    current: bool,
    size: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &entry.display,
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(size),
            color: ui.visuals().text_color(),
            ..Default::default()
        },
    );
    job.append(
        &format!(
            "\n{}{}",
            if current { "● 当前位置 · " } else { "" },
            metadata(entry)
        ),
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional((size - 2.0).max(12.0)),
            color: theme::MUTED(),
            ..Default::default()
        },
    );
    job
}

fn row(
    ui: &mut egui::Ui,
    entry: &SourceOutlineEntry,
    current: bool,
    focused: bool,
    size: f32,
) -> egui::Response {
    let job = row_text(ui, entry, current, size);
    let indent = (entry.depth.min(4) as f32) * 12.0;
    let response = ui
        .allocate_ui_with_layout(
            egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
            egui::Layout::left_to_right(egui::Align::Center).with_main_align(egui::Align::Min),
            |ui| {
                ui.add_space(indent);
                // add_sized 会把 Button 的内容居中；用最小尺寸保留全宽点击区，
                // 水平布局默认也居中，因此显式设 main_align，缩进不受文字长度影响。
                ui.add(
                    egui::Button::selectable(current, job)
                        .wrap()
                        .min_size(egui::vec2(ui.available_width(), 0.0)),
                )
            },
        )
        .inner;
    if focused {
        ui.painter().rect_stroke(
            response.rect.shrink(0.5),
            4,
            egui::Stroke::new(1.5_f32, theme::ACCENT()),
            egui::StrokeKind::Inside,
        );
    }
    response
}
impl WorldeditApp {
    pub(in crate::app) fn source_outline_window(&mut self, ctx: &egui::Context) {
        if !self.source_outline.open {
            return;
        }
        if self.tab != Tab::Edit || self.source_outline.path.as_ref() != Some(&self.active_file) {
            self.source_outline.open = false;
            return;
        }
        let refreshed = self.refresh_source_outline(false)
            || self.source_outline.projection_serial != self.source_outline.rendered_serial;
        let current = self.source_outline_current(ctx);
        let top = self.edit_layer_is_top("source-outline");
        let blocked = self.source_outline_blocked(ctx);
        let screen = ctx.screen_rect();
        let width = (screen.width() - 48.0).clamp(200.0, 620.0);
        let mut state = std::mem::take(&mut self.source_outline);
        let mut open = true;
        let mut close = false;
        let mut refresh = false;
        let mut action = None;
        egui::Window::new("本文件结构")
            .id(egui::Id::new("source-outline-window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(width)
            .min_width(width)
            .max_width(width)
            .max_height((screen.height() - 96.0).max(100.0))
            .anchor(egui::Align2::CENTER_TOP, [0.0, 48.0])
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.add(
                    egui::Label::new(theme::muted(theme::relative_source(
                        &self.project.root,
                        &self.active_file,
                    )))
                    .wrap(),
                );
                let keys_enabled =
                    top && !blocked && ctx.memory(|memory| memory.has_focus(query_id()));
                let (down, up, first, last, enter) = if keys_enabled {
                    ui.input_mut(|input| {
                        (
                            input
                                .count_and_consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                            input.count_and_consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                            input.consume_key(egui::Modifiers::NONE, egui::Key::Home),
                            input.consume_key(egui::Modifiers::NONE, egui::Key::End),
                            input.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                        )
                    })
                } else {
                    (0, 0, false, false, false)
                };
                let query = ui.add(
                    egui::TextEdit::singleline(&mut state.query)
                        .id(query_id())
                        .desired_width(f32::INFINITY)
                        .hint_text("筛选名称、完整 ID 或类型"),
                );
                if state.focus_query {
                    query.request_focus();
                    state.focus_query = false;
                }
                let query_changed = query.changed() || state.query != state.last_query;
                if query_changed {
                    state.selected = 0;
                    state.scroll_selected = true;
                    state.last_query = state.query.clone();
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button("收起并回到源码").clicked() {
                        close = true;
                    }
                    if crate::theme::add_enabled(ui, !blocked, egui::Button::new("刷新结构"))
                        .clicked()
                    {
                        refresh = true;
                    }
                    ui.label(theme::muted("↑↓选择 · Home/End 首末 · Enter定位 · Esc收起"));
                });
                if blocked {
                    ui.colored_label(theme::GOLD(), "输入法组合或未提交稿尚未完成，定位暂不可用");
                }
                if let Some(notice) = &state.notice {
                    ui.add(
                        egui::Label::new(egui::RichText::new(notice).color(theme::GOLD())).wrap(),
                    );
                }
                let Some(cache) = &state.cache else {
                    ui.label("当前文件不可定位");
                    return;
                };
                if cache.outline.status != SourceOutlineStatus::Ready {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(
                                cache
                                    .outline
                                    .message
                                    .as_deref()
                                    .unwrap_or("当前稿结构暂不可用"),
                            )
                            .color(theme::GOLD()),
                        )
                        .wrap(),
                    );
                    return;
                }
                let entries = filtered(&cache.outline, &state.query);
                if entries.is_empty() {
                    ui.label(if cache.outline.entries.is_empty() {
                        "本文件没有显式声明；仍可直接编辑源码"
                    } else {
                        "没有匹配的声明；试试名称、完整 ID 或类型"
                    });
                    return;
                }
                let navigation = top && !blocked && !query_changed && !refreshed && !refresh;
                let before = state.selected;
                if navigation {
                    state.selected = state.selected.saturating_add(down).saturating_sub(up);
                    if first {
                        state.selected = 0;
                    }
                    if last {
                        state.selected = entries.len() - 1;
                    }
                }
                state.selected = state.selected.min(entries.len() - 1);
                let scroll_selected = state.scroll_selected || state.selected != before;
                let mut visible = false;
                let height = (screen.bottom() - 64.0 - ui.cursor().top()).max(70.0);
                egui::ScrollArea::vertical()
                    .id_salt("source-outline-results")
                    .max_height(height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for (index, occurrence) in entries.iter().enumerate() {
                            let Some(entry) = cache.outline.entries.get(*occurrence) else {
                                continue;
                            };
                            let focused = state.selected == index;
                            let response = row(
                                ui,
                                entry,
                                current == Some(*occurrence),
                                focused && top && ctx.memory(|memory| memory.has_focus(query_id())),
                                self.personal.appearance().body_size,
                            );
                            if navigation && response.clicked() {
                                action = Some(*occurrence);
                            }
                            if focused {
                                visible = ui.clip_rect().contains_rect(response.rect);
                                if scroll_selected {
                                    response.scroll_to_me(None);
                                }
                            }
                        }
                    });
                state.scroll_selected = false;
                if !visible {
                    ui.label(theme::muted("选择项在视野外；按↑↓显示后再定位"));
                }
                if navigation && visible && enter {
                    action = entries.get(state.selected).copied();
                }
            });
        state.rendered_serial = state.projection_serial;
        self.source_outline = state;
        if refresh {
            self.refresh_source_outline(true);
            self.source_outline.notice = None;
        }
        if !open || close {
            self.close_source_outline(ctx);
        } else if let Some(occurrence) = action {
            self.navigate_source_outline(ctx, occurrence);
        }
    }
}

#[cfg(test)]
mod tests;
