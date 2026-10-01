//! 通用属性表单；引用的能力门和候选身份均来自 core。
use worldline_core::ast::PropertyValue;
use worldline_core::catalog::{is_object_reference_kind, Catalog, TargetRef};
use worldline_core::CompileOptions;

#[cfg(test)]
pub(in crate::app) fn properties(ui: &mut egui::Ui, values: &mut Vec<(String, PropertyValue)>) {
    render(ui, values, None);
}

pub(in crate::app) fn properties_with_references(
    ui: &mut egui::Ui,
    values: &mut Vec<(String, PropertyValue)>,
    catalog: &Catalog,
    options: CompileOptions,
) {
    render(ui, values, Some((catalog, options)));
}

fn render(
    ui: &mut egui::Ui,
    values: &mut Vec<(String, PropertyValue)>,
    references: Option<(&Catalog, CompileOptions)>,
) {
    let mut remove = None;
    for (i, (name, value)) in values.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.label(super::super::reading::property_label(name));
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    // 先分配真实删除按钮，再用剩余空间放输入；不假设主题的按钮/字体尺寸。
                    if ui.small_button("×").clicked() {
                        remove = Some(i);
                    }
                    ui.add(
                        egui::TextEdit::singleline(name)
                            .hint_text("属性 ID")
                            .desired_width(ui.available_width()),
                    );
                },
            );
            ui.horizontal(|ui| {
                let current = match value {
                    PropertyValue::Str(_) => 0,
                    PropertyValue::Num(_) => 1,
                    PropertyValue::Bool(_) => 2,
                    PropertyValue::Ref(target) => match target.kind.as_str() {
                        "character" => 5,
                        "relation" => 4,
                        _ => 3,
                    },
                };
                let mut kind = current;
                if current >= 3 && references.is_none() {
                    ui.label("对象引用");
                } else {
                    egui::ComboBox::from_id_salt("type")
                        // 六个类型选项在生产主题的32px行高下仍可直接到达。
                        .height(320.0)
                        .width(65.0)
                        .selected_text(
                            ["文本", "数值", "布尔", "实体引用", "关系引用", "人物引用"][kind],
                        )
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut kind, 0, "文本");
                            ui.selectable_value(&mut kind, 1, "数值");
                            ui.selectable_value(&mut kind, 2, "布尔");
                            if let Some((_, options)) = references {
                                for (index, target, label) in [
                                    (3, "entity", "实体引用"),
                                    (4, "relation", "关系引用"),
                                    (5, "character", "人物引用"),
                                ] {
                                    if is_object_reference_kind(target, options) {
                                        ui.selectable_value(&mut kind, index, label);
                                    }
                                }
                            }
                        });
                }
                if kind != current {
                    *value = match kind {
                        1 => PropertyValue::Num(0.0),
                        2 => PropertyValue::Bool(false),
                        3..=5 => PropertyValue::Ref(TargetRef::new(
                            ["entity", "relation", "character"][kind - 3],
                            "",
                        )),
                        _ => PropertyValue::Str(String::new()),
                    };
                }
            });
            // 值独占纵向行，picker 的可用宽度不再反向撑大父 Window。
            match value {
                PropertyValue::Str(text) => {
                    ui.add(
                        egui::TextEdit::multiline(text)
                            .desired_rows(3)
                            .desired_width(ui.available_width())
                            .hint_text("属性值"),
                    );
                }
                PropertyValue::Num(number) => {
                    ui.add(egui::DragValue::new(number).speed(1.0));
                }
                PropertyValue::Bool(value) => {
                    ui.checkbox(value, "是 / 否");
                }
                PropertyValue::Ref(target) => {
                    if let Some((catalog, _)) = references
                        .filter(|(_, options)| is_object_reference_kind(&target.kind, *options))
                    {
                        let mut selected = (!target.id.is_empty()).then(|| target.clone());
                        super::super::object_picker::object_picker(
                            ui,
                            ("property-reference", i),
                            "目标",
                            &mut selected,
                            catalog,
                            &[&target.kind],
                        );
                        target.id = selected.map(|value| value.id).unwrap_or_default();
                    } else {
                        ui.label(format!("{}:{}（当前能力只读保留）", target.kind, target.id));
                    }
                }
            }
            ui.add_space(6.0);
        });
    }
    if let Some(index) = remove {
        values.remove(index);
    }
    if ui.button("＋ 添加属性").clicked() {
        values.push((String::new(), PropertyValue::Str(String::new())));
    }
}

#[cfg(test)]
mod tests;
