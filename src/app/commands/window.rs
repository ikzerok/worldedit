use super::*;

impl WorldeditApp {
    pub(in crate::app) fn command_window(&mut self, ctx: &egui::Context) {
        if !self.command_palette.open {
            return;
        }
        let top = self.edit_layer_is_top("commands");
        let mut palette = std::mem::take(&mut self.command_palette);
        if self.ime_composing || palette.ime || palette.ime_frame {
            super::super::object_picker::consume_candidate_ime_keys(ctx);
            let query_id = egui::Id::new("author-command-query");
            if ctx.memory(|memory| memory.had_focus_last_frame(query_id)) {
                ctx.memory_mut(|memory| memory.request_focus(query_id));
            }
        }
        let mut open = true;
        let mut action = None;
        egui::Window::new("快速导航")
            .id(egui::Id::new("author-command-palette"))
            .frame(crate::theme::popup())
            .open(&mut open)
            .collapsible(false)
            .default_width(640.0)
            .max_width((ctx.screen_rect().width() - 40.0).max(240.0))
            .anchor(egui::Align2::CENTER_TOP, [0.0, 32.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let objects = ui
                        .selectable_value(&mut palette.commands_only, false, "对象")
                        .changed();
                    let commands = ui
                        .selectable_value(&mut palette.commands_only, true, "命令")
                        .changed();
                    if objects || commands {
                        palette.selected = 0;
                        palette.scroll_selected = true;
                    }
                });
                let response = ui.add(
                    egui::TextEdit::singleline(&mut palette.query)
                        .id(egui::Id::new("author-command-query"))
                        .desired_width(f32::INFINITY)
                        .hint_text(if palette.commands_only {
                            "输入任务名称 · ↑↓选择，Enter执行"
                        } else {
                            "名称、别名、类型、ID或来源 · ↑↓选择，Enter打开"
                        }),
                );
                if palette.focus {
                    response.request_focus();
                    palette.focus = false;
                }
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        response.id,
                        egui::EventFilter {
                            horizontal_arrows: true,
                            vertical_arrows: true,
                            escape: true,
                            ..Default::default()
                        },
                    )
                });
                if response.changed() {
                    palette.selected = 0;
                    palette.scroll_selected = true;
                }
                let keyboard = top && !self.ime_composing && !palette.ime && !palette.ime_frame;
                let mut stale = false;
                let entries: Vec<(String, Action)> = if palette.commands_only {
                    let query = palette.query.trim().to_lowercase();
                    let entries: Vec<_> = commands()
                        .into_iter()
                        .filter(|(label, _)| label.to_lowercase().contains(&query))
                        .map(|(label, action)| (label.into(), action))
                        .collect();
                    ui.small(format!("{} 项可执行命令 · Esc返回", entries.len()));
                    entries
                } else {
                    let filter = super::super::object_picker::filter(&[], None);
                    let mut view =
                        self.current_object_page(&mut palette.objects, &palette.query, &filter);
                    stale = view.stale;
                    if stale {
                        palette.selected = 0;
                        palette.scroll_selected = true;
                        palette.notice = Some("目录已变化，已回到首页；请重新选择".into());
                    }
                    if let Some(warning) = &view.warning {
                        ui.colored_label(
                            crate::theme::GOLD(),
                            "已应用目录 · 当前稿不可解析；总数仅含已解析对象",
                        )
                        .on_hover_text(warning);
                    } else {
                        ui.small("当前稿目录 · 保留完整对象身份 · PageUp / PageDown翻页");
                    }
                    let mut turned = palette.objects.controls(ui);
                    if keyboard && !stale {
                        let (next, previous) = ui.input_mut(|i| {
                            (
                                i.count_and_consume_key(egui::Modifiers::NONE, egui::Key::PageDown),
                                i.count_and_consume_key(egui::Modifiers::NONE, egui::Key::PageUp),
                            )
                        });
                        for previous in std::iter::repeat_n(false, next)
                            .chain(std::iter::repeat_n(true, previous))
                        {
                            if palette.objects.turn(previous) {
                                turned = true;
                                view = self.current_object_page(
                                    &mut palette.objects,
                                    &palette.query,
                                    &filter,
                                );
                            }
                        }
                    }
                    if turned {
                        palette.selected = 0;
                        palette.scroll_selected = true;
                        view =
                            self.current_object_page(&mut palette.objects, &palette.query, &filter);
                    }
                    match view.page {
                        Ok(page) => page
                            .items
                            .into_iter()
                            .map(|object| {
                                (
                                    super::super::object_picker::candidate_caption(
                                        &object,
                                        Some(&self.project.root),
                                    ),
                                    Action::Object(object, view.applied),
                                )
                            })
                            .collect(),
                        Err(error) => {
                            ui.colored_label(crate::theme::ERROR(), error);
                            Vec::new()
                        }
                    }
                };
                let previous_selection = palette.selected;
                if keyboard && !stale {
                    let (down, up) = ui.input_mut(|i| {
                        (
                            i.count_and_consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                            i.count_and_consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                        )
                    });
                    palette.selected = palette.selected.saturating_add(down).saturating_sub(up);
                }
                palette.selected = palette.selected.min(entries.len().saturating_sub(1));
                let scroll_selected =
                    palette.scroll_selected || previous_selection != palette.selected;
                let mut selected_visible = false;
                egui::ScrollArea::vertical()
                    .id_salt("author-command-results")
                    .max_height((ctx.screen_rect().height() - 270.0).clamp(120.0, 450.0))
                    .show(ui, |ui| {
                        for (index, (label, candidate)) in entries.iter().enumerate() {
                            let selected = index == palette.selected;
                            let row = if let Action::Object(object, _) = candidate {
                                super::super::object_picker::candidate_row_at_revision(
                                    ui,
                                    object,
                                    Some(&self.project.root),
                                    selected,
                                    palette.objects.serial,
                                )
                            } else {
                                let row = ui
                                    .push_id((&palette.query, label), |ui| {
                                        ui.selectable_label(selected, label)
                                    })
                                    .inner;
                                crate::theme::selection_frame(ui, &row, selected);
                                row
                            };
                            if row.clicked() && !stale {
                                action = Some(candidate.clone());
                            }
                            if selected {
                                selected_visible = ui.clip_rect().contains_rect(row.rect);
                                if scroll_selected {
                                    row.scroll_to_me(None);
                                }
                            }
                        }
                    });
                palette.scroll_selected = false;
                if !entries.is_empty() && !selected_visible {
                    ui.small("选中项在视野外；用↑↓定位或点击可见项");
                }
                if entries.is_empty() {
                    ui.label("没有匹配项；可修改输入或Esc返回");
                }
                if let Some(notice) = &palette.notice {
                    ui.colored_label(crate::theme::GOLD(), notice);
                }
                if keyboard
                    && !stale
                    && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
                    && selected_visible
                {
                    action = entries
                        .get(palette.selected)
                        .map(|(_, action)| action.clone());
                }
            });
        palette.open = open && action.is_none();
        if !palette.open {
            if let Some(id) = palette.previous_focus {
                ctx.memory_mut(|m| m.request_focus(id));
            }
        }
        self.command_palette = palette;
        if let Some(action) = action {
            self.execute_palette_action(ctx, action);
        }
    }

    fn execute_palette_action(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Tab(tab) => self.switch_tab(tab),
            Action::Object(object, applied) => {
                if !self.navigate_object_candidate(ctx, &object, applied, true) {
                    self.command_palette.open = true;
                    self.command_palette.focus = true;
                    self.command_palette.notice = self.message.clone();
                }
            }
            Action::MoveEntitySource => self.begin_current_entity_source_move(),
            Action::SourceOutline => self.open_source_outline(ctx),
            Action::SourceJump => self.open_source_jump(ctx),
            Action::Save => {
                self.save();
            }
            Action::Focus => self.personal.settings.focus = !self.personal.settings.focus,
            Action::Navigation => {
                if self.focus_style() {
                    if self.navigation_drawer_open(ctx) {
                        self.close_navigation_drawer(ctx);
                    } else {
                        self.open_navigation_drawer(ctx);
                    }
                } else {
                    self.personal.settings.navigation = !self.personal.settings.navigation;
                }
            }
            Action::References => {
                self.personal.settings.references_visible =
                    !self.personal.settings.references_visible
            }
            Action::Back => self.author_back(ctx),
            Action::Settings => self.personal.preferences_open = true,
            Action::Capabilities => self.open_capabilities(),
            Action::TemporalIssues => self.open_temporal_issues(ctx),
            Action::Problems => self.open_problems(ctx),
            Action::NextProblem => {
                self.open_problems(ctx);
                self.step_problem(ctx, false, true);
            }
            Action::PreviousProblem => {
                self.open_problems(ctx);
                self.step_problem(ctx, true, true);
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
