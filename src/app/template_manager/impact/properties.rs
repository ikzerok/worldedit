//! 只显示 core 预览绑定的属性快照；不从当前目录或未提交输入推断旧值。
use super::*;
use serde_json::Value;

pub(super) fn show(ui: &mut egui::Ui, change: &ProjectTemplateFieldChange) {
    ui.push_id((&change.field_id, &change.change, "properties"), |ui| {
        match (&change.old_properties, &change.new_properties) {
            (Some(old), Some(new)) => {
                if old.label != new.label {
                    pair(ui, "显示名称", brief(&old.label), brief(&new.label));
                }
                if old.key != new.key {
                    pair(
                        ui,
                        "属性 key",
                        old.key.clone().unwrap_or_else(|| "无".into()),
                        new.key.clone().unwrap_or_else(|| "无".into()),
                    );
                }
                if old.field_type != new.field_type {
                    pair(ui, "类型", type_name(old), type_name(new));
                }
                if old.required != new.required {
                    pair(ui, "必填提示", required(old), required(new));
                }
                if old.choices != new.choices {
                    pair(ui, "枚举选项", choices(old), choices(new));
                }
                if old.target != new.target {
                    pair(ui, "引用限制", target(old), target(new));
                }
                if old.default != new.default {
                    pair(ui, "默认提示", default(old), default(new));
                }
                if old == new {
                    ui.label(theme::muted("字段属性未变，只有位置变化。"));
                }
            }
            (None, Some(new)) => {
                ui.label("新增字段属性");
                side(ui, new);
            }
            (Some(old), None) => {
                ui.label("删除字段原属性（实例值不删除）");
                side(ui, old);
            }
            (None, None) => {}
        }
        ui.horizontal_wrapped(|ui| {
            if let Some(old) = &change.old_properties {
                if ui.small_button("复制完整旧属性").clicked() {
                    if let Ok(text) = serde_json::to_string_pretty(old) {
                        ui.ctx().copy_text(text);
                    }
                }
            }
            if let Some(new) = &change.new_properties {
                if ui.small_button("复制完整新属性").clicked() {
                    if let Ok(text) = serde_json::to_string_pretty(new) {
                        ui.ctx().copy_text(text);
                    }
                }
            }
        });
    });
}
fn pair(ui: &mut egui::Ui, label: &str, old: String, new: String) {
    ui.label(format!("{label}：{old} → {new}"));
}
fn side(ui: &mut egui::Ui, properties: &ProjectTemplateFieldProperties) {
    ui.label(format!(
        "显示名称：{}；类型：{}",
        brief(&properties.label),
        type_name(properties)
    ));
    if properties.field_type == ProjectTemplateFieldType::Group {
        return;
    }
    ui.label(format!(
        "属性 key：{}；必填提示：{}；默认提示：{}",
        properties.key.as_deref().unwrap_or("无"),
        required(properties),
        default(properties)
    ));
    if properties.field_type == ProjectTemplateFieldType::Enum {
        ui.label(format!("枚举选项：{}", choices(properties)));
    }
    if properties.field_type == ProjectTemplateFieldType::ObjectRef {
        ui.label(format!("引用限制：{}", target(properties)));
    }
}
fn brief(text: &str) -> String {
    if text.len() <= 240 {
        format!("{text:?}")
    } else {
        format!(
            "{:?}…（共 {} UTF-8 字节；可复制完整属性）",
            text.chars().take(120).collect::<String>(),
            text.len()
        )
    }
}
fn type_name(properties: &ProjectTemplateFieldProperties) -> String {
    design::type_label(properties.field_type.as_str()).into()
}
fn required(properties: &ProjectTemplateFieldProperties) -> String {
    if properties.required {
        "开启"
    } else {
        "关闭"
    }
    .into()
}
fn default(properties: &ProjectTemplateFieldProperties) -> String {
    match &properties.default {
        None => "未设置".into(),
        Some(Value::String(text)) => brief(text),
        Some(value) => value.to_string(),
    }
}
fn choices(properties: &ProjectTemplateFieldProperties) -> String {
    let shown = properties
        .choices
        .iter()
        .take(12)
        .map(|text| brief(text))
        .collect::<Vec<_>>()
        .join("、");
    if properties.choices.len() > 12 {
        format!(
            "[{shown}, …]（共 {} 项；可复制完整属性）",
            properties.choices.len()
        )
    } else {
        format!("[{shown}]")
    }
}
fn target(properties: &ProjectTemplateFieldProperties) -> String {
    properties
        .target
        .as_ref()
        .map(|target| {
            format!(
                "{} / {}",
                target.kind,
                target
                    .entity_type
                    .as_deref()
                    .map(brief)
                    .unwrap_or_else(|| "全部".into())
            )
        })
        .unwrap_or_else(|| "无".into())
}
