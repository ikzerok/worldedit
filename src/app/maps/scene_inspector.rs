use worldline_core::vector_scene::{SceneNavigation, SceneOp};

impl super::super::WorldeditApp {
    pub(super) fn scene_inspector(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(mut node) = self.map_canvas.scene.inspector.take() else {
            return false;
        };
        let is_new = self
            .map_canvas
            .scene
            .source
            .as_ref()
            .is_none_or(|scene| !scene.nodes.contains_key(&node.id));
        ui.heading(if is_new {
            "新建矢量对象"
        } else {
            "当前矢量选择"
        });
        crate::theme::technical_value(ui, "稳定 ID", &node.id);
        if let Some(target) = &node.target_ref {
            ui.label(format!("绑定：{}:{}", target.kind, target.id));
            if ui.button("打开资料").clicked() {
                self.open_reading(target.clone());
            }
        }
        if !is_new && ui.button("批注此图元…").clicked() {
            self.new_comment_for_anchor(
                worldline_core::collaboration::CommentAnchor::MapPlacement {
                    map_id: self.map_canvas.map_id().into(),
                    placement_id: node.id.clone(),
                },
            );
        }
        if !is_new
            && self.map_canvas.is_edit_mode()
            && ui
                .add_enabled(
                    !self.map_canvas.scene.inspector_dirty,
                    egui::Button::new("新建地点并绑定…"),
                )
                .clicked()
        {
            self.open_scene_place(node.id.clone());
        }
        if let Some(navigation) = &node.navigation {
            if let Some(map) = self
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.map_index.maps.get(&navigation.map_id))
            {
                if ui.button(format!("进入子地图：{}", map.title)).clicked() {
                    self.enter_submap(super::navigation::MapNavigationDto {
                        map_id: map.id.clone(),
                        available: true,
                    });
                }
            } else {
                ui.colored_label(crate::theme::WARNING(), "子地图不可用；引用保留，可修复");
            }
        }
        let mut changed = false;
        let mut apply = false;
        let mut cancel = false;
        let editable = self.map_canvas.is_edit_mode() && self.map_canvas.scene.job.is_none();
        ui.add_enabled_ui(editable, |ui| {
            ui.label("显示名称");
            changed |= ui.text_edit_singleline(&mut node.name).changed();
            ui.label("展示说明");
            changed |= ui.text_edit_multiline(&mut node.annotation).changed();
            ui.label("用途");
            changed |= ui.text_edit_singleline(&mut node.role).changed();
            egui::CollapsingHeader::new("几何与文字")
                .default_open(true)
                .show(ui, |ui| {
                    changed |= super::scene_fields::geometry_fields(ui, &mut node.geometry);
                });
            egui::CollapsingHeader::new("样式")
                .default_open(true)
                .show(ui, |ui| {
                    changed |= super::scene_fields::style_fields(ui, &mut node.style);
                    if self.map_canvas.scene.selection.len() > 1
                        && ui.button("样式应用到全部选择").clicked()
                    {
                        let operations = self
                            .map_canvas
                            .scene
                            .selection
                            .iter()
                            .filter_map(|id| {
                                let mut copy = self
                                    .map_canvas
                                    .scene
                                    .source
                                    .as_ref()?
                                    .nodes
                                    .get(id)?
                                    .clone();
                                super::scene_fields::copy_known_style(&node.style, &mut copy.style);
                                Some(SceneOp::Update { node: copy })
                            })
                            .collect();
                        self.map_canvas.scene_queue(operations);
                    }
                });
            egui::CollapsingHeader::new("Affine 与层").show(ui, |ui| {
                for (index, label) in ["a", "b", "c", "d", "e", "f"].iter().enumerate() {
                    changed |= super::scene_fields::number(ui, label, &mut node.transform.0[index]);
                }
                if let Some(clip) = node.clip_rect {
                    ui.label(format!("根 viewport 裁剪：{clip:?}"));
                }
                ui.label("移到图层（保持世界位置）");
                for layer in &self.map_canvas.snapshot.layers {
                    if layer.id != node.layer_id
                        && ui
                            .add_enabled(!layer.locked, egui::Button::new(&layer.title))
                            .clicked()
                    {
                        self.map_canvas.scene.operations.push(SceneOp::MoveToLayer {
                            node_ids: self.map_canvas.scene.selection.iter().cloned().collect(),
                            layer_id: layer.id.clone(),
                        });
                    }
                }
            });
            egui::CollapsingHeader::new("对象绑定与导航").show(ui, |ui| {
                changed |= self.scene_binding_fields(ui, &mut node);
                let mut destination = node
                    .navigation
                    .as_ref()
                    .map(|navigation| navigation.map_id.clone())
                    .unwrap_or_default();
                ui.label("子地图 ID（留空解除导航）");
                if ui.text_edit_singleline(&mut destination).changed() {
                    node.navigation = (!destination.trim().is_empty()).then(|| SceneNavigation {
                        map_id: destination,
                        extra: Default::default(),
                    });
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                apply = ui
                    .add_enabled(
                        self.map_canvas.scene.inspector_dirty || changed,
                        crate::theme::primary("应用对象修改"),
                    )
                    .clicked();
                cancel = ui.button("取消检查器输入").clicked();
            });
        });
        self.map_canvas.scene.inspector_dirty |= changed;
        if changed && self.map_canvas.scene.intent_baseline.is_none() {
            self.map_canvas.scene.intent_baseline = self.map_canvas.command_baseline.clone();
        }
        if apply {
            let operation = if is_new {
                SceneOp::Insert {
                    node: node.clone(),
                    index: None,
                }
            } else {
                SceneOp::Update { node: node.clone() }
            };
            self.map_canvas.scene_queue(vec![operation]);
        }
        if cancel {
            self.map_canvas.scene.inspector_dirty = false;
            self.map_canvas.scene.intent_baseline = None;
            self.map_canvas.scene.inspector = self
                .map_canvas
                .scene
                .source
                .as_ref()
                .and_then(|scene| scene.nodes.get(&node.id))
                .cloned();
        } else {
            self.map_canvas.scene.inspector = Some(node);
        }
        true
    }

    fn scene_binding_fields(
        &mut self,
        ui: &mut egui::Ui,
        node: &mut worldline_core::vector_scene::SceneNode,
    ) -> bool {
        let mut changed = false;
        ui.label("查找对象名称或 ID");
        ui.text_edit_singleline(&mut self.map_canvas.scene.binding_query);
        let query = self.map_canvas.scene.binding_query.trim().to_lowercase();
        if !query.is_empty() {
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
                            object.display.to_lowercase().contains(&query)
                                || object.target.id.to_lowercase().contains(&query)
                        })
                        .take(12)
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            for object in candidates {
                if ui
                    .button(format!(
                        "{} · {}:{}",
                        object.display, object.target.kind, object.target.id
                    ))
                    .clicked()
                {
                    node.target_ref = Some(object.target);
                    self.map_canvas.scene.binding_query.clear();
                    changed = true;
                }
            }
        }
        if node.target_ref.is_some() && ui.button("解除绑定（保留资料）").clicked() {
            node.target_ref = None;
            changed = true;
        }
        changed
    }
}
