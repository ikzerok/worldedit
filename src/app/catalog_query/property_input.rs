use super::{CatalogQuery, FilterInputs, ScalarInputKind, TargetRef};
use std::path::Path;
use worldline_core::catalog::OBJECT_REFERENCE_TARGET_KINDS;
use worldline_core::queries::{CatalogQueryFilter, PropertyCondition, PropertyScalar};

pub(super) fn render(
    ui: &mut egui::Ui,
    values: &mut Vec<PropertyCondition>,
    inputs: &mut FilterInputs,
    root: &Path,
) {
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::TextEdit::singleline(&mut inputs.property_key).hint_text("属性键"));
        egui::ComboBox::from_id_salt("query-property-scalar")
            .selected_text(kind_label(inputs.property_kind))
            .show_ui(ui, |ui| {
                for kind in [
                    ScalarInputKind::String,
                    ScalarInputKind::Number,
                    ScalarInputKind::Boolean,
                    ScalarInputKind::Reference,
                ] {
                    ui.selectable_value(&mut inputs.property_kind, kind, kind_label(kind));
                }
            });
        match inputs.property_kind {
            ScalarInputKind::String | ScalarInputKind::Number => {
                ui.add(egui::TextEdit::singleline(&mut inputs.property_value).hint_text("精确值"));
                if inputs.property_kind == ScalarInputKind::String
                    && inputs.property_value.is_empty()
                {
                    ui.label("空字符串（可显式添加）");
                }
            }
            ScalarInputKind::Boolean => {
                ui.checkbox(&mut inputs.property_bool, "值为真");
            }
            ScalarInputKind::Reference => {
                egui::ComboBox::from_id_salt("query-property-reference-kind")
                    .selected_text(
                        inputs
                            .property_reference_kind
                            .as_deref()
                            .unwrap_or("entity"),
                    )
                    .show_ui(ui, |ui| {
                        for kind in OBJECT_REFERENCE_TARGET_KINDS {
                            ui.selectable_value(
                                &mut inputs.property_reference_kind,
                                Some((*kind).into()),
                                *kind,
                            );
                        }
                    });
                ui.add(
                    egui::TextEdit::singleline(&mut inputs.property_value)
                        .hint_text("完整目标 ID（允许不存在）"),
                );
            }
        }
        let condition = condition_from_inputs(inputs, root);
        if crate::theme::add_enabled(ui, condition.is_ok(), egui::Button::new("添加属性值"))
            .clicked()
        {
            if let Ok(condition) = condition {
                if !values.contains(&condition) {
                    values.push(condition);
                }
                inputs.property_key.clear();
                inputs.property_value.clear();
            }
        }
    });
    if inputs.property_kind == ScalarInputKind::Reference {
        ui.label("添加对象引用条件将显式启用查询 v3；按 kind 与 ID 精确相等，不匹配显示名或别名。");
    }
    if !inputs.property_key.trim().is_empty() {
        if let Err(error) = condition_from_inputs(inputs, root) {
            ui.colored_label(crate::theme::ERROR(), error);
        }
    }
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (index, condition) in values.iter().enumerate() {
            if ui
                .small_button(format!(
                    "× {} = {}",
                    condition.key,
                    value_label(&condition.equals)
                ))
                .clicked()
            {
                remove = Some(index);
            }
        }
    });
    if let Some(index) = remove {
        values.remove(index);
    }
}

fn kind_label(kind: ScalarInputKind) -> &'static str {
    match kind {
        ScalarInputKind::String => "文字",
        ScalarInputKind::Number => "数字",
        ScalarInputKind::Boolean => "布尔值",
        ScalarInputKind::Reference => "对象引用（需查询 v3）",
    }
}

pub(super) fn condition_from_inputs(
    inputs: &FilterInputs,
    root: &Path,
) -> Result<PropertyCondition, String> {
    let equals = match inputs.property_kind {
        ScalarInputKind::String => PropertyScalar::String(inputs.property_value.clone()),
        ScalarInputKind::Number => inputs
            .property_value
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .map(PropertyScalar::Number)
            .ok_or("请输入有限数字；空输入不代表 0")?,
        ScalarInputKind::Boolean => PropertyScalar::Boolean(inputs.property_bool),
        ScalarInputKind::Reference => PropertyScalar::Reference(TargetRef::new(
            inputs
                .property_reference_kind
                .as_deref()
                .unwrap_or("entity"),
            &inputs.property_value,
        )),
    };
    let condition = PropertyCondition {
        key: inputs.property_key.clone(),
        equals,
    };
    // 使用正式 DTO 的验证，不在 UI 复制语言身份规则或要求目标已经存在。
    let mut candidate = CatalogQuery {
        filters: vec![CatalogQueryFilter::Property {
            values: vec![condition.clone()],
            negate: false,
        }],
        ..Default::default()
    };
    candidate.sync_edited_version();
    candidate
        .validate(root)
        .map_err(|error| error.to_string())?;
    Ok(condition)
}

pub(super) fn value_label(value: &PropertyScalar) -> String {
    match value {
        PropertyScalar::String(value) if value.is_empty() => "空字符串".into(),
        PropertyScalar::String(value) => serde_json::to_string(value).unwrap_or_default(),
        PropertyScalar::Number(value) => value.to_string(),
        PropertyScalar::Boolean(value) => if *value { "真" } else { "假" }.into(),
        PropertyScalar::Reference(target) => format!("对象引用 {}:{}", target.kind, target.id),
    }
}
