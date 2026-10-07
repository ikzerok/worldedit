use worldline_core::vector_scene::{SceneStyle, MAX_DASH_ENTRIES};

pub(super) fn fields(ui: &mut egui::Ui, style: &mut SceneStyle) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.label("虚线");
        changed |= ui
            .selectable_value(&mut style.stroke_dasharray, None, "继承")
            .changed();
        changed |= ui
            .selectable_value(&mut style.stroke_dasharray, Some(Vec::new()), "实线")
            .changed();
        changed |= ui
            .selectable_value(
                &mut style.stroke_dasharray,
                Some(vec![15.0, 12.0]),
                "航道 15/12",
            )
            .changed();
        changed |= ui
            .selectable_value(
                &mut style.stroke_dasharray,
                Some(vec![6.0, 4.0]),
                "短虚线 6/4",
            )
            .changed();
    });
    if let Some(array) = &mut style.stroke_dasharray {
        let mut remove = None;
        ui.horizontal_wrapped(|ui| {
            ui.label("自定义数列（user unit / px）");
            for (index, value) in array.iter_mut().enumerate() {
                ui.push_id(("dash", index), |ui| {
                    changed |= ui
                        .add(
                            egui::DragValue::new(value)
                                .speed(0.2)
                                .clamp_existing_to_range(false),
                        )
                        .on_hover_text(if index % 2 == 0 {
                            "实段长度；允许零，不能为负"
                        } else {
                            "空段长度；允许零，不能为负"
                        })
                        .changed();
                    if ui.small_button("×").on_hover_text("移除此数值").clicked() {
                        remove = Some(index);
                    }
                });
            }
            if crate::theme::add_enabled(
                ui,
                array.len() < MAX_DASH_ENTRIES,
                egui::Button::new("＋数值"),
            )
            .clicked()
            {
                array.push(6.0);
                changed = true;
            }
        });
        if let Some(index) = remove {
            array.remove(index);
            changed = true;
        }
        if array.len() % 2 == 1 {
            ui.label(crate::theme::muted(
                "奇数项按完整数列重复一次；不改写原数列",
            ));
        }
        if !array.is_empty() && array.iter().all(|value| *value == 0.0) {
            ui.label(crate::theme::muted("全零数列按实线绘制"));
        }
    }
    changed |= super::optional_number(ui, "虚线偏移", &mut style.stroke_dashoffset, 0.0);
    ui.label(crate::theme::muted(
        "长度和偏移随图形变换；不支持百分比。文字片段需显式实线；超预算会保留输入并拒绝应用",
    ));
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untouched_dash_controls_do_not_upgrade_legacy_styles_or_change_unknown_fields() {
        let mut style = SceneStyle::default();
        style
            .extra
            .insert("future-style".into(), serde_json::json!({"owned":true}));
        let original = style.clone();
        let context = egui::Context::default();
        let _ = context.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!fields(ui, &mut style));
            });
        });
        assert_eq!(style, original);
    }

    #[test]
    fn copy_known_dash_style_keeps_each_targets_unknown_fields() {
        let source = SceneStyle {
            stroke_dasharray: Some(vec![15.0, 12.0, 3.0]),
            stroke_dashoffset: Some(-7.0),
            extra: serde_json::from_value(serde_json::json!({"source-only":true})).unwrap(),
            ..SceneStyle::default()
        };
        let mut target = SceneStyle {
            extra: serde_json::from_value(serde_json::json!({"target-only":{"v":4}})).unwrap(),
            ..SceneStyle::default()
        };
        let extra = target.extra.clone();
        super::super::copy_known_style(&source, &mut target);
        assert_eq!(target.stroke_dasharray, source.stroke_dasharray);
        assert_eq!(target.stroke_dashoffset, source.stroke_dashoffset);
        assert_eq!(target.extra, extra);
    }
}
