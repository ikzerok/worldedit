use super::*;
use worldline_core::presentation_commands::Command;

#[derive(Clone)]
pub(in crate::app) struct TextDraft {
    pub map_id: String,
    pub layer_id: String,
    pub placement_id: Option<String>,
    pub position: NormalizedPoint,
    pub text: String,
    pub font_size: f32,
    pub color: String,
    pub baseline: Option<MapCommandBaseline>,
}

impl super::super::WorldeditApp {
    pub(super) fn map_text_panel(
        &mut self,
        ui: &mut egui::Ui,
        map_id: Option<&str>,
        selected: Option<&MapPlacement>,
    ) -> bool {
        let selected_text = selected.filter(|p| matches!(p.geometry, MapGeometry::Text { .. }));
        if !self.map_canvas.is_edit_mode() {
            if self.map_form.text_draft.is_some() {
                ui.label("文字草稿已保留，返回编辑展示后可保存");
                if ui.button("取消文字草稿").clicked() {
                    self.map_form.text_draft = None;
                }
                return true;
            }
            return false;
        }
        if let Some(selected) = selected_text {
            let unlocked = self
                .map_canvas
                .placement_layer(&selected.id)
                .is_some_and(|(_, _, locked)| !locked);
            if self.map_form.text_draft.is_none()
                && crate::theme::add_enabled(ui, unlocked, egui::Button::new("编辑文字标签"))
                    .on_disabled_hover_text("图层已锁定，请先解锁图层")
                    .clicked()
            {
                if self.map_form.has_uncommitted_work() {
                    self.io_error = Some("请先保存或取消当前表单".into());
                } else if let MapGeometry::Text {
                    position,
                    text,
                    font_size,
                    color,
                } = &selected.geometry
                {
                    let map_id = map_id.unwrap_or_default();
                    self.map_form.text_draft = Some(TextDraft {
                        map_id: map_id.into(),
                        layer_id: String::new(),
                        placement_id: Some(selected.id.clone()),
                        position: *position,
                        text: text.clone(),
                        font_size: *font_size,
                        color: color.clone(),
                        baseline: self.map_command_baseline(map_id),
                    });
                }
            }
        }
        let Some(draft) = self.map_form.text_draft.as_mut() else {
            if self.map_canvas.tool == CanvasTool::Text {
                ui.label("在地图上点按文字的左上角落点");
                ui.label(crate::theme::muted("独立文字不创建地点资料"));
                return true;
            }
            return selected_text.is_some();
        };
        ui.separator();
        ui.strong(if draft.placement_id.is_some() {
            "编辑文字标签"
        } else {
            "新建文字标签"
        });
        ui.label("纯文本 · 最多160字 / 4行");
        ui.add(
            egui::TextEdit::multiline(&mut draft.text)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text("例如：北境山脉"),
        );
        ui.horizontal(|ui| {
            ui.label("字号");
            ui.add(
                egui::DragValue::new(&mut draft.font_size)
                    .range(12.0..=64.0)
                    .speed(1.0),
            );
            ui.label("地图单位");
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("文字颜色");
            for (name, hex) in [
                ("浅白", "#e8eef8"),
                ("墨色", "#243447"),
                ("琥珀", "#f1c56c"),
                ("青蓝", "#71c6d1"),
            ] {
                ui.selectable_value(&mut draft.color, hex.into(), name);
            }
        });
        let valid = worldline_core::presentation::valid_map_text(
            &draft.text,
            draft.font_size as f64,
            &draft.color,
        );
        if !valid {
            ui.colored_label(
                crate::theme::GOLD(),
                "请输入非空文字；仅支持普通换行，不支持制表符",
            );
        }
        let mut save = false;
        let mut cancel = false;
        let mut delete = false;
        ui.horizontal_wrapped(|ui| {
            save =
                crate::theme::add_enabled(ui, valid, egui::Button::new("保存文字标签")).clicked();
            cancel = ui.button("取消文字草稿").clicked();
            if draft.placement_id.is_some() {
                delete = ui.button("删除文字标签").clicked();
            }
        });
        if save {
            self.commit_map_text(false);
        } else if delete {
            self.commit_map_text(true);
        } else if cancel {
            self.map_form.text_draft = None;
        }
        true
    }

    pub(super) fn commit_map_text(&mut self, delete: bool) -> bool {
        let Some(draft) = self.map_form.text_draft.clone() else {
            return false;
        };
        if self.map_selection.as_deref() != Some(&draft.map_id) {
            self.io_error = Some("地图已切换，请返回原地图处理文字草稿".into());
            return false;
        }
        let geometry = core_geometry(&MapGeometry::Text {
            position: draft.position,
            text: draft.text,
            font_size: draft.font_size,
            color: draft.color,
        });
        let command = match draft.placement_id {
            Some(placement_id) if delete => Command::DeletePlacement {
                map_id: draft.map_id.clone(),
                placement_id,
            },
            Some(placement_id) => Command::UpdatePlacement {
                map_id: draft.map_id.clone(),
                placement_id,
                geometry: Some(geometry),
                target_ref: None,
                annotation: None,
                role: None,
                label_override: None,
                layer_id: None,
            },
            None => Command::CreatePlacement {
                map_id: draft.map_id.clone(),
                placement_id: self.next_map_placement_id(&draft.map_id),
                layer_id: draft.layer_id,
                target_ref: None,
                geometry,
                annotation: "文字标签".into(),
                role: "文字标签".into(),
                label_override: None,
            },
        };
        if self.apply_map_command_with_baseline(
            &draft.map_id,
            command,
            "已保存文字标签修改（可撤销）",
            draft.baseline,
        ) {
            self.map_form.text_draft = None;
            true
        } else {
            false
        }
    }
}

pub(super) fn text_layout(
    painter: &egui::Painter,
    camera: &Camera2D,
    text: &str,
    font_size: f32,
    color: &str,
) -> std::sync::Arc<egui::Galley> {
    let color = u32::from_str_radix(color.trim_start_matches('#'), 16).unwrap_or(0xe8eef8);
    painter.layout_no_wrap(
        text.into(),
        egui::FontId::proportional((font_size * camera.zoom()).clamp(1.0, 4096.0)),
        Color32::from_rgb((color >> 16) as u8, (color >> 8) as u8, color as u8),
    )
}

pub(super) fn paint_text(
    painter: &egui::Painter,
    camera: &Camera2D,
    viewport: Rect,
    position: NormalizedPoint,
    text: &str,
    font_size: f32,
    color: &str,
) {
    let origin = camera.normalized_to_screen(position.as_pos2(), viewport);
    let galley = text_layout(painter, camera, text, font_size, color);
    painter.galley(origin, galley, Color32::WHITE);
}
