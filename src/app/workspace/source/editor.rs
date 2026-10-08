use super::super::super::WorldeditApp;
use super::text::{active_mention, source_link_at_cursor, source_selection};
use crate::app::personal::source_view::SourceFrame;
use crate::theme;
mod layout;
mod mentions;
mod problem_marker;
mod selection_action;
use std::path::PathBuf;

impl WorldeditApp {
    pub(super) fn source_text_editor(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        path: PathBuf,
        relative: String,
        mut text: String,
    ) {
        if let Some((ime_path, ime_text, _)) = &self.ime_source_draft {
            if ime_path == &path {
                text = ime_text.clone();
            }
        }
        let previous_tab = self.tab;
        let compact_heading = self.source_outline_heading(ctx, ui, &path, &relative);
        // 页头中的返回/批注可能已切换来源，旧编辑器不能消费新位置的恢复请求。
        if self.active_file != path || self.tab != previous_tab {
            return;
        }
        if !compact_heading {
            self.problem_source_summary(ui, &path);
        }
        let stale_ime_draft = self
            .ime_source_draft
            .as_ref()
            .filter(|(draft_path, _, _)| draft_path == &path)
            .is_some_and(|(_, _, baseline)| {
                self.project.document(&path).ok() != Some(baseline.as_str())
            });
        if stale_ime_draft {
            ui.colored_label(
                theme::GOLD(),
                "源码在输入法组合期间被外部修改。草稿已保留，尚未写入工程。",
            );
            if ui.button("放弃本地草稿并恢复外部版本").clicked() {
                self.ime_source_draft = None;
                self.ime_source_baseline = None;
                self.ime_composing = false;
                return;
            }
        }
        let id = egui::Id::new(("source", &path));
        crate::app::writing_workspace::prepare_text_undo(ctx, id, &text);
        let target = self.jump.take();
        let mut changed = false;
        let mut source_focused = ctx.memory(|memory| memory.has_focus(id));
        let ime_events = ctx.input(|input| input.events.clone());
        let ime_frame = ime_events
            .iter()
            .any(|event| matches!(event, egui::Event::Ime(_)));
        let ime_committed = ime_events
            .iter()
            .any(|event| matches!(event, egui::Event::Ime(egui::ImeEvent::Commit(_))));
        if ime_frame
            && ctx.memory(|memory| memory.had_focus_last_frame(id) && memory.focused().is_none())
        {
            ctx.memory_mut(|memory| memory.request_focus(id));
            source_focused = true;
        }
        if ime_frame && source_focused {
            crate::app::object_picker::consume_candidate_ime_keys(ctx);
        }
        for event in &ime_events {
            match event {
                egui::Event::Ime(egui::ImeEvent::Enabled | egui::ImeEvent::Preedit(_))
                    if source_focused =>
                {
                    self.ime_composing = true;
                    if self
                        .ime_source_baseline
                        .as_ref()
                        .is_none_or(|(baseline_path, _)| baseline_path != &path)
                    {
                        self.ime_source_baseline = self
                            .project
                            .document(&path)
                            .ok()
                            .map(|baseline| (path.clone(), baseline.to_owned()));
                    }
                }
                egui::Event::Ime(egui::ImeEvent::Commit(_) | egui::ImeEvent::Disabled) => {
                    self.ime_composing = false;
                }
                _ => {}
            }
        }
        let mut mention_action = None;
        let mut mention_popup = None;
        let mut mention_anchor = None;
        let mut create_from_selection = None;
        let mut selected_source_text = None;
        let mut selection_anchor = None;
        let mut source_link_action = None;
        let language_version = self.project.language_version_kind();
        let body_size = self.personal.appearance().source_size;
        let line_height = body_size * self.personal.appearance().line_spacing;
        let wrap = self.personal.settings.source_wrap;
        let problem_range = self.problem_source_range(&path);
        let previous_view = self
            .personal
            .source_view
            .as_ref()
            .filter(|(view_path, _)| view_path == &path)
            .map(|(_, view)| view.clone());
        let mut source_frame = None;
        let mut search_navigation = false;
        let scroll_salt = egui::Id::new(("source-scroll", &path));
        let mut scroll = if wrap {
            egui::ScrollArea::vertical().horizontal_scroll_offset(0.0)
        } else {
            egui::ScrollArea::both()
        };
        let restoring_scroll = self.personal.restore_source && target.is_none();
        if restoring_scroll {
            let offset = egui::vec2(
                self.personal.source_scroll[0],
                self.personal.source_scroll[1],
            );
            // 完整返回位置优先于上一命中的动画和惯性。
            let mut state = egui::scroll_area::State::default();
            state.offset = offset;
            state.store(ctx, ui.make_persistent_id(scroll_salt));
            scroll = scroll.scroll_offset(offset).animated(false);
        }
        self.personal.restore_source = false;
        let mut scroll_output = scroll
            .id_salt(("source-scroll", &path))
            // 短视口也按真实剩余高度定位；默认64点下限会把目标滚入裁剪区外。
            .min_scrolled_height(0.0)
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                ui.horizontal_top(|ui| {
                    let gutter = super::gutter::reserve(ui, &text, body_size);
                    ui.separator();
                    let mut layouter =
                        |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, width: f32| {
                            ui.fonts(|f| {
                                f.layout_job(layout::job(
                                    buffer.as_str(),
                                    body_size,
                                    line_height,
                                    language_version,
                                    wrap,
                                    width,
                                ))
                            })
                        };
                    let editor_state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
                    let editor_focused = ctx.memory(|memory| memory.has_focus(id));
                    if editor_focused && !self.ime_composing && !ime_frame {
                        if let Some(range) = editor_state.cursor.char_range() {
                            let cursor_char = range.primary.index;
                            let mention = (range.primary.index == range.secondary.index)
                                .then(|| active_mention(&text, cursor_char))
                                .flatten()
                                .filter(|(_, query)| !query.is_empty())
                                .filter(|(at, query)| {
                                    self.mention_suppression.as_ref()
                                        != Some(&(path.clone(), *at, query.clone()))
                                });
                            if let Some((at_char, query)) = mention {
                                let page = self.mention_page(ctx, &path, at_char, &query, true);
                                let candidates = page.items;
                                if !candidates.is_empty() {
                                    let matches_current =
                                        self.mention_selection.as_ref().is_some_and(
                                            |(selected_path, selected_at, selected_query, _)| {
                                                selected_path == &path
                                                    && *selected_at == at_char
                                                    && selected_query == &query
                                            },
                                        );
                                    let mut selected_index = self
                                        .mention_selection
                                        .as_ref()
                                        .filter(|_| matches_current)
                                        .map(|(_, _, _, index)| *index)
                                        .unwrap_or(0)
                                        .min(candidates.len() - 1);
                                    let moved_down = ctx.input_mut(|input| {
                                        input.consume_key(
                                            egui::Modifiers::NONE,
                                            egui::Key::ArrowDown,
                                        )
                                    });
                                    let moved_up = ctx.input_mut(|input| {
                                        input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
                                    });
                                    if moved_down {
                                        selected_index = (selected_index + 1) % candidates.len();
                                    } else if moved_up {
                                        selected_index = if selected_index == 0 {
                                            candidates.len() - 1
                                        } else {
                                            selected_index - 1
                                        };
                                    }
                                    if moved_down || moved_up {
                                        self.mention_selection = Some((
                                            path.clone(),
                                            at_char,
                                            query.clone(),
                                            selected_index,
                                        ));
                                    }
                                    if ctx.input_mut(|input| {
                                        input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                                    }) {
                                        self.mention_suppression =
                                            Some((path.clone(), at_char, query.clone()));
                                    } else if ctx.input_mut(|input| {
                                        input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                                    }) && !page.stale
                                        && page.visible.contains(&candidates[selected_index].target)
                                    {
                                        mention_action = Some((
                                            at_char,
                                            query.clone(),
                                            candidates[selected_index].target.clone(),
                                        ));
                                    }
                                }
                            }

                            if let Some(selection) = source_selection(&text, &path, range) {
                                if ctx.input_mut(|input| {
                                    input.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                                }) {
                                    create_from_selection = Some(selection);
                                }
                            } else if range.primary.index == range.secondary.index {
                                let target = self.snapshot.as_ref().and_then(|snapshot| {
                                    source_link_at_cursor(
                                        &text,
                                        &path,
                                        cursor_char,
                                        &snapshot.result.analysis.catalog.text_links,
                                    )
                                });
                                if let Some(target) = target {
                                    if ctx.input_mut(|input| {
                                        input
                                            .consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                                    }) {
                                        source_link_action = Some((target, cursor_char));
                                    }
                                }
                            }
                        }
                    }
                    search_navigation = crate::app::search::restore_editor_selection(
                        ui, id, &path, &text, 0, &text,
                    );
                    let marker = ui.painter().add(egui::Shape::Noop);
                    let mut output = egui::TextEdit::multiline(&mut text)
                        .id(id)
                        .code_editor()
                        .font(theme::source_font(body_size))
                        .desired_width(if wrap {
                            ui.available_width().max(1.0)
                        } else {
                            ui.available_width().max(500.0)
                        })
                        .desired_rows(36)
                        .frame(false)
                        .layouter(&mut layouter)
                        .show(ui);
                    if !output.response.changed() {
                        if let Some(range) = problem_range.clone() {
                            ui.painter().set(
                                marker,
                                egui::Shape::Vec(problem_marker::shapes(
                                    &output.galley,
                                    output.galley_pos,
                                    gutter,
                                    ui.clip_rect(),
                                    range,
                                )),
                            );
                        }
                    }
                    super::gutter::paint(ui, gutter, &output.galley, output.galley_pos, body_size);
                    crate::app::search::observe_manual_selection(ctx, &output, search_navigation);
                    crate::app::writing_workspace::remember_text_undo(ctx, id, &text);
                    crate::app::search::scroll_editor_selection(ui, &output);
                    if output.response.has_focus() {
                        if let Some(range) = output.cursor_range {
                            crate::app::search::record_editor_selection(
                                ctx, id, &path, None, &text, 0, &text, range,
                            );
                        }
                    }
                    changed = output.response.changed();
                    if output.response.clicked() {
                        if let (Some(pointer), Some(snapshot)) = (
                            output.response.interact_pointer_pos(),
                            self.snapshot.as_ref(),
                        ) {
                            let cursor = output
                                .galley
                                .cursor_from_pos(pointer - output.galley_pos)
                                .index;
                            if let Some(target) = source_link_at_cursor(
                                &text,
                                &path,
                                cursor,
                                &snapshot.result.analysis.catalog.text_links,
                            ) {
                                source_link_action = Some((target, cursor));
                            }
                        }
                    }
                    // Commit 帧只准备浮层首次不可见测量，不能响应候选操作。
                    let active = if self.ime_composing
                        || (ime_frame && !ime_committed)
                        || !self.source_mention_has_input(ctx, &path)
                        || self.command_palette.open
                        || self.search_open
                        || self.personal.preferences_open
                        || self
                            .command_palette
                            .focus_stack
                            .iter()
                            .any(|(kind, _)| *kind != "problems")
                    {
                        None
                    } else {
                        output
                            .state
                            .cursor
                            .char_range()
                            .filter(|range| range.primary.index == range.secondary.index)
                            .and_then(|range| active_mention(&text, range.primary.index))
                            .filter(|(_, query)| !query.is_empty())
                    };
                    if active.is_some() || self.ime_composing || ime_frame {
                        ui.memory_mut(|memory| {
                            memory.set_focus_lock_filter(
                                id,
                                egui::EventFilter {
                                    tab: true,
                                    horizontal_arrows: true,
                                    vertical_arrows: true,
                                    escape: true,
                                },
                            )
                        });
                    }
                    let active_key = active
                        .as_ref()
                        .map(|(at_char, query)| (path.clone(), *at_char, query.clone()));
                    if self
                        .mention_suppression
                        .as_ref()
                        .is_some_and(|suppressed| Some(suppressed) != active_key.as_ref())
                    {
                        self.mention_suppression = None;
                    }
                    if self.mention_selection.as_ref().is_some_and(
                        |(selected_path, selected_at, selected_query, _)| {
                            !active_key.as_ref().is_some_and(
                                |(active_path, active_at, active_query)| {
                                    active_path == selected_path
                                        && active_at == selected_at
                                        && active_query == selected_query
                                },
                            )
                        },
                    ) {
                        self.mention_selection = None;
                    }
                    if let Some((at_char, query)) = active {
                        let key = (path.clone(), at_char, query.clone());
                        if self.mention_suppression.as_ref() != Some(&key) {
                            if !ime_frame
                                && ctx.input_mut(|input| {
                                    input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                                })
                            {
                                self.mention_suppression = Some(key);
                            } else {
                                let candidates =
                                    self.mention_page(ctx, &path, at_char, &query, false).items;
                                let index = self
                                    .mention_selection
                                    .as_ref()
                                    .filter(|(selected_path, selected_at, selected_query, _)| {
                                        selected_path == &path
                                            && *selected_at == at_char
                                            && selected_query == &query
                                    })
                                    .map(|(_, _, _, index)| *index)
                                    .unwrap_or(0)
                                    .min(candidates.len().saturating_sub(1));
                                let cursor = egui::text::CCursor::new(
                                    output
                                        .state
                                        .cursor
                                        .char_range()
                                        .map(|range| range.primary.index)
                                        .unwrap_or_default(),
                                );
                                mention_anchor = Some(
                                    output
                                        .galley
                                        .pos_from_cursor(cursor)
                                        .translate(output.galley_pos.to_vec2())
                                        .left_bottom(),
                                );
                                mention_popup = Some((at_char, query, index));
                            }
                        }
                    }
                    if let Some(range) = output.state.cursor.char_range() {
                        if let Some(selection) = source_selection(&text, &path, range) {
                            let start = range.primary.index.min(range.secondary.index);
                            let cursor = egui::text::CCursor::new(start);
                            selection_anchor = Some(
                                output
                                    .galley
                                    .pos_from_cursor(cursor)
                                    .translate(output.galley_pos.to_vec2())
                                    .left_bottom(),
                            );
                            selected_source_text = Some(selection);
                        }
                    }
                    if let Some((line, column)) = target {
                        let offset = text
                            .split_inclusive('\n')
                            .take(line.saturating_sub(1) as usize)
                            .map(|l| l.chars().count())
                            .sum::<usize>()
                            + column.saturating_sub(1) as usize;
                        let cursor = egui::text::CCursor::new(offset.min(text.chars().count()));
                        output
                            .state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(cursor)));
                        output.state.clone().store(ctx, id);
                        output.response.request_focus();
                        let rect = output
                            .galley
                            .pos_from_cursor(cursor)
                            .translate(output.galley_pos.to_vec2());
                        ui.scroll_to_rect(rect, Some(egui::Align::Center));
                    }
                    source_frame = Some(SourceFrame::new(
                        &output,
                        viewport,
                        self.personal.appearance(),
                        wrap,
                    ));
                });
                if restoring_scroll {
                    ui.scroll_to_rect(ui.clip_rect(), None);
                }
            });
        if let Some(frame) = source_frame {
            let view = frame.finish(
                ctx,
                &mut scroll_output,
                previous_view.as_ref(),
                target.is_some() || search_navigation,
            );
            self.personal.source_view = Some((path.clone(), view));
        }
        self.personal.source_scroll = [scroll_output.state.offset.x, scroll_output.state.offset.y];
        if let (Some((at_char, query, selected_index)), Some(anchor)) =
            (mention_popup.take(), mention_anchor)
        {
            if let Some(target) =
                self.mention_popup(ctx, &path, anchor, at_char, &query, selected_index)
            {
                mention_action = Some((at_char, query, target));
            }
        }
        if let Some(selection) = self.source_selection_suggestion(
            ctx,
            id,
            &path,
            source_focused,
            selected_source_text.take(),
            selection_anchor,
        ) {
            create_from_selection = Some(selection);
        }
        if self.ime_composing {
            if changed {
                if let Some((baseline_path, baseline)) = &self.ime_source_baseline {
                    if baseline_path == &path {
                        self.ime_source_draft = Some((path.clone(), text, baseline.clone()));
                    }
                }
            }
        } else if changed {
            let baseline = self
                .ime_source_draft
                .as_ref()
                .filter(|(draft_path, _, _)| draft_path == &path)
                .map(|(_, _, baseline)| baseline.clone())
                .or_else(|| {
                    self.ime_source_baseline
                        .as_ref()
                        .filter(|(baseline_path, _)| baseline_path == &path)
                        .map(|(_, baseline)| baseline.clone())
                });
            let current = self.project.document(&path).ok().map(str::to_owned);
            if baseline
                .as_ref()
                .is_some_and(|baseline| current.as_deref() != Some(baseline.as_str()))
            {
                let baseline = baseline.unwrap();
                self.ime_source_draft = Some((path.clone(), text, baseline));
                self.io_error = Some(
                            "源码在输入法组合期间被外部修改。输入已保留；可复制草稿，或放弃草稿并恢复外部版本。".into(),
                        );
            } else {
                let before = self.project.clone();
                if self.project.set_text(&path, text).is_ok() {
                    self.remember(before);
                    self.recompile();
                    self.ime_source_draft = None;
                    self.ime_source_baseline = None;
                }
            }
        } else if !self.ime_composing {
            self.ime_source_baseline = None;
        }
        if let Some((at_char, query, target)) = mention_action {
            self.apply_source_mention(ctx, &path, at_char, &query, target);
        }
        if let Some(selection) = create_from_selection {
            self.edit_entity(None);
            if let Some(form) = self.entity_editor.as_mut() {
                form.source_selection = Some(selection.clone());
                form.draft.display = selection.expected_text.clone();
                form._focus_name_on_open = true;
            }
        }
        if let Some((target, cursor)) = source_link_action {
            ctx.memory_mut(|memory| memory.surrender_focus(id));
            self.reading_return = Some((path.clone(), cursor));
            self.open_reading(target);
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod selection_tests;

#[cfg(test)]
mod scroll_tests;
