//! 单色尺子工具、短校准表单与画布端点，均为本地界面状态。
use super::measurement::distance_label;
use super::*;

impl MapCanvas {
    pub(super) fn measurement_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(crate::theme::muted("测距"));
            let enabled = self.snapshot.measurement.is_some()
                && self.measurement.calibration.is_none()
                && !(self.is_edit_mode() && self.svg_import.open);
            if ui
                .add_enabled(
                    enabled,
                    egui::Button::new("尺子 · 只读").selected(self.measurement.ruler),
                )
                .on_disabled_hover_text("先在编辑展示中完成两点校准；当前校准须先确认或取消")
                .clicked()
            {
                self.set_ruler(!self.measurement.ruler);
            }
            if self.is_edit_mode()
                && self.measurement.calibration.is_none()
                && ui
                    .add_enabled(
                        !self.has_uncommitted_work() && !self.measurement_blocked,
                        egui::Button::new(if self.snapshot.measurement.is_some() {
                            "重新校准"
                        } else {
                            "两点校准"
                        }),
                    )
                    .on_disabled_hover_text("请先完成或取消当前绘制和表单")
                    .clicked()
            {
                self.begin_calibration();
            }
            if let Some(value) = &self.snapshot.measurement {
                ui.label(crate::theme::muted(format!(
                    "已校准 · 参照线 {}",
                    distance_label(value.distance, &value.unit)
                )));
            } else {
                ui.label(crate::theme::muted("尚未校准 · 进入编辑展示设置比例"));
            }
        });
        if self.measurement.calibration.is_some() {
            self.calibration_form(ui);
        }
        if self.measurement.ruler && !self.calibration_active() {
            ui.horizontal_wrapped(|ui| {
                match self.ruler_distance() {
                    Ok(distance) => {
                        let unit = &self.snapshot.measurement.as_ref().unwrap().unit;
                        ui.strong(format!("平面距离：{}", distance_label(distance, unit)));
                        ui.label(crate::theme::muted("再次点按开始新测量"));
                    }
                    Err(message) if self.measurement.ruler_points.len() == 2 => {
                        ui.colored_label(crate::theme::GOLD(), message);
                    }
                    Err(_) => {
                        ui.label(if self.measurement.ruler_points.is_empty() {
                            "点选起点 A"
                        } else {
                            "点选终点 B"
                        });
                    }
                }
                if ui.small_button("清除测量点").clicked() {
                    self.measurement.ruler_points.clear();
                }
                if ui.small_button("退出尺子").clicked() {
                    self.set_ruler(false);
                }
            });
            ui.label(crate::theme::muted(
                "临时测量不保存 · 拖动平移 / 滚轮缩放 / Esc 退出",
            ));
        }
    }

    fn calibration_form(&mut self, ui: &mut egui::Ui) {
        let editing = self.is_edit_mode();
        ui.separator();
        ui.strong(if editing {
            "两点校准 · 确认前不会写入地图"
        } else {
            "校准草稿已保留 · 返回编辑展示后可继续"
        });
        let draft = self.measurement.calibration.as_mut().unwrap();
        ui.add_enabled_ui(editing, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(match draft.points.len() {
                    0 => "在地图内点选参照点 A",
                    1 => "在地图内点选参照点 B",
                    _ => "参照点 A、B 已选定",
                });
                if ui.small_button("重选两点").clicked() {
                    draft.points.clear();
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("已知距离");
                ui.add(
                    egui::TextEdit::singleline(&mut draft.distance)
                        .desired_width(120.0)
                        .hint_text("例如 12.5"),
                );
                ui.label("单位");
                ui.add(
                    egui::TextEdit::singleline(&mut draft.unit)
                        .desired_width(95.0)
                        .hint_text("例如 千米"),
                );
            });
        });
        ui.label(crate::theme::muted(
            "使用地图逻辑比例；不推断单位或换算。读者站不公开校准与尺子。",
        ));
        let validation = self.calibration_value();
        let unchanged =
            validation.as_ref().ok() == self.snapshot.measurement.as_ref() && validation.is_ok();
        if unchanged {
            ui.label(crate::theme::muted("校准未改变，无需保存"));
        }
        if editing {
            if let Err(error) = &validation {
                ui.colored_label(crate::theme::GOLD(), error);
            }
        }
        let mut confirm = false;
        let mut cancel = false;
        ui.horizontal_wrapped(|ui| {
            confirm = ui
                .add_enabled(
                    editing && validation.is_ok() && !unchanged,
                    egui::Button::new("确认保存校准"),
                )
                .on_hover_text("提交一个可撤销的展示命令；保存工程后保留校准")
                .clicked();
            cancel = ui.button("取消校准").clicked();
            ui.label(crate::theme::muted("Esc 取消"));
        });
        if cancel {
            self.cancel_measurement();
        } else if confirm {
            self.confirm_calibration();
        }
        if let Some(error) = &self.measurement.error {
            ui.colored_label(crate::theme::ERROR(), error);
        }
    }

    pub(super) fn draw_measurement(&self, painter: &egui::Painter, viewport: Rect) {
        let (points, calibration) = if self.calibration_active() {
            (&self.measurement.calibration.as_ref().unwrap().points, true)
        } else if self.measurement.ruler {
            (&self.measurement.ruler_points, false)
        } else {
            return;
        };
        let points: Vec<_> = points
            .iter()
            .map(|point| {
                self.camera
                    .normalized_to_screen(Pos2::new(point[0] as f32, point[1] as f32), viewport)
            })
            .collect();
        let color = Color32::from_gray(242);
        if points.len() == 2 {
            painter.line_segment(
                [points[0], points[1]],
                egui::Stroke::new(5.0_f32, Color32::from_black_alpha(190)),
            );
            painter.line_segment([points[0], points[1]], egui::Stroke::new(2.0_f32, color));
            if !calibration {
                if let Ok(distance) = self.ruler_distance() {
                    let label =
                        distance_label(distance, &self.snapshot.measurement.as_ref().unwrap().unit);
                    let center = points[0].lerp(points[1], 0.5) + egui::vec2(0.0, -16.0);
                    let galley =
                        painter.layout_no_wrap(label, egui::FontId::proportional(14.0), color);
                    let rect =
                        Rect::from_center_size(center, galley.size() + egui::vec2(12.0, 8.0));
                    painter.rect_filled(rect, 3.0, Color32::from_black_alpha(220));
                    painter.galley(rect.min + egui::vec2(6.0, 4.0), galley, color);
                }
            }
        }
        for (index, point) in points.into_iter().enumerate() {
            painter.circle_filled(point, 6.0, Color32::from_black_alpha(220));
            painter.circle_stroke(point, 4.0, egui::Stroke::new(2.0_f32, color));
            painter.text(
                point + egui::vec2(9.0, 9.0),
                egui::Align2::LEFT_TOP,
                if index == 0 { "A" } else { "B" },
                egui::FontId::proportional(14.0),
                color,
            );
        }
    }
}
