use super::*;
#[cfg(not(target_arch = "wasm32"))]
fn frame_profile_input_active(ctx: &egui::Context) -> bool {
    ctx.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Copy
                    | egui::Event::Cut
                    | egui::Event::Paste(_)
                    | egui::Event::Text(_)
                    | egui::Event::Key { .. }
                    | egui::Event::PointerMoved(_)
                    | egui::Event::MouseMoved(_)
                    | egui::Event::PointerButton { .. }
                    | egui::Event::Zoom(_)
                    | egui::Event::Touch { .. }
                    | egui::Event::MouseWheel { .. }
            )
        })
    })
}
impl eframe::App for WorldeditApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.template_manager
            .filter_raw_input(ctx, raw, self.tab == Tab::Templates);
        self.manuscript_raw_input_hook(ctx, raw);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.catalog_workbench.save_favorites(storage);
        self.personal.save(storage);
        self.catalog_workbench.save_columns(storage);
    }

    // 原生持久化仅保存明确的个人收藏，不顺带保存编辑草稿或 egui 窗口状态。
    #[cfg(not(target_arch = "wasm32"))]
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn clear_color(&self, visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::from(visuals.panel_fill).to_array()
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Escape 可能在 author_shortcuts 取消准备；本帧其余快捷键仍不得穿透。
        let rehearsal_preparation = self.draft_rehearsal.preparation_blocks_frame(ctx);
        let closing = self.reader_app_close_pending();
        let appearance_shortcuts_blocked = self.ime_composing
            || self.command_palette.ime
            || ctx.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Ime(_)))
            });
        if !closing && !appearance_shortcuts_blocked {
            self.personal.appearance_shortcuts(ctx);
        }
        let _theme = crate::theme::configure_appearance(ctx, self.personal.appearance());
        if closing {
            if self.poll_reader_app_close(ctx) {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.heading("正在清理后台任务");
                    if self.reader_app_close_failed() {
                        ui.label("无法确认后台清理结果，请检查目标与退出状态；尚未允许退出。");
                    } else {
                        ui.label("等待本次临时文件清理与原子提交的真实结果，然后关闭工作台。");
                        ui.spinner();
                    }
                    if let Some(error) = &self.io_error {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                });
            }
            return;
        }
        self.capture_new_draft_baselines();
        self.frame_dirty_drafts = self.dirty_draft_names();
        if self.personal.pending_restore {
            self.restore_personal_view(ctx);
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.conflict_view.prepare_frame(ctx);
        #[cfg(not(target_arch = "wasm32"))]
        let reconciliation_open = self.conflict_view.is_open();
        #[cfg(target_arch = "wasm32")]
        let reconciliation_open = false;
        if !reconciliation_open {
            self.author_shortcuts(ctx);
            self.prepare_play_keyboard(ctx);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let input_active = self
            .frame_profile
            .as_ref()
            .filter(|profile| profile.is_active())
            .map(|_| frame_profile_input_active(ctx));
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(profile) = self.frame_profile.as_mut() {
            profile.begin_frame(_frame.info().cpu_usage);
        }
        if self.event_editor.is_none()
            && self.character_editor.is_none()
            && self.world_editor.is_none()
            && self.tag_editor.is_none()
            && self.state_editor.is_none()
            && self.anchor_editor.is_none()
            && self.wiki_editor.is_none()
            && self.entity_editor.is_none()
            && self.relation_editor.is_none()
            && self.relation_type_editor.is_none()
            && self.delete_form.is_none()
            && self.rename_form.is_none()
        {
            self.stale_form = false;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
            if self.saved_location
                && self.last_refresh.elapsed() >= std::time::Duration::from_secs(1)
            {
                self.last_refresh = std::time::Instant::now();
                let scan = worldline_core::file_access::workspace_files(&self.project.root)
                    .and_then(|paths| {
                        paths
                            .into_iter()
                            .map(|p| {
                                let m = std::fs::metadata(&p)?;
                                Ok((p, m.len(), m.modified().ok()))
                            })
                            .collect::<std::io::Result<Vec<_>>>()
                    });
                match scan {
                    Ok(stamp) if stamp != self.disk_stamp => {
                        let baseline = self.project.content_baseline();
                        let recovery_conflicts = self.project.recovery_conflicts().to_vec();
                        let had_draft = self.has_open_authoring_form();
                        match self.project.refresh() {
                            Ok(conflicts) => {
                                self.disk_stamp = stamp;
                                // 保存、触碰时间戳也会改变磁盘元数据；只有 core 确认
                                // 缓冲变化、外部冲突或恢复安全状态变化时，才使版本失效。
                                let editing_state_changed = self.project.content_baseline()
                                    != baseline
                                    || !conflicts.is_empty()
                                    || self.project.recovery_conflicts() != recovery_conflicts;
                                self.map_canvas.invalidate_rasters();
                                if editing_state_changed {
                                    if had_draft {
                                        self.stale_form = true;
                                    }
                                    self.history.clear();
                                    self.redo.clear();
                                    if !self.project.documents.contains_key(&self.active_file) {
                                        self.active_file = self.project.entry.clone();
                                    }
                                    self.recompile();
                                }
                                if !conflicts.is_empty() {
                                    self.io_error = Some(format!(
                                        "外部修改与未保存内容冲突，已保留缓冲：{}",
                                        conflicts
                                            .iter()
                                            .map(|p| p.display().to_string())
                                            .collect::<Vec<_>>()
                                            .join("、")
                                    ));
                                }
                            }
                            Err(e) => self.io_error = Some(e),
                        }
                    }
                    Err(e) => self.io_error = Some(e.to_string()),
                    _ => {}
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        self.browser_events(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request_action(Pending::Close, ctx);
            if self.reader_app_close_pending() {
                return;
            }
        }
        if !reconciliation_open
            && !rehearsal_preparation
            && !self.ime_composing
            && !self.command_palette.ime
            && !self.command_palette.ime_frame
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::S))
        {
            self.save();
        }
        if !reconciliation_open
            && !rehearsal_preparation
            && !self.ime_composing
            && !self.command_palette.ime
            && !self.command_palette.ime_frame
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O))
        {
            self.open_dialog(ctx, false);
        }
        self.poll_problems(ctx);
        self.poll_playthrough_report(ctx);
        self.poll_manuscript_delivery(ctx);
        self.top_bar(ctx);
        self.catalog_scope_return_bar(ctx);
        self.status_bar(ctx);
        self.problems_panel(ctx);
        if self.personal.settings.navigation
            && !self.personal.settings.focus
            && !self.compact_workspace_navigation(ctx)
        {
            self.sidebar(ctx);
        }
        self.docked_reading(ctx);
        if self.tab != Tab::Play {
            self.poll_replay(ctx);
            self.poll_comparison(ctx);
        }
        self.review_navigation_guard(ctx);
        object_picker::set_workspace_snapshot(ctx, &self.project.root, self.version);
        match self.tab {
            Tab::Timeline | Tab::Graph => {
                self.event_inspector(ctx);
                self.canvas_tab(ctx);
            }
            Tab::Map => self.map_tab(ctx),
            Tab::Network => self.network_tab(ctx),
            Tab::Review => self.review_tab(ctx),
            Tab::Overview => self.overview_tab(ctx),
            Tab::Edit => self.source_tab(ctx),
            Tab::Characters => self.characters_tab(ctx),
            Tab::Catalog => self.catalog_tab(ctx),
            Tab::CatalogImport => self.catalog_import_tab(ctx),
            Tab::Wiki => self.wiki_tab(ctx),
            Tab::World => self.world_tab(ctx),
            Tab::Play => self.play_tab(ctx),
            Tab::Localization => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    if localization_ui::show(
                        ui,
                        &mut self.project,
                        &mut self.localization_ui,
                        self.version,
                    ) {
                        if let Some(before) = self.localization_ui.take_applied_before() {
                            self.remember(before);
                        }
                        self.recompile();
                        self.message = Some("本地化修改已应用到工程，可撤销；保存后落盘".into());
                    }
                });
                if self.localization_ui.enable_requested {
                    self.localization_ui.enable_requested = false;
                    self.open_capabilities();
                }
                if self.localization_ui.return_requested {
                    self.localization_ui.return_requested = false;
                    self.tab = Tab::Play;
                }
                if let Some(request) = self.localization_ui.take_navigation() {
                    self.open_localization_source(ctx, request);
                }
            }
            Tab::Manuscript => self.manuscript_tab(ctx),
            Tab::Templates => self.template_manager_tab(ctx),
            Tab::CheckpointHistory => self.checkpoint_history_tab(ctx),
        }
        self.dialogs(ctx);
        object_picker::set_workspace_snapshot(ctx, &self.project.root, self.version);
        self.source_outline_window(ctx);
        self.source_jump_window(ctx);
        self.project_search(ctx);
        self.temporal_issues_window(ctx);
        self.command_window(ctx);
        self.preferences_window(ctx);
        self.draft_exit_dialog(ctx);
        self.reading_window(ctx);
        self.navigation_drawer_window(ctx);
        self.wiki_editor_window(ctx);
        self.entity_editor_window(ctx);
        self.relation_editor_window(ctx);
        self.relation_type_editor_window(ctx);
        self.content_deletion_window(ctx);
        self.target_rename_window(ctx);
        self.source_move_window(ctx);
        self.entity_source_move_window(ctx);
        self.markdown_import_window(ctx);
        self.reader_publish_window(ctx);
        self.preset_editor_window(ctx);
        self.schema_editor_window(ctx);
        self.export_scope_dialog(ctx);
        self.play_scope_dialog(ctx);
        self.draft_rehearsal_dialog(ctx);
        self.playthrough_report_window(ctx);
        self.capability_window(ctx);
        self.capture_edit_focus(ctx);
        #[cfg(not(target_arch = "wasm32"))]
        self.show_reconciliation(ctx);
        #[cfg(not(target_arch = "wasm32"))]
        crate::chrome::resize_edges(ctx);
        #[cfg(target_arch = "wasm32")]
        crate::web::set_dirty(
            (self.browser_pending_save
                || self.project.is_dirty()
                || self.has_open_authoring_form())
                && !self.allow_close,
        );
        self.capture_personal_view(ctx);
        #[cfg(target_arch = "wasm32")]
        {
            self.personal.save_browser();
            self.catalog_workbench.save_browser_columns(ctx);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(input_active) = input_active {
            if let Some(profile) = self.frame_profile.as_mut() {
                profile.finish_frame(
                    matches!(self.tab, Tab::Map | Tab::Network),
                    input_active,
                    ctx.pixels_per_point(),
                );
            }
        }
    }
}
