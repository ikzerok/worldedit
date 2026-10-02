use super::*;
impl super::super::WorldeditApp {
    pub(in crate::app) fn map_tab(&mut self, ctx: &egui::Context) {
        self.poll_scene_operations();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape))
            && !self.map_canvas.measurement_active()
            && self.map_canvas.measurement.calibration.is_none()
        {
            self.map_form.text_draft = None;
        }
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
                    } else {
                        self.map_canvas.sync_scene(document.scene.as_ref());
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
                        } else {
                            self.map_canvas.panel = MapPanel::Layers;
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
            if let MapGeometry::Text { text, .. } = &placement.geometry {
                return Some(text.clone());
            }
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
        // 画布选择改变时将详情带回视野，不能只把它放在长滚动列表的逻辑顶部。
        let selected_identity = selected_placement
            .as_ref()
            .map(|placement| (selected_map_id.clone(), placement.id.clone()))
            .or_else(|| {
                self.map_canvas
                    .scene
                    .inspector
                    .as_ref()
                    .map(|node| (selected_map_id.clone(), node.id.clone()))
            });
        let selection_changed = ctx.data_mut(|data| {
            let id = egui::Id::new("map-inspector-selection");
            let previous = data.get_temp::<Option<(Option<String>, String)>>(id);
            data.insert_temp(id, selected_identity.clone());
            previous.as_ref() != Some(&selected_identity)
        });
        if selection_changed && self.map_canvas.scene.inspector.is_some() {
            self.map_canvas.panel = MapPanel::Inspector;
        }
        let mut back_requested = false;
        let mut enter_requested = None;
        // 面板宽度包括边距；用主导航之后的空间与上次实际拖拽宽度预算画布。
        let panel_width = |name, fallback| {
            egui::containers::panel::PanelState::load(ctx, egui::Id::new(name))
                .map_or(fallback, |state| state.size().x)
        };
        let inspector_id = egui::Id::new("map-inspector-visible");
        let index_id = egui::Id::new("map-index-visible");
        let needs_inspector = self.map_creation.open
            || self.map_canvas.svg_import.open
            || self.map_form.has_uncommitted_work()
            || self.map_canvas.scene.inspector_dirty
            || self.map_locate_request.is_some();
        if selection_changed {
            ctx.data_mut(|data| data.remove::<bool>(inspector_id));
        }
        let mut inspector_visible = needs_inspector
            || ctx
                .data(|data| data.get_temp::<bool>(inspector_id))
                .unwrap_or(
                    selected_identity.is_some()
                        || self.map_canvas.is_edit_mode()
                        || !has_document
                        || !map_diagnostics.is_empty(),
                );
        let canvas_margin = crate::theme::panel().total_margin().sum().x;
        let inspector_width = if inspector_visible {
            panel_width("map-inspector", crate::theme::INSPECTOR_WIDTH).clamp(260.0, 420.0)
        } else {
            0.0
        };
        let font_scale =
            ctx.style().text_styles[&egui::TextStyle::Body].size / crate::theme::BODY_SIZE;
        let compact = ctx.available_rect().width()
            - panel_width("map-index", crate::theme::INDEX_WIDTH)
            - inspector_width
            - canvas_margin
            < 600.0 * font_scale.max(1.0);
        let mut index_visible = ctx
            .data(|data| data.get_temp::<bool>(index_id))
            .unwrap_or(!compact);
        egui::TopBottomPanel::top("map-layout-controls")
            .frame(crate::theme::panel())
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong("地图画布");
                    let index_label = if index_visible {
                        "收起地图目录"
                    } else {
                        "显示地图目录"
                    };
                    if ui.button(index_label).clicked() {
                        index_visible = !index_visible;
                        ctx.data_mut(|data| data.insert_temp(index_id, index_visible));
                    }
                    let label = if inspector_visible {
                        "收起地图面板"
                    } else {
                        "显示地图面板"
                    };
                    if ui
                        .add_enabled(!needs_inspector, egui::Button::new(label))
                        .on_disabled_hover_text("请先完成或取消当前表单")
                        .clicked()
                    {
                        inspector_visible = !inspector_visible;
                        ctx.data_mut(|data| data.insert_temp(inspector_id, inspector_visible));
                    }
                    if !self.map_canvas.map_title().is_empty() {
                        ui.add(egui::Label::new(self.map_canvas.map_title()).truncate())
                            .on_hover_text(self.map_canvas.map_title());
                    }
                });
            });
        if index_visible {
            egui::SidePanel::left("map-index")
                .default_width(crate::theme::INDEX_WIDTH)
                .resizable(true)
                .frame(crate::theme::index_panel())
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("map-index-scroll")
                        .show(ui, |ui| {
                            self.map_overview_panel(ui, &map_summaries, &mut back_requested);
                            self.map_search_panel(ui);
                        });
                });
        }
        if inspector_visible {
            egui::SidePanel::right("map-inspector")
                .resizable(true)
                .default_width(crate::theme::INSPECTOR_WIDTH)
                .width_range(260.0..=420.0)
                .frame(crate::theme::panel())
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut self.map_canvas.panel,
                            MapPanel::Inspector,
                            "当前选择",
                        );
                        ui.selectable_value(
                            &mut self.map_canvas.panel,
                            MapPanel::Layers,
                            "图层/对象",
                        );
                    });
                    let mut scroll = egui::ScrollArea::vertical()
                        .id_salt("map-inspector-scroll")
                        .auto_shrink([false, false]);
                    if selection_changed {
                        scroll = scroll.vertical_scroll_offset(0.0);
                    }
                    scroll.show(ui, |ui| {
                        if self.map_canvas.calibration_active() {
                            ui.disable();
                        }
                        if self.map_canvas.panel == MapPanel::Layers {
                            self.map_layers_panel(ui, &selected_map_id);
                            self.map_canvas.scene_tree_panel(ui);
                            return;
                        }
                        let scene_active = self.scene_inspector(ui);
                        let text_active = !scene_active
                            && has_document
                            && self.map_text_panel(
                                ui,
                                selected_map_id.as_deref(),
                                selected_placement.as_ref(),
                            );
                        if has_document && !text_active && !scene_active {
                            self.map_selected_marker_panel(
                                ui,
                                selected_map_id.clone(),
                                selected_placement.clone(),
                                selected_label,
                                &mut enter_requested,
                            );
                        }
                        if !index_visible {
                            self.map_overview_panel(ui, &map_summaries, &mut back_requested);
                        }
                        if has_document {
                            // 打开的导入表单不能被折叠或藏在其他长表单之后。
                            self.svg_import_panel(ui);
                            if !text_active && !scene_active {
                                self.map_markers_panel(
                                    ui,
                                    selected_map_id.clone(),
                                    selected_placement,
                                );
                            }
                            if !index_visible {
                                self.map_search_panel(ui);
                            }
                        } else if self.map_form.has_uncommitted_work() {
                            ui.separator();
                            ui.label(egui::RichText::new("保留的标记表单").strong());
                            ui.colored_label(
                        crate::theme::GOLD(),
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
                                            worldline_core::Severity::Error => {
                                                crate::theme::ERROR()
                                            }
                                            worldline_core::Severity::Warning => {
                                                crate::theme::GOLD()
                                            }
                                            worldline_core::Severity::Hint => {
                                                crate::theme::ACCENT()
                                            }
                                        };
                                        ui.colored_label(
                                            color,
                                            format!("[{}] {}", diagnostic.code, diagnostic.message),
                                        );
                                        crate::theme::source_caption(
                                            ui,
                                            &self.project.root,
                                            std::path::Path::new(&diagnostic.file),
                                        );
                                        if ui.small_button("打开原文").clicked() {
                                            self.jump_to_file(
                                                &diagnostic.file,
                                                diagnostic.span.line,
                                                diagnostic.span.column,
                                            );
                                        }
                                    }
                                });
                        }
                    });
                });
        }
        if back_requested {
            self.back_from_map_navigation();
        }
        if let Some(target) = enter_requested {
            self.enter_submap(target);
        }

        let mut retry_failed = false;
        let mut cancel_failed = false;
        egui::CentralPanel::default()
            .frame(crate::theme::panel().fill(crate::theme::BG()))
            .show(ctx, |ui| {
                if has_document || self.map_canvas.has_uncommitted_work() {
                    self.map_canvas.measurement_blocked = self.map_form.has_uncommitted_work()
                        || self.map_failed_command.is_some()
                        || self.map_creation.open;
                    self.map_canvas.toolbar(ui);
                    self.map_canvas.scene.render_status_panel(ui);
                    self.scene_job_panel(ui);
                    ui.menu_button("导出矢量…", |ui| {
                        if ui.button("导出整图矢量 SVG…").clicked() {
                            ui.close();
                            self.begin_svg_export(ctx, false);
                        }
                        let selected = !self.map_canvas.scene.selection.is_empty()
                            || self.map_canvas.selected_placement().is_some();
                        if ui
                            .add_enabled(selected, egui::Button::new("导出当前选择…"))
                            .clicked()
                        {
                            self.begin_svg_export(ctx, true);
                            ui.close();
                        }
                    });
                } else {
                    ui.heading("开始绘制你的世界");
                    ui.label("先创建空白地图，再添加图层、地点或导入矢量图形。");
                    if ui.add(crate::theme::primary("创建第一张地图")).clicked() {
                        self.map_canvas.set_mode(CanvasMode::Edit);
                        self.map_creation
                            .open_with_defaults(self.map_revision, self.map_manifest_baseline());
                    }
                }
                if self.map_failed_command.is_some() {
                    ui.separator();
                    ui.colored_label(
                        crate::theme::GOLD(),
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
                self.map_canvas.form_blocked =
                    self.map_form.text_draft.is_some() || self.map_form.pending_place.is_some();
                self.map_canvas.legacy_place_tool = self.map_form.create_place_on_next_point;
                self.map_canvas.show(ui);
            });

        if self.map_canvas.measurement.calibration.is_none()
            && self
                .map_failed_command
                .as_ref()
                .is_some_and(|pending| matches!(pending.intent, EditIntent::SetMeasurement(_)))
        {
            self.map_failed_command = None;
        }
        if retry_failed {
            self.retry_failed_map_command();
        } else if cancel_failed {
            self.map_failed_command = None;
            self.map_canvas.reset_local_preview();
        }
        let (intents, baselines) = self.map_canvas.take_edit_batch();
        self.apply_map_intents(intents, baselines);
        self.scene_review_window(ctx);
        self.svg_import_window(ctx);
        self.scene_place_window(ctx);
        self.scene_export_window(ctx);
        self.submit_scene_operations(ctx);
    }
}
