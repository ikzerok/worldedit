use super::super::super::WorldeditApp;
use super::text::{active_mention, source_link_at_cursor, source_selection};
use crate::{highlight, theme};
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
        self.page_heading(
                    ui,
                    &relative,
                    "当前缓冲区与整个工程一起编译 · Ctrl+Enter 打开源码引用或按选中文本建档 · Ctrl+S 保存全部文件",
                );
        let stale_ime_draft = self
            .ime_source_draft
            .as_ref()
            .filter(|(draft_path, _, _)| draft_path == &path)
            .is_some_and(|(_, _, baseline)| {
                self.project.document(&path).ok() != Some(baseline.as_str())
            });
        if stale_ime_draft {
            ui.colored_label(
                theme::GOLD,
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
        let target = self.jump.take();
        let mut changed = false;
        let source_focused = ctx.memory(|memory| memory.has_focus(id));
        let ime_events = ctx.input(|input| input.events.clone());
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
        egui::ScrollArea::both()
            .id_salt(("source-scroll", &path))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let gutter = super::gutter::reserve(ui, &text);
                    ui.separator();
                    let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, _: f32| {
                        ui.fonts(|f| {
                            f.layout_job(highlight::layout_job(
                                buffer.as_str(),
                                14.0,
                                language_version,
                            ))
                        })
                    };
                    let editor_state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
                    let editor_focused = ctx.memory(|memory| memory.has_focus(id));
                    if editor_focused && !self.ime_composing {
                        if let Some(range) = editor_state.cursor.char_range() {
                            let cursor_char = range.primary.index;
                            let mention = (range.primary.index == range.secondary.index)
                                .then(|| active_mention(&text, cursor_char))
                                .flatten()
                                .filter(|(_, query)| !query.is_empty());
                            if let Some((at_char, query)) = mention {
                                let candidates = self
                                    .snapshot
                                    .as_ref()
                                    .map(|snapshot| {
                                        snapshot.result.analysis.catalog.search_objects(&query)
                                    })
                                    .unwrap_or_default();
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
                                    }) {
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
                    let mut output = egui::TextEdit::multiline(&mut text)
                        .id(id)
                        .code_editor()
                        .font(egui::FontId::monospace(14.0))
                        .desired_width(ui.available_width().max(500.0))
                        .desired_rows(36)
                        .frame(false)
                        .layouter(&mut layouter)
                        .show(ui);
                    super::gutter::paint(ui, gutter, &output.galley, output.galley_pos);
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
                    let active = if self.ime_composing {
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
                            if ctx.input_mut(|input| {
                                input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                            }) {
                                self.mention_suppression = Some(key);
                            } else {
                                let candidates = self
                                    .snapshot
                                    .as_ref()
                                    .map(|snapshot| {
                                        snapshot.result.analysis.catalog.search_objects(&query)
                                    })
                                    .unwrap_or_default();
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
                                if !candidates.is_empty() {
                                    mention_popup = Some((at_char, query, candidates, index));
                                }
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
        if let (Some((at_char, query, candidates, selected_index)), Some(anchor)) =
            (mention_popup.take(), mention_anchor)
        {
            let screen = ctx.screen_rect();
            let pos = egui::pos2(
                anchor.x.clamp(
                    screen.left() + 8.0,
                    (screen.right() - 440.0).max(screen.left() + 8.0),
                ),
                anchor.y.clamp(
                    screen.top() + 8.0,
                    (screen.bottom() - 120.0).max(screen.top() + 8.0),
                ),
            );
            egui::Area::new(egui::Id::new(("source-mention-popup", &path)))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(format!("引用 @{query}："));
                        ui.horizontal_wrapped(|ui| {
                            for (index, candidate) in candidates.into_iter().enumerate() {
                                let label = format!(
                                    "{} · {} · {}:{}",
                                    super::super::super::catalog::kind_label(
                                        &candidate.target.kind
                                    ),
                                    candidate.display,
                                    candidate.target.kind,
                                    candidate.target.id
                                );
                                if ui
                                    .selectable_label(index == selected_index, &label)
                                    .clicked()
                                {
                                    mention_action =
                                        Some((at_char, query.clone(), candidate.target));
                                }
                            }
                        });
                        ui.label(theme::muted("↑ / ↓ 选择 · Enter 插入 · Esc 收起"));
                    });
                });
        }
        if let (Some(selection), Some(anchor)) = (selected_source_text.take(), selection_anchor) {
            let screen = ctx.screen_rect();
            let pos = egui::pos2(
                anchor.x.clamp(
                    screen.left() + 8.0,
                    (screen.right() - 220.0).max(screen.left() + 8.0),
                ),
                anchor.y.clamp(
                    screen.top() + 8.0,
                    (screen.bottom() - 44.0).max(screen.top() + 8.0),
                ),
            );
            egui::Area::new(egui::Id::new(("source-selection-action", &path)))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(theme::muted("Ctrl+Enter 也可从选中文本建档"));
                        if ui.button("从选中文本建档").clicked() {
                            create_from_selection = Some(selection);
                        }
                    });
                });
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
