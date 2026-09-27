use super::*;
impl super::super::WorldeditApp {
    pub(super) fn map_markers_panel(
        &mut self,
        ui: &mut egui::Ui,
        selected_map_id: Option<String>,
        selected_placement: Option<MapPlacement>,
        selected_label: Option<String>,
        enter_requested: &mut Option<navigation::MapNavigationDto>,
    ) {
        if self.map_form.pending_place.is_some() && !self.map_canvas.is_edit_mode() {
            ui.separator();
            ui.colored_label(crate::theme::GOLD, "地点落点已保留；返回编辑展示后可提交");
            if ui.small_button("取消落点").clicked() {
                self.cancel_map_form();
            }
        }
        if self.map_canvas.is_edit_mode() {
            ui.separator();
            ui.label(egui::RichText::new("标记编辑").strong());
            if self.map_form.pending_place.is_none()
                && ui
                    .checkbox(
                        &mut self.map_form.create_place_on_next_point,
                        "下一次点落位：新建地点资料并放置入口",
                    )
                    .changed()
                && self.map_form.create_place_on_next_point
            {
                self.map_canvas.set_tool(CanvasTool::Point);
            }
            if let Some(pending) = self.map_form.pending_place.as_ref() {
                ui.colored_label(crate::theme::GOLD, "落点已保留，尚未写入资料或标记");
                ui.label(format!(
                    "地图 {} · 图层 {} · 资料 {} · 入口 {}",
                    pending.map_id, pending.layer_id, pending.entity_id, pending.placement_id
                ));
                if let MapGeometry::Point(point) = &pending.geometry {
                    ui.label(format!("落点：{:.3}, {:.3}", point.x, point.y));
                }
                ui.label("地点名称");
                ui.text_edit_singleline(&mut self.map_form.place_name);
                ui.label("地点说明");
                ui.text_edit_multiline(&mut self.map_form.place_description);
                ui.label("入口说明（可选）");
                ui.text_edit_singleline(&mut self.map_form.annotation);
                ui.horizontal(|ui| {
                    if ui.button("新建地点并放置入口").clicked() {
                        self.commit_pending_place();
                    }
                    if ui.small_button("取消落点").clicked() {
                        self.cancel_map_form();
                    }
                });
                ui.separator();
            }
            ui.label(crate::theme::muted(
                "选择已有对象可保持引用；留空则创建说明标记。",
            ));
            ui.text_edit_singleline(&mut self.map_form.target_query);
            let target_query = self.map_form.target_query.trim().to_lowercase();
            if !target_query.is_empty() {
                let candidates = self
                    .snapshot
                    .as_ref()
                    .map(|snapshot| {
                        snapshot
                            .result
                            .analysis
                            .catalog
                            .objects
                            .iter()
                            .filter(|object| {
                                object.display.to_lowercase().contains(&target_query)
                                    || object.target.id.to_lowercase().contains(&target_query)
                            })
                            .take(8)
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                for object in candidates {
                    if ui
                        .small_button(format!(
                            "{}  ·  {}:{}",
                            object.display, object.target.kind, object.target.id
                        ))
                        .clicked()
                    {
                        self.map_form.target = Some(object.target);
                        self.map_form.target_query.clear();
                    }
                }
            }
            if let Some(target) = self.map_form.target.clone() {
                ui.horizontal(|ui| {
                    ui.label(format!("已选：{}:{}", target.kind, target.id));
                    if ui.small_button("清除引用").clicked() {
                        self.map_form.target = None;
                    }
                });
            }
            ui.label("说明");
            ui.text_edit_singleline(&mut self.map_form.annotation);
            ui.label("role");
            ui.text_edit_singleline(&mut self.map_form.role);
            ui.label("自定义标签（可选）");
            ui.text_edit_singleline(&mut self.map_form.label_override);
            if self.map_form.has_uncommitted_work() && ui.small_button("取消当前表单").clicked()
            {
                self.cancel_map_form();
            }
            if let Some(selected) = selected_placement.as_ref() {
                if ui.small_button("载入当前标记到表单").clicked() {
                    self.map_form.editing_placement = Some(selected.id.clone());
                    self.map_form.target = selected.target_ref.clone();
                    self.map_form.annotation = selected.annotation.clone();
                    self.map_form.role = selected.role.clone();
                    self.map_form.label_override =
                        selected.label_override.clone().unwrap_or_default();
                }
            }
            if let Some(placement_id) = self.map_form.editing_placement.clone() {
                if ui.button("保存当前标记说明").clicked() {
                    let applied = self.apply_map_command(
                        selected_map_id.as_deref().unwrap_or_default(),
                        worldline_core::presentation_commands::Command::UpdatePlacement {
                            map_id: selected_map_id.clone().unwrap_or_default(),
                            placement_id: placement_id.clone(),
                            geometry: None,
                            target_ref: Some(self.map_form.target.clone()),
                            annotation: Some(self.map_form.annotation.clone()),
                            role: Some(self.map_form.role.clone()),
                            label_override: Some(
                                (!self.map_form.label_override.trim().is_empty())
                                    .then(|| self.map_form.label_override.clone()),
                            ),
                            layer_id: None,
                        },
                        "已保存标记说明",
                    );
                    if applied {
                        self.map_form.clear();
                    }
                }
                if ui.button("删除标记（资料仍保留）").clicked() {
                    let _ = self.apply_map_command(
                        selected_map_id.as_deref().unwrap_or_default(),
                        worldline_core::presentation_commands::Command::DeletePlacement {
                            map_id: selected_map_id.clone().unwrap_or_default(),
                            placement_id,
                        },
                        "已删除地图标记，资料仍保留",
                    );
                    self.map_form.editing_placement = None;
                }
            }
        }

        if !self.map_canvas.raster_states().is_empty() {
            ui.separator();
            ui.label(egui::RichText::new("栅格图层").strong());
            for (asset, available, error) in self.map_canvas.raster_states() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(&asset);
                    if available && error.is_none() {
                        ui.colored_label(crate::theme::ACCENT, "已加载");
                    } else {
                        ui.colored_label(crate::theme::GOLD, "不可用");
                    }
                });
                let unavailable = !available || error.is_some();
                if let Some(error) = error.as_deref() {
                    ui.label(crate::theme::muted(error));
                }
                if unavailable && ui.small_button("打开地图文档修复引用").clicked() {
                    if let Some(map_id) = selected_map_id.as_deref() {
                        match worldline_core::presentation_commands::map_document_path(
                            &self.project,
                            map_id,
                        ) {
                            Ok(path) => {
                                let file = path.to_string_lossy().into_owned();
                                self.jump_to_file(&file, 1, 1);
                            }
                            Err(error) => self.io_error = Some(error.to_string()),
                        }
                    }
                }
            }
        }

        ui.separator();
        ui.label(egui::RichText::new("标记信息").strong());
        if let Some(placement) = selected_placement.as_ref() {
            let display = selected_label
                .clone()
                .unwrap_or_else(|| placement.id.clone());
            ui.label(egui::RichText::new(display).strong());
            ui.label(crate::theme::muted(format!(
                "标记 ID：{} · {}",
                placement.id, placement.role
            )));
            if !placement.annotation.is_empty() {
                ui.label(&placement.annotation);
            }
            if let Some(map_id) = selected_map_id.clone() {
                if ui.small_button("批注此标记…").clicked() {
                    self.new_comment_for_anchor(
                        worldline_core::collaboration::CommentAnchor::MapPlacement {
                            map_id,
                            placement_id: placement.id.clone(),
                        },
                    );
                }
            }
            if let Some(target) = placement.target_ref.clone() {
                ui.label(crate::theme::muted(format!(
                    "对象：{} · {}",
                    target.kind, target.id
                )));
                let target_resolved = self.snapshot.as_ref().is_some_and(|snapshot| {
                    snapshot.result.analysis.catalog.object(&target).is_some()
                });
                if !target_resolved {
                    ui.colored_label(crate::theme::GOLD, "对象引用未解析");
                    if ui.small_button("编辑展示并重绑定").clicked() {
                        self.map_canvas.set_mode(CanvasMode::Edit);
                        self.map_form.editing_placement = Some(placement.id.clone());
                        self.map_form.target = None;
                        self.map_form.target_query.clear();
                        self.map_form.annotation = placement.annotation.clone();
                        self.map_form.role = placement.role.clone();
                        self.map_form.label_override =
                            placement.label_override.clone().unwrap_or_default();
                    }
                }
                if ui.small_button("打开资料").clicked() {
                    self.open_reading(target);
                }
            }
            if let Some(target) = placement.navigation.clone() {
                let target_in_index = self
                    .snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.map_index.maps.contains_key(&target.map_id));
                if target.available && target_in_index {
                    if ui.small_button("进入子地图").clicked() {
                        *enter_requested = Some(target);
                    }
                } else {
                    ui.colored_label(
                        crate::theme::GOLD,
                        if target.available {
                            "目标地图文档不可用，请修复原文"
                        } else {
                            "目标地图未注册或不可用"
                        },
                    );
                    if ui.small_button("打开目标地图原文修复").clicked() {
                        match worldline_core::presentation_commands::map_document_path(
                            &self.project,
                            &target.map_id,
                        ) {
                            Ok(path) => {
                                let file = path.to_string_lossy().into_owned();
                                self.jump_to_file(&file, 1, 1);
                            }
                            Err(error) => self.io_error = Some(error.to_string()),
                        }
                    }
                }
            }
        } else {
            ui.label(crate::theme::muted("点击画布上的标记查看信息。"));
        }
    }
}
