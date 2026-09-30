use super::*;
impl super::super::WorldeditApp {
    pub(in crate::app) fn map_tab(&mut self, ctx: &egui::Context) {
        let (map_summaries, map_ids, map_diagnostics) = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                (
                    snapshot
                        .map_index
                        .maps
                        .values()
                        .map(|map| (map.id.clone(), map.title.clone()))
                        .collect::<Vec<_>>(),
                    snapshot.map_index.maps.keys().cloned().collect::<Vec<_>>(),
                    snapshot.map_index.diagnostics.clone(),
                )
            })
            .unwrap_or_default();
        let has_canvas_snapshot = !self.map_canvas.map_id().is_empty()
            || !self.map_canvas.snapshot.layers.is_empty()
            || !self.map_canvas.snapshot.raster_layers.is_empty();
        let mut selection_invalidated = false;
        if map_ids.is_empty() {
            let had_navigation = self.map_navigation.is_some() || has_canvas_snapshot;
            self.map_navigation = None;
            self.pending_map_camera = None;
            self.map_selection = None;
            if had_navigation {
                if self.map_navigation_blocked() {
                    self.message = Some(
                        "当前地图已失效；未提交的展示修改仍保留，请保存、重试或取消后处理。".into(),
                    );
                } else {
                    self.map_canvas.clear();
                    self.message = Some("当前没有可用地图，已清除失效的地图访问路径。".into());
                }
            }
        } else if let Some(selected) = self.map_selection.clone() {
            if !map_ids.iter().any(|map_id| map_id == &selected) {
                self.map_navigation = None;
                self.pending_map_camera = None;
                if self.map_navigation_blocked() {
                    self.map_selection = None;
                    self.message = Some(
                        "当前地图已失效；未提交的展示修改仍保留，请保存、重试或取消后选择地图。"
                            .into(),
                    );
                } else {
                    self.map_canvas.clear();
                    self.map_selection = map_ids.first().cloned();
                    selection_invalidated = true;
                    self.message =
                        Some("当前地图文档不可用，已清除失效的地图访问路径并选择可用地图。".into());
                }
            }
        } else if !self.map_navigation_blocked() {
            self.map_selection = map_ids.first().cloned();
        } else {
            self.message =
                Some("当前地图有未提交的展示修改，请保存、重试或取消后再选择地图。".into());
        }

        if let (Some(selected), Some(current)) = (
            self.map_selection.clone(),
            self.map_navigation
                .as_ref()
                .map(|navigation| navigation.current().map_id.clone())
                .or_else(|| {
                    (!self.map_canvas.map_id().is_empty()).then(|| self.map_canvas.map_id().into())
                }),
        ) {
            if selected != current && self.map_navigation_blocked() {
                self.map_selection = Some(current);
            }
        }
        let selected_map_id = self.map_selection.clone();
        let has_document = selected_map_id
            .as_ref()
            .is_some_and(|id| !selection_invalidated && map_ids.iter().any(|map_id| map_id == id));
        if let Some(map_id) = selected_map_id.as_deref().filter(|_| has_document) {
            let title = map_summaries
                .iter()
                .find(|(id, _)| id == map_id)
                .map(|(_, title)| title.as_str())
                .unwrap_or(map_id);
            self.ensure_map_navigation(map_id, title);
        }
        if let Some(map_id) = selected_map_id.as_deref().filter(|_| has_document) {
            if self.map_canvas.needs_snapshot(self.version, map_id) {
                let document = self
                    .snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.map_index.maps.get(map_id));
                if let Some(document) = document {
                    if !self
                        .map_canvas
                        .set_snapshot(self.version, render_snapshot(document))
                    {
                        self.map_selection = Some(self.map_canvas.map_id().to_owned());
                        self.message = Some(
                            "当前地图有未提交的展示修改，请保存、重试或取消后再切换地图。".into(),
                        );
                    }
                } else {
                    self.map_navigation = None;
                    self.pending_map_camera = None;
                    if !self.map_navigation_blocked() {
                        self.map_canvas.clear();
                    }
                }
            }
            if let Some(camera) = self.pending_map_camera.take() {
                self.map_canvas.restore_camera(camera);
            }
            if let Some((pending_map, visibility)) = self.pending_preset_layers.clone() {
                if pending_map == map_id && self.map_canvas.map_id() == map_id {
                    for (layer_id, visible) in visibility {
                        self.map_canvas.set_layer_visible(&layer_id, visible);
                    }
                    self.pending_preset_layers = None;
                }
            }
            self.map_canvas.prepare_rasters(ctx, &self.project.root);
            if let Some(request) = self.map_locate_request.clone() {
                if request.map_id == map_id {
                    if let Some((_, visible, _)) =
                        self.map_canvas.placement_layer(&request.placement_id)
                    {
                        if visible {
                            self.map_canvas.select_placement_id(&request.placement_id);
                            self.map_locate_request = None;
                        }
                    }
                }
            }
        } else if (!self.map_canvas.map_id().is_empty()
            || !self.map_canvas.snapshot.layers.is_empty()
            || !self.map_canvas.snapshot.raster_layers.is_empty())
            && !self.map_navigation_blocked()
        {
            self.map_canvas.clear();
        }

        let command_baseline = if self.map_canvas.is_edit_mode() {
            selected_map_id
                .as_deref()
                .and_then(|map_id| self.map_command_baseline(map_id))
        } else {
            None
        };
        self.map_canvas.set_command_baseline(command_baseline);

        let selected_placement = self.map_canvas.selected_placement();
        let selected_label = selected_placement.as_ref().and_then(|placement| {
            placement.label_override.clone().or_else(|| {
                placement.target_ref.as_ref().and_then(|target| {
                    self.snapshot
                        .as_ref()?
                        .result
                        .analysis
                        .catalog
                        .object(target)
                        .map(|object| object.display.clone())
                })
            })
        });
        let mut back_requested = false;
        let mut enter_requested = None;
        egui::SidePanel::right("map-inspector")
            .resizable(true)
            .default_width(310.0)
            .width_range(260.0..=420.0)
            .frame(crate::theme::panel())
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("map-inspector-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.map_overview_panel(ui, &map_summaries, &mut back_requested);
                        if has_document {
                            self.map_layers_panel(ui, &selected_map_id);
                            self.svg_import_panel(ui);

                            self.map_search_panel(ui);

                            self.map_markers_panel(
                                ui,
                                selected_map_id,
                                selected_placement,
                                selected_label,
                                &mut enter_requested,
                            );
                        } else if self.map_form.has_uncommitted_work() {
                            ui.separator();
                            ui.label(egui::RichText::new("保留的标记表单").strong());
                            ui.colored_label(
                        crate::theme::GOLD,
                        "当前地图文档不可用，表单输入已保留；修复地图或取消表单后才能切换。",
                    );
                            ui.label(crate::theme::muted(self.map_form.clipboard_text()));
                            ui.horizontal(|ui| {
                                if ui.small_button("复制保留输入").clicked() {
                                    ui.ctx().copy_text(self.map_form.clipboard_text());
                                    self.message = Some("已复制保留的标记表单输入".into());
                                }
                                if ui.small_button("取消当前表单").clicked() {
                                    self.cancel_map_form();
                                }
                            });
                        }

                        if !map_diagnostics.is_empty() {
                            ui.separator();
                            ui.label(egui::RichText::new("地图诊断").strong());
                            egui::ScrollArea::vertical()
                                .id_salt("map-diagnostics")
                                .show(ui, |ui| {
                                    for diagnostic in &map_diagnostics {
                                        let color = match diagnostic.severity {
                                            worldline_core::Severity::Error => crate::theme::ERROR,
                                            worldline_core::Severity::Warning => crate::theme::GOLD,
                                            worldline_core::Severity::Hint => crate::theme::ACCENT,
                                        };
                                        ui.colored_label(
                                            color,
                                            format!("[{}] {}", diagnostic.code, diagnostic.message),
                                        );
                                        ui.horizontal(|ui| {
                                            ui.label(crate::theme::muted(diagnostic.file.clone()));
                                            if ui.small_button("打开原文").clicked() {
                                                self.jump_to_file(
                                                    &diagnostic.file,
                                                    diagnostic.span.line,
                                                    diagnostic.span.column,
                                                );
                                            }
                                        });
                                    }
                                });
                        }
                    });
            });

        if back_requested {
            self.back_from_map_navigation();
        }
        if let Some(target) = enter_requested {
            self.enter_submap(target);
        }

        let mut retry_failed = false;
        let mut cancel_failed = false;
        egui::CentralPanel::default()
            .frame(crate::theme::panel().fill(crate::theme::BG))
            .show(ctx, |ui| {
                self.page_heading(ui, "地图画布", "查看和编辑注册地图、图层和标记。");
                if !self.map_canvas.map_title().is_empty() {
                    ui.label(egui::RichText::new(self.map_canvas.map_title()).strong());
                }
                self.map_canvas.toolbar(ui);
                if self.map_failed_command.is_some() {
                    ui.separator();
                    ui.colored_label(
                        crate::theme::GOLD,
                        "展示命令未提交，当前预览仍保留；重试会按当前地图版本重新检查。",
                    );
                    ui.horizontal(|ui| {
                        if self.map_canvas.is_edit_mode() {
                            if ui.button("按当前版本重试提交").clicked() {
                                retry_failed = true;
                            }
                        } else {
                            ui.label(crate::theme::muted("进入编辑展示后才能重试提交。"));
                        }
                        if ui.small_button("取消预览").clicked() {
                            cancel_failed = true;
                        }
                    });
                }
                ui.separator();
                self.map_canvas.show(ui);
            });

        if retry_failed {
            self.retry_failed_map_command();
        } else if cancel_failed {
            self.map_failed_command = None;
            self.map_canvas.reset_local_preview();
        }
        let (intents, baselines) = self.map_canvas.take_edit_batch();
        self.apply_map_intents(intents, baselines);
    }
}
