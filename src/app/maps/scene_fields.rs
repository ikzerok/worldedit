#[path = "scene_dash_fields.rs"]
mod dash_fields;

use worldline_core::vector_scene::{PathSegment, SceneGeometry, SceneStyle, TextRun};

pub(super) fn number(ui: &mut egui::Ui, label: &str, value: &mut f64) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(value)
                .speed(0.2)
                .clamp_existing_to_range(false),
        )
        .changed()
    })
    .inner
}

fn point(ui: &mut egui::Ui, label: &str, value: &mut [f64; 2]) -> bool {
    ui.push_id(label, |ui| {
        number(ui, &format!("{label} x"), &mut value[0])
            | number(ui, &format!("{label} y"), &mut value[1])
    })
    .inner
}

fn optional_text(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut Option<String>,
    default: &str,
) -> bool {
    ui.push_id(label, |ui| {
        ui.horizontal(|ui| {
            let mut explicit = value.is_some();
            let mut changed = ui
                .checkbox(&mut explicit, label)
                .on_hover_text("不勾选表示继承；none 表示显式无颜色")
                .changed();
            if changed {
                *value = explicit.then(|| default.to_owned());
            }
            if let Some(text) = value {
                changed |= ui.text_edit_singleline(text).changed();
            } else {
                ui.label(crate::theme::muted("继承"));
            }
            changed
        })
        .inner
    })
    .inner
}

fn optional_number(ui: &mut egui::Ui, label: &str, value: &mut Option<f64>, default: f64) -> bool {
    ui.push_id(label, |ui| {
        ui.horizontal(|ui| {
            let mut explicit = value.is_some();
            let mut changed = ui.checkbox(&mut explicit, label).changed();
            if changed {
                *value = explicit.then_some(default);
            }
            if let Some(number) = value {
                changed |= ui
                    .add(
                        egui::DragValue::new(number)
                            .speed(0.1)
                            .clamp_existing_to_range(false),
                    )
                    .changed();
            } else {
                ui.label(crate::theme::muted("继承"));
            }
            changed
        })
        .inner
    })
    .inner
}

pub(super) fn style_fields(ui: &mut egui::Ui, style: &mut SceneStyle) -> bool {
    let mut changed = optional_text(ui, "填充", &mut style.fill, "#357ebe");
    changed |= optional_text(ui, "描边", &mut style.stroke, "#65b4ff");
    changed |= optional_number(ui, "线宽", &mut style.stroke_width, 2.0);
    changed |= dash_fields::fields(ui, style);
    changed |= optional_number(ui, "整体透明度", &mut style.opacity, 1.0);
    changed |= optional_number(ui, "填充透明度", &mut style.fill_opacity, 1.0);
    changed |= optional_number(ui, "描边透明度", &mut style.stroke_opacity, 1.0);
    ui.horizontal(|ui| {
        ui.label("填充规则");
        changed |= ui
            .selectable_value(&mut style.fill_rule, None, "继承")
            .changed();
        changed |= ui
            .selectable_value(&mut style.fill_rule, Some("nonzero".into()), "nonzero")
            .changed();
        changed |= ui
            .selectable_value(&mut style.fill_rule, Some("evenodd".into()), "evenodd")
            .changed();
    });
    egui::CollapsingHeader::new("描边细节与文字样式").show(ui, |ui| {
        changed |= optional_text(ui, "端点", &mut style.line_cap, "butt");
        changed |= optional_text(ui, "连接", &mut style.line_join, "miter");
        changed |= optional_number(ui, "斜接上限", &mut style.miter_limit, 4.0);
        changed |= optional_number(ui, "字号", &mut style.font_size, 24.0);
        changed |= optional_text(ui, "字体", &mut style.font_family, "Noto Sans SC");
        changed |= optional_text(ui, "字重", &mut style.font_weight, "normal");
        changed |= optional_text(ui, "字形", &mut style.font_style, "normal");
        changed |= optional_text(ui, "文字锚点", &mut style.text_anchor, "start");
        ui.label(crate::theme::muted(
            "只使用内嵌字体；未知或非法样式由核心拒绝，输入保留",
        ));
    });
    changed
}

pub(super) fn geometry_fields(ui: &mut egui::Ui, geometry: &mut SceneGeometry) -> bool {
    let mut changed = false;
    match geometry {
        SceneGeometry::Group { children } => {
            ui.label(format!("组 · {} 个直接子项", children.len()));
        }
        SceneGeometry::Point { position } => changed |= point(ui, "位置", position),
        SceneGeometry::Rect {
            x,
            y,
            width,
            height,
            rx,
            ry,
        } => {
            for (label, value) in [
                ("x", x),
                ("y", y),
                ("宽", width),
                ("高", height),
                ("圆角 x", rx),
                ("圆角 y", ry),
            ] {
                changed |= number(ui, label, value);
            }
        }
        SceneGeometry::Ellipse { cx, cy, rx, ry } => {
            for (label, value) in [
                ("中心 x", cx),
                ("中心 y", cy),
                ("半径 x", rx),
                ("半径 y", ry),
            ] {
                changed |= number(ui, label, value);
            }
        }
        SceneGeometry::Polyline { points } | SceneGeometry::Polygon { points } => {
            let index = item_index(ui, "顶点", points.len());
            if let Some(value) = points.get_mut(index) {
                changed |= point(ui, "位置", value);
            }
            if ui.button("添加顶点").clicked() {
                points.push(points.last().copied().unwrap_or([0.0, 0.0]));
                changed = true;
            }
            if ui
                .add_enabled(!points.is_empty(), egui::Button::new("删除此顶点"))
                .clicked()
            {
                points.remove(index);
                changed = true;
            }
        }
        SceneGeometry::Path { segments } => {
            let index = item_index(ui, "路径段", segments.len());
            if let Some(segment) = segments.get_mut(index) {
                changed |= match segment {
                    PathSegment::Move { to } => {
                        ui.label("新子路径 Move");
                        point(ui, "起点", to)
                    }
                    PathSegment::Line { to } => {
                        ui.label("Line");
                        point(ui, "终点", to)
                    }
                    PathSegment::Cubic {
                        control1,
                        control2,
                        to,
                    } => {
                        ui.label("Cubic");
                        point(ui, "控制柄 1", control1)
                            | point(ui, "控制柄 2", control2)
                            | point(ui, "终点", to)
                    }
                    PathSegment::Quadratic { control, to } => {
                        ui.label("Quadratic");
                        point(ui, "控制柄", control) | point(ui, "终点", to)
                    }
                    PathSegment::Arc {
                        rx,
                        ry,
                        rotation,
                        large_arc,
                        sweep,
                        to,
                    } => {
                        ui.label("Arc");
                        number(ui, "半径 x", rx)
                            | number(ui, "半径 y", ry)
                            | number(ui, "旋转", rotation)
                            | ui.checkbox(large_arc, "大弧").changed()
                            | ui.checkbox(sweep, "顺时针").changed()
                            | point(ui, "终点", to)
                    }
                    PathSegment::Close => {
                        ui.label("Close · 保留闭合语义");
                        false
                    }
                };
            }
            ui.horizontal(|ui| {
                if ui.button("闭合末尾子路径").clicked()
                    && !matches!(segments.last(), Some(PathSegment::Close))
                {
                    segments.push(PathSegment::Close);
                    changed = true;
                }
                if ui
                    .add_enabled(!segments.is_empty(), egui::Button::new("删除此段"))
                    .clicked()
                {
                    segments.remove(index);
                    changed = true;
                }
            });
        }
        SceneGeometry::Text { x, y, runs } => {
            changed |= number(ui, "x", x) | number(ui, "y", y);
            let index = item_index(ui, "文字片段", runs.len());
            if let Some(run) = runs.get_mut(index) {
                changed |= ui.text_edit_multiline(&mut run.text).changed();
                changed |= optional_number(ui, "片段绝对 x", &mut run.x, 0.0);
                changed |= optional_number(ui, "片段绝对 y", &mut run.y, 0.0);
                changed |= number(ui, "相对 dx", &mut run.dx) | number(ui, "相对 dy", &mut run.dy);
                egui::CollapsingHeader::new("此文字片段样式").show(ui, |ui| {
                    changed |= style_fields(ui, &mut run.style);
                });
            }
            ui.horizontal(|ui| {
                if ui.button("添加片段").clicked() {
                    runs.push(TextRun::default());
                    changed = true;
                }
                if ui
                    .add_enabled(runs.len() > 1, egui::Button::new("删除片段"))
                    .clicked()
                {
                    runs.remove(index);
                    changed = true;
                }
            });
        }
    }
    changed
}

fn item_index(ui: &mut egui::Ui, label: &str, count: usize) -> usize {
    let id = ui.id().with(label);
    let mut index = ui
        .ctx()
        .data(|data| data.get_temp::<usize>(id))
        .unwrap_or(0)
        .min(count.saturating_sub(1));
    ui.horizontal(|ui| {
        ui.label(format!("{label} · 共 {count} 项"));
        ui.add(
            egui::DragValue::new(&mut index)
                .range(0..=count.saturating_sub(1))
                .prefix("索引 "),
        );
    });
    ui.ctx().data_mut(|data| data.insert_temp(id, index));
    index
}

/// 批量样式编辑只复制已知解释字段；未知扩展始终归原节点所有。
pub(super) fn copy_known_style(source: &SceneStyle, target: &mut SceneStyle) {
    let extra = std::mem::take(&mut target.extra);
    *target = source.clone();
    target.extra = extra;
}
