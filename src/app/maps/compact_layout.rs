//! 窄矮窗口复用原动作到渐进菜单，始终给地图留下真实剩余空间。
impl crate::app::WorldeditApp {
    pub(super) fn compact_map_controls(
        &mut self,
        ui: &mut egui::Ui,
        index: &mut bool,
        inspector: &mut bool,
        needs_inspector: bool,
    ) {
        ui.horizontal(|ui| {
            let label = if self.map_canvas.scene.job.is_some() {
                "地图操作 · 任务"
            } else if self.map_canvas.measurement_active()
                || self.map_canvas.measurement.calibration.is_some()
            {
                "地图操作 · 测距"
            } else {
                "地图操作"
            };
            let button = ui.button(label);
            let was_open =
                egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&button));
            // Scene 状态使根菜单上移并覆盖开按钮时，hover 子菜单不能把
            // 打开根的同一次 click 当作外部关闭；后续帧恢复正常关闭。
            let opening_frame = button.clicked() && !was_open;
            egui::Popup::menu(&button)
                .close_behavior(if opening_frame {
                    egui::PopupCloseBehavior::IgnoreClicks
                } else {
                    egui::PopupCloseBehavior::CloseOnClickOutside
                })
                .show(|ui| {
                    ui.set_max_width((ui.ctx().screen_rect().width() - 40.0).clamp(220.0, 360.0));
                    egui::ScrollArea::vertical()
                        .id_salt("compact-map-actions")
                        .max_height((ui.ctx().screen_rect().height() - 48.0).max(96.0))
                        .show(ui, |ui| {
                            #[cfg(test)]
                            ui.ctx().data_mut(|data| {
                                data.insert_temp(
                                    egui::Id::new("catalog-scope-map-actions-clip"),
                                    ui.clip_rect(),
                                )
                            });
                            if ui
                                .button(if *index {
                                    "收起地图目录"
                                } else {
                                    "显示地图目录"
                                })
                                .clicked()
                            {
                                *index = !*index;
                                ui.ctx().data_mut(|data| {
                                    data.insert_temp(egui::Id::new("map-index-visible"), *index)
                                });
                                ui.close();
                            }
                            if crate::theme::add_enabled(
                                ui,
                                !needs_inspector,
                                egui::Button::new(if *inspector {
                                    "收起地图面板"
                                } else {
                                    "显示地图面板"
                                }),
                            )
                            .on_disabled_hover_text("请先完成或取消当前表单")
                            .clicked()
                            {
                                *inspector = !*inspector;
                                ui.ctx().data_mut(|data| {
                                    data.insert_temp(
                                        egui::Id::new("map-inspector-visible"),
                                        *inspector,
                                    )
                                });
                                ui.close();
                            }
                            ui.separator();
                            self.map_controls_and_export(ui);
                        });
                });
            // 关闭此菜单的 Esc 不得继续取消画布中的校准/绘制草稿。
            if was_open || opening_frame {
                ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            }
            ui.add(egui::Label::new(self.map_canvas.map_title()).truncate())
                .on_hover_text(self.map_canvas.map_title());
        });
    }
    pub(super) fn map_controls_and_export(&mut self, ui: &mut egui::Ui) {
        self.map_canvas.toolbar(ui);
        self.map_canvas.scene.render_status_panel(ui);
        self.scene_job_panel(ui);
        ui.menu_button("导出矢量…", |ui| {
            if ui.button("导出整图矢量 SVG…").clicked() {
                ui.close();
                self.begin_svg_export(ui.ctx(), false);
            }
            let selected = !self.map_canvas.scene.selection.is_empty()
                || self.map_canvas.selected_placement().is_some();
            if crate::theme::add_enabled(ui, selected, egui::Button::new("导出当前选择…")).clicked()
            {
                ui.close();
                self.begin_svg_export(ui.ctx(), true);
            }
        });
    }
}
