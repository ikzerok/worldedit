impl super::super::WorldeditApp {
    pub(super) fn map_layers_panel(&mut self, ui: &mut egui::Ui, selected_map_id: &Option<String>) {
        ui.separator();
        ui.label(egui::RichText::new("图层").strong());
        let editing = self.map_canvas.is_edit_mode();
        ui.label(crate::theme::muted(if editing {
            "编辑展示：显隐默认会写入展示文档；锁定和顺序也会保存。"
        } else {
            "浏览模式：显隐只作用于本次浏览；进入编辑展示后才能保存图层设置。"
        }));
        let layer_details = self.map_canvas.layer_details();
        for (index, (id, title, visible, locked, count)) in layer_details.iter().enumerate() {
            let mut next = if editing {
                self.map_canvas
                    .layer_default_visibility(id)
                    .unwrap_or(*visible)
            } else {
                *visible
            };
            ui.horizontal(|ui| {
                if ui
                    .checkbox(
                        &mut next,
                        if editing {
                            format!("{}  ·  默认可见 · {} 个标记", title, count)
                        } else {
                            format!("{}  ·  临时显示 · {} 个标记", title, count)
                        },
                    )
                    .changed()
                {
                    if editing {
                        let applied = self.apply_map_command(
                            selected_map_id.as_deref().unwrap_or_default(),
                            worldline_core::presentation_commands::Command::SetLayer {
                                map_id: selected_map_id.clone().unwrap_or_default(),
                                layer_id: id.clone(),
                                title: None,
                                visible_default: Some(next),
                                locked: None,
                                layer_order: None,
                            },
                            "已保存图层默认显隐",
                        );
                        if applied {
                            self.map_canvas.set_layer_default_visible(id, next);
                        }
                    } else {
                        self.map_canvas.set_layer_visible(id, next);
                    }
                }
                if editing {
                    let lock_label = if *locked { "🔒" } else { "🔓" };
                    if ui
                        .small_button(lock_label)
                        .on_hover_text(if *locked {
                            "解锁图层"
                        } else {
                            "锁定图层"
                        })
                        .clicked()
                    {
                        let _ = self.apply_map_command(
                            selected_map_id.as_deref().unwrap_or_default(),
                            worldline_core::presentation_commands::Command::SetLayer {
                                map_id: selected_map_id.clone().unwrap_or_default(),
                                layer_id: id.clone(),
                                title: None,
                                visible_default: None,
                                locked: Some(!locked),
                                layer_order: None,
                            },
                            if *locked {
                                "已解锁图层"
                            } else {
                                "已锁定图层"
                            },
                        );
                    }
                    if index > 0 && ui.small_button("↑").on_hover_text("上移图层").clicked() {
                        let mut order = self.map_canvas.layer_order();
                        order.swap(index, index - 1);
                        let _ = self.apply_map_command(
                            selected_map_id.as_deref().unwrap_or_default(),
                            worldline_core::presentation_commands::Command::SetLayer {
                                map_id: selected_map_id.clone().unwrap_or_default(),
                                layer_id: id.clone(),
                                title: None,
                                visible_default: None,
                                locked: None,
                                layer_order: Some(order),
                            },
                            "已调整图层顺序",
                        );
                    }
                    if index + 1 < layer_details.len()
                        && ui.small_button("↓").on_hover_text("下移图层").clicked()
                    {
                        let mut order = self.map_canvas.layer_order();
                        order.swap(index, index + 1);
                        let _ = self.apply_map_command(
                            selected_map_id.as_deref().unwrap_or_default(),
                            worldline_core::presentation_commands::Command::SetLayer {
                                map_id: selected_map_id.clone().unwrap_or_default(),
                                layer_id: id.clone(),
                                title: None,
                                visible_default: None,
                                locked: None,
                                layer_order: Some(order),
                            },
                            "已调整图层顺序",
                        );
                    }
                }
            });
        }

        if let Some(request) = self.map_locate_request.clone() {
            if request.map_id == selected_map_id.as_deref().unwrap_or_default() {
                ui.separator();
                ui.colored_label(crate::theme::GOLD, "命中对象位于隐藏图层");
                ui.label(crate::theme::muted(format!(
                    "图层 `{}` 当前隐藏。是否临时显示以定位？",
                    request.layer_id
                )));
                ui.horizontal(|ui| {
                    if ui.button("临时显示并定位").clicked() {
                        self.map_canvas.reveal_layer_for_session(&request.layer_id);
                        self.map_canvas.select_placement_id(&request.placement_id);
                        self.map_locate_request = None;
                    }
                    if ui.small_button("取消").clicked() {
                        self.map_locate_request = None;
                    }
                });
            }
        }
    }
}
