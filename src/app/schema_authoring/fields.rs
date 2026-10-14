//! 只选择 core 给出的控件类型和候选；不复制 required、enum 或 ref 判定。
use super::Session;
use crate::theme;
use worldline_core::ast::PropertyValue;
use worldline_core::schemas::{SchemaAuthoringField, SchemaAuthoringSource, SchemaType};
pub(super) fn render(
    ui: &mut egui::Ui,
    session: &mut Session,
    writable: bool,
    actions: bool,
) -> (Option<SchemaAuthoringSource>, Option<egui::Response>) {
    let mut source = None;
    let mut focused = None;
    let mut changed = false;
    for field in &session.projection.fields {
        ui.push_id((&session.base.target, &field.id), |ui| {
            let response = ui
                .vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        remember_focus(&mut focused, ui.strong(&field.key));
                        remember_focus(
                            &mut focused,
                            ui.label(theme::muted(type_label(&field.value_type))),
                        );
                        if field.required {
                            remember_focus(&mut focused, ui.label("必填"));
                        }
                        let present = session.values.get(&field.id).is_some_and(Option::is_some);
                        remember_focus(
                            &mut focused,
                            ui.label(if present { "已填写" } else { "未填写" }),
                        );
                    });
                    let value = session.values.entry(field.id.clone()).or_default();
                    let before = value.clone();
                    ui.add_enabled_ui(writable, |ui| {
                        let mut needs_reference = false;
                        if let Some(current) = value.as_mut() {
                            if let Some(response) = control(
                                ui,
                                field,
                                current,
                                input_id(&session.base.target, &field.id),
                            ) {
                                remember_focus(&mut focused, response);
                            } else {
                                remember_focus(
                                    &mut focused,
                                    ui.label(format!("保留原值：{}", value_summary(current))),
                                );
                                remember_focus(
                                    &mut focused,
                                    ui.label(theme::muted("原值不会自动转换。可明确替换或移除。")),
                                );
                                if let Some(initial) = initial_value(&field.value_type) {
                                    if remember_focus(
                                        &mut focused,
                                        crate::theme::add_enabled(
                                            ui,
                                            actions,
                                            egui::Button::new("替换为此约束类型的值"),
                                        ),
                                    )
                                    .clicked()
                                    {
                                        *current = initial;
                                    }
                                } else {
                                    needs_reference = true;
                                }
                            }
                            if remember_focus(
                                &mut focused,
                                crate::theme::add_enabled(
                                    ui,
                                    actions,
                                    egui::Button::new("移除值（设为未填写）").small(),
                                ),
                            )
                            .clicked()
                            {
                                *value = None;
                            }
                        } else if let Some(initial) = initial_value(&field.value_type) {
                            if remember_focus(
                                &mut focused,
                                crate::theme::add_enabled(
                                    ui,
                                    actions,
                                    egui::Button::new("添加此字段"),
                                ),
                            )
                            .clicked()
                            {
                                *value = Some(initial);
                            }
                        } else {
                            needs_reference = true;
                        }
                        if needs_reference {
                            ui.add_enabled_ui(actions, |ui| {
                                remember_focus(&mut focused, reference_choices(ui, field, value));
                            });
                        }
                    });
                    changed |= *value != before;
                    for diagnostic in &field.diagnostics {
                        remember_focus(
                            &mut focused,
                            ui.colored_label(
                                if diagnostic.severity == worldline_core::Severity::Error {
                                    theme::ERROR()
                                } else {
                                    theme::WARNING()
                                },
                                format!(
                                    "{} · {} · {}",
                                    diagnostic.severity.label(),
                                    diagnostic.code,
                                    diagnostic.message
                                ),
                            ),
                        );
                    }
                    let header = egui::CollapsingHeader::new("字段身份与来源").show(ui, |ui| {
                        remember_focus(
                            &mut focused,
                            ui.label(theme::muted(format!("稳定字段 ID · {}", field.id))),
                        );
                        if remember_focus(&mut focused, ui.link("查看约束声明")).clicked() {
                            source = Some(field.constraint_source.clone());
                        }
                        if let Some(property) = &field.property_source {
                            if remember_focus(&mut focused, ui.link("查看现值来源")).clicked()
                            {
                                source = Some(property.clone());
                            }
                        }
                        remember_focus(
                            &mut focused,
                            ui.label(theme::muted(format!(
                                "{}:{}",
                                field.constraint_source.file, field.constraint_source.line
                            ))),
                        );
                    });
                    remember_focus(&mut focused, header.header_response);
                    ui.add_space(8.0);
                    ui.separator();
                })
                .response;
            if session.scroll_field.as_ref() == Some(&field.id) {
                response.scroll_to_me(Some(egui::Align::Center));
                session.scroll_field = None;
            }
        });
    }
    if changed {
        session.invalidate_preview();
    }
    (source, focused)
}

/// 只记录正文实际创建的控件；不以位置或与窗框同层来猜测归属。
pub(super) fn remember_focus(
    focused: &mut Option<egui::Response>,
    response: egui::Response,
) -> egui::Response {
    if response.has_focus() {
        *focused = Some(response.clone());
    }
    response
}
pub(super) fn input_id(target: &worldline_core::TargetRef, field_id: &str) -> egui::Id {
    egui::Id::new(("schema-field-input", target, field_id))
}
fn control(
    ui: &mut egui::Ui,
    field: &SchemaAuthoringField,
    value: &mut PropertyValue,
    id: egui::Id,
) -> Option<egui::Response> {
    match (&field.value_type, value) {
        (SchemaType::Text, PropertyValue::Str(text)) => Some(
            ui.add(
                egui::TextEdit::multiline(text)
                    .id(id)
                    .desired_rows(3)
                    .desired_width(ui.available_width()),
            ),
        ),
        (SchemaType::Number, PropertyValue::Num(number)) => {
            Some(ui.add(egui::DragValue::new(number).speed(0.1)))
        }
        (SchemaType::Boolean, PropertyValue::Bool(value)) => {
            Some(ui.checkbox(value, "是 / 否（false 也是已填写）"))
        }
        (SchemaType::Enum { values }, PropertyValue::Str(text)) => {
            let choice = egui::ComboBox::from_id_salt("schema-enum")
                .width(ui.available_width().min(320.0))
                .truncate()
                .selected_text(if text.is_empty() {
                    "空字符串"
                } else {
                    text.as_str()
                })
                .show_ui(ui, |ui| {
                    let mut chosen = false;
                    for candidate in values {
                        chosen |= ui
                            .selectable_value(text, candidate.clone(), candidate)
                            .clicked();
                    }
                    if chosen {
                        ui.close();
                    }
                    chosen
                });
            if choice.inner == Some(true) {
                choice.response.request_focus();
            }
            Some(choice.response)
        }
        (SchemaType::Ref { .. }, PropertyValue::Ref(target)) => {
            let mut selection = Some(PropertyValue::Ref(target.clone()));
            let response = reference_choices(ui, field, &mut selection);
            if let Some(PropertyValue::Ref(selected)) = selection {
                *target = selected;
            }
            Some(response)
        }
        _ => None,
    }
}
fn reference_choices(
    ui: &mut egui::Ui,
    field: &SchemaAuthoringField,
    value: &mut Option<PropertyValue>,
) -> egui::Response {
    let selected = value
        .as_ref()
        .map(value_summary)
        .unwrap_or_else(|| "选择引用对象（未填写）".into());
    egui::ComboBox::from_id_salt("schema-reference")
        .width(ui.available_width().min(360.0))
        .truncate()
        .selected_text(selected)
        .show_ui(ui, |ui| {
            ui.set_max_width(ui.available_width().min(440.0));
            let key = ui.id().with("query");
            let mut query = ui
                .data_mut(|data| data.get_temp::<String>(key))
                .unwrap_or_default();
            ui.add(
                egui::TextEdit::singleline(&mut query)
                    .hint_text("搜索适用对象")
                    .desired_width(ui.available_width()),
            );
            let needle = query.to_lowercase();
            let candidates: Vec<_> = field
                .reference_candidates
                .iter()
                .filter(|object| {
                    needle.is_empty()
                        || object.display.to_lowercase().contains(&needle)
                        || object.target.id.to_lowercase().contains(&needle)
                        || object.target.kind.to_lowercase().contains(&needle)
                })
                .collect();
            let row_height = ui.spacing().interact_size.y;
            egui::ScrollArea::vertical().max_height(260.0).show_rows(
                ui,
                row_height,
                candidates.len(),
                |ui, rows| {
                    for index in rows {
                        let object = candidates[index];
                        let label = format!(
                            "{} · {}:{}",
                            object.display, object.target.kind, object.target.id
                        );
                        if ui
                            .add(
                                egui::Button::selectable(
                                    value.as_ref()
                                        == Some(&PropertyValue::Ref(object.target.clone())),
                                    &label,
                                )
                                .truncate(),
                            )
                            .on_hover_text(&label)
                            .clicked()
                        {
                            *value = Some(PropertyValue::Ref(object.target.clone()));
                            ui.close();
                        }
                    }
                },
            );
            if candidates.is_empty() {
                ui.label("没有适用的对象");
            }
            ui.data_mut(|data| data.insert_temp(key, query));
        })
        .response
}
fn initial_value(value_type: &SchemaType) -> Option<PropertyValue> {
    match value_type {
        SchemaType::Text => Some(PropertyValue::Str(String::new())),
        SchemaType::Number => Some(PropertyValue::Num(0.0)),
        SchemaType::Boolean => Some(PropertyValue::Bool(false)),
        SchemaType::Enum { values } => values.first().cloned().map(PropertyValue::Str),
        SchemaType::Ref { .. } => None,
    }
}
fn type_label(value_type: &SchemaType) -> &'static str {
    match value_type {
        SchemaType::Text => "文本",
        SchemaType::Number => "数值",
        SchemaType::Boolean => "布尔",
        SchemaType::Enum { .. } => "枚举",
        SchemaType::Ref { .. } => "对象引用",
    }
}
pub(super) fn value_summary(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Str(value) => format!("文本 {value:?}"),
        PropertyValue::Num(value) => value.to_string(),
        PropertyValue::Bool(value) => value.to_string(),
        PropertyValue::Ref(target) => format!("{}:{}", target.kind, target.id),
    }
}
