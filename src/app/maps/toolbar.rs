use super::*;
use worldline_core::vector_scene::SceneOp;

impl MapCanvas {
    pub(in crate::app) fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(self.mode == CanvasMode::Browse, "浏览")
                .clicked()
            {
                self.set_mode(CanvasMode::Browse);
            }
            if ui
                .selectable_label(self.mode == CanvasMode::Edit, "编辑展示")
                .clicked()
            {
                self.set_mode(CanvasMode::Edit);
            }
            ui.separator();
            if ui.small_button("适配全图").clicked() && self.viewport.is_positive() {
                self.camera.fit(self.viewport);
            }
            if ui.small_button("重置镜头").clicked() {
                self.camera.reset();
            }
            ui.label(format!("{:.0}%", self.camera.zoom() * 100.0));
            if !self.measurement_active() && self.measurement.calibration.is_none() {
                ui.menu_button("测距与校准", |ui| self.measurement_toolbar(ui));
            }
        });
        if self.measurement_active() || self.measurement.calibration.is_some() {
            self.measurement_toolbar(ui);
        }
        if self.mode == CanvasMode::Edit && !self.measurement_active() {
            if self.scene.source.is_none() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(crate::theme::muted("旧标记工具保留；矢量曲线需显式启用"));
                    if ui.button("启用矢量工具（可撤销）").clicked() {
                        self.scene_queue(vec![SceneOp::EnableScene]);
                    }
                });
            }
            ui.add_enabled_ui(
                self.scene.job.is_none() && self.scene.review_plan.is_none(),
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (tool, label) in [
                            (CanvasTool::Select, "选择"),
                            (CanvasTool::Nodes, "节点"),
                            (CanvasTool::Pan, "平移"),
                            (CanvasTool::Point, "点"),
                            (CanvasTool::Polyline, "线"),
                            (CanvasTool::Polygon, "区"),
                            (CanvasTool::Rectangle, "矩形"),
                            (CanvasTool::Ellipse, "椭圆"),
                            (CanvasTool::Bezier, "曲线"),
                            (CanvasTool::Text, "文字"),
                        ] {
                            let supported = self.scene.source.is_some()
                                || !matches!(
                                    tool,
                                    CanvasTool::Bezier
                                        | CanvasTool::Rectangle
                                        | CanvasTool::Ellipse
                                );
                            if ui
                                .add_enabled(
                                    supported,
                                    egui::Button::new(label).selected(self.tool == tool),
                                )
                                .clicked()
                            {
                                if !self.scene.path.is_empty() {
                                    self.scene.error =
                                        Some("请先完成或取消当前路径，再切换工具".into());
                                } else {
                                    self.set_tool(tool);
                                }
                            }
                        }
                    });
                    if self.scene.source.is_some() {
                        ui.horizontal_wrapped(|ui| {
                            ui.menu_button("新图形样式", |ui| {
                                super::scene_fields::style_fields(ui, &mut self.scene.style);
                            });
                            if self.tool == CanvasTool::Bezier {
                                ui.checkbox(&mut self.scene.close_path, "闭合并填充");
                                if ui
                                    .add_enabled(
                                        !self.scene.path.is_empty(),
                                        egui::Button::new("添加子路径 · Alt"),
                                    )
                                    .clicked()
                                {
                                    self.scene.new_subpath = true;
                                }
                            }
                            if !self.scene.path.is_empty() {
                                if ui.button("完成路径 · Enter").clicked() {
                                    self.scene_finish_path();
                                }
                                if ui.button("取消路径 · Esc").clicked() {
                                    self.scene.path.clear();
                                    self.scene.gesture = None;
                                    self.scene.outgoing = None;
                                    self.scene.new_subpath = false;
                                    self.scene.intent_baseline = None;
                                }
                            }
                        });
                        self.scene_selection_tools(ui);
                    } else {
                        ui.label(crate::theme::muted(
                            "线/区双击完成，Esc 取消；节点模式编辑旧标记顶点",
                        ));
                    }
                },
            );
            if self.draft.is_some() && ui.small_button("取消旧标记草稿").clicked() {
                self.reset_local_preview();
            }
        }
        if let Some(error) = self.validation_error() {
            ui.colored_label(crate::theme::ERROR(), error);
        }
    }
}
