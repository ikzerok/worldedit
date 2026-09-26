//! 16 类可选内容模板的共享 UI；定义与匹配来自 worldline-core。
use crate::theme;
use egui::RichText;
use worldline_core::ast::PropertyValue;
use worldline_core::catalog::{Catalog, TargetRef};
use worldline_core::project_templates::{
    ProjectTemplate, ProjectTemplateField, ProjectTemplateIndex,
};

fn add_optional_text(
    ui: &mut egui::Ui,
    values: &mut Vec<(String, PropertyValue)>,
    key: &str,
    label: &str,
) {
    if let Some((_, value)) = values.iter_mut().find(|(name, _)| name == key) {
        ui.label(RichText::new(label).strong());
        match value {
            PropertyValue::Str(text) => {
                ui.add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
            }
            _ => {
                ui.label(theme::muted("此字段已有非文本自定义值；模板不会覆盖它。"));
            }
        }
    } else if ui.button(format!("＋ {label}")).clicked() {
        values.push((key.into(), PropertyValue::Str(String::new())));
    }
}
pub(super) fn template_panel(
    ui: &mut egui::Ui,
    kind: &str,
    entity_type: Option<&str>,
    project_templates: &ProjectTemplateIndex,
    catalog: &Catalog,
    values: &mut Vec<(String, PropertyValue)>,
) -> Option<String> {
    let template = worldline_core::content_templates::matching_template(kind, entity_type);
    if template.is_none()
        && !project_templates.projects.values().any(|document| {
            document.template.as_ref().is_some_and(|template| {
                template.applies_to.kind == kind
                    && template
                        .applies_to_entity_type
                        .as_deref()
                        .is_none_or(|expected| Some(expected) == entity_type)
            })
        })
    {
        ui.label(theme::muted(
            "当前分类没有专用模板；仍可使用正文和自定义属性。",
        ));
    }

    let mut suggestion = None;
    if let Some(template) = template {
        egui::CollapsingHeader::new(format!("创作模板 · {}", template.title))
            .default_open(false)
            .show(ui, |ui| {
                ui.label(theme::muted(
                    "模板字段全部可选；切换模板不会删除正文、既有属性或未知字段。",
                ));
                for prompt in &template.prompts {
                    ui.label(format!("思考：{prompt}"));
                }
                ui.separator();
                ui.label(RichText::new("关系建议").strong());
                ui.label(theme::muted("点击只会打开关系草稿，不会自动生成事实。"));
                ui.horizontal_wrapped(|ui| {
                    for label in &template.suggested_relations {
                        if ui.button(label).clicked() {
                            suggestion = Some(label.clone());
                        }
                    }
                });
                ui.separator();
                for field in &template.fields {
                    add_optional_text(ui, values, &field.key, &field.label);
                }
            });
    }

    egui::CollapsingHeader::new("来源、陈述性质与创作状态").show(ui, |ui| {
        ui.label(theme::muted(
            "这些是作者资料，不把“已接受”自动解释为“客观事实”。",
        ));
        add_optional_text(ui, values, "source_note", "来源 / 依据");
        add_optional_text(
            ui,
            values,
            "statement_nature",
            "陈述性质（事实 / 传闻 / 观点等）",
        );
        add_optional_text(
            ui,
            values,
            "authoring_status",
            "创作状态（草稿 / 已接受等）",
        );
    });
    for (id, document) in &project_templates.projects {
        let Some(project_template) = document.template.as_ref() else {
            continue;
        };
        if project_template.applies_to.kind != kind
            || project_template
                .applies_to_entity_type
                .as_deref()
                .is_some_and(|expected| Some(expected) != entity_type)
        {
            continue;
        }
        egui::CollapsingHeader::new(format!("工程模板 · {}", project_template.title))
            .id_salt(("project-template-form", id))
            .default_open(true)
            .show(ui, |ui| {
                ui.label(theme::muted(format!("稳定模板 ID · {id}")));
                if document.read_only {
                    ui.colored_label(
                        theme::GOLD,
                        "模板格式或必需能力不受支持，只读查看；不会覆盖未知字段。",
                    );
                    for diagnostic in &document.diagnostics {
                        ui.label(format!(
                            "{} · {}:{} · {}",
                            diagnostic.code,
                            diagnostic.file,
                            diagnostic.span.line,
                            diagnostic.message
                        ));
                    }
                    return;
                }
                if document
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.severity == worldline_core::Severity::Error)
                {
                    ui.colored_label(
                        theme::GOLD,
                        "模板含有字段错误；为避免丢失原值，暂不显示可编辑字段。",
                    );
                    for diagnostic in &document.diagnostics {
                        ui.label(format!("{} · {}", diagnostic.code, diagnostic.message));
                    }
                    return;
                }
                ui.label(theme::muted(
                    "字段在当前表单中编辑；未填写默认值不会自动写入，未知属性保持原样。",
                ));
                render_project_fields(
                    ui,
                    project_template,
                    &project_template.fields,
                    catalog,
                    values,
                );
            });
    }
    suggestion
}

fn render_project_fields(
    ui: &mut egui::Ui,
    template: &ProjectTemplate,
    fields: &[ProjectTemplateField],
    catalog: &Catalog,
    values: &mut Vec<(String, PropertyValue)>,
) {
    for field in fields {
        ui.push_id(("project-template-field", &template.id, &field.id), |ui| {
            if field.field_type == "group" {
                egui::CollapsingHeader::new(&field.label)
                    .default_open(true)
                    .show(ui, |ui| {
                        render_project_fields(ui, template, &field.fields, catalog, values);
                    });
                return;
            }
            let Some(key) = field.key.as_deref() else {
                ui.colored_label(
                    theme::GOLD,
                    format!("{} · 字段 key 缺失，按只读显示。", field.label),
                );
                return;
            };
            let label = format!(
                "{}{} · {}",
                if field.required { "* " } else { "" },
                field.label,
                key
            );
            let position = values.iter().position(|(name, _)| name == key);
            let current = position.map(|index| values[index].1.clone());

            if let Some(value) = current.as_ref() {
                let matches = matches_field_value(field, value, catalog);
                let enum_value =
                    field.field_type == "enum" && matches!(value, PropertyValue::Str(_));
                if !matches && !enum_value {
                    ui.label(RichText::new(&label).strong());
                    ui.colored_label(
                        theme::GOLD,
                        format!("已有值类型不匹配，保留原值：{}", value_summary(value)),
                    );
                    return;
                }
            }

            match field.field_type.as_str() {
                "text" => {
                    if let Some(index) = position {
                        if let PropertyValue::Str(value) = &mut values[index].1 {
                            ui.label(RichText::new(&label).strong());
                            ui.add(
                                egui::TextEdit::multiline(value)
                                    .desired_rows(2)
                                    .desired_width(f32::INFINITY),
                            );
                        }
                    } else if add_property_button(ui, field, &label) {
                        let initial = field
                            .default
                            .as_ref()
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                            .to_owned();
                        values.push((key.to_owned(), PropertyValue::Str(initial)));
                    }
                }
                "number" => {
                    if let Some(index) = position {
                        if let PropertyValue::Num(value) = &mut values[index].1 {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&label).strong());
                                ui.add(egui::DragValue::new(value).speed(0.1));
                            });
                        }
                    } else if add_property_button(ui, field, &label) {
                        let initial = field
                            .default
                            .as_ref()
                            .and_then(serde_json::Value::as_f64)
                            .unwrap_or(0.0);
                        values.push((key.to_owned(), PropertyValue::Num(initial)));
                    }
                }
                "boolean" => {
                    if let Some(index) = position {
                        if let PropertyValue::Bool(value) = &mut values[index].1 {
                            ui.checkbox(value, &label);
                        }
                    } else {
                        let initial = field
                            .default
                            .as_ref()
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false);
                        if ui.button(format!("＋ {label} · {initial}")).clicked() {
                            values.push((key.to_owned(), PropertyValue::Bool(initial)));
                        }
                    }
                }
                "enum" => render_enum_field(ui, field, &label, key, values),
                "object_ref" => render_reference_field(ui, field, &label, key, catalog, values),
                _ => {
                    ui.label(theme::muted(format!(
                        "{} · 未知字段类型，保留原值。",
                        label
                    )));
                }
            }
        });
    }
}

fn add_property_button(ui: &mut egui::Ui, field: &ProjectTemplateField, label: &str) -> bool {
    let text = if field.default.is_some() {
        format!("使用模板默认值 · {label}")
    } else {
        format!("＋ {label}")
    };
    ui.button(text).clicked()
}

fn render_enum_field(
    ui: &mut egui::Ui,
    field: &ProjectTemplateField,
    label: &str,
    key: &str,
    values: &mut Vec<(String, PropertyValue)>,
) {
    let index = values.iter().position(|(name, _)| name == key);
    let old = index.and_then(|index| match &values[index].1 {
        PropertyValue::Str(value) => Some(value.clone()),
        _ => None,
    });
    let mut selected = old.clone().filter(|value| field.choices.contains(value));
    let selected_text = selected
        .clone()
        .or_else(|| {
            old.as_ref()
                .map(|value| format!("原值（不在选项中）：{value}"))
        })
        .unwrap_or_else(|| "选择…".into());
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).strong());
        egui::ComboBox::from_id_salt(("project-template-enum", &field.id, key))
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                for choice in &field.choices {
                    ui.selectable_value(&mut selected, Some(choice.clone()), choice);
                }
            });
    });
    if let Some(selected) = selected {
        if old.as_deref() != Some(&selected) {
            set_property(values, key, PropertyValue::Str(selected));
        }
    }
}

fn render_reference_field(
    ui: &mut egui::Ui,
    field: &ProjectTemplateField,
    label: &str,
    key: &str,
    catalog: &Catalog,
    values: &mut Vec<(String, PropertyValue)>,
) {
    let Some(target_kind) = field.target.as_ref().map(|target| target.kind.as_str()) else {
        ui.colored_label(theme::GOLD, format!("{label} · 缺少对象类型约束。"));
        return;
    };
    let index = values.iter().position(|(name, _)| name == key);
    let old = index.and_then(|index| match &values[index].1 {
        PropertyValue::Ref(target) => Some(target.clone()),
        _ => None,
    });
    let candidates = catalog
        .objects
        .iter()
        .filter(|object| object.target.kind == target_kind)
        .filter(|object| {
            field
                .target_entity_type
                .as_deref()
                .is_none_or(|entity_type| {
                    catalog
                        .entities
                        .get(&object.target.id)
                        .is_some_and(|entity| entity.entity_type == entity_type)
                })
        })
        .collect::<Vec<_>>();
    let mut selected = old.clone();
    let selected_text = old
        .as_ref()
        .map(|target| target_label(catalog, target))
        .unwrap_or_else(|| "选择对象…".into());
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).strong());
        egui::ComboBox::from_id_salt(("project-template-reference", &field.id, key))
            .selected_text(selected_text)
            .show_ui(ui, |ui| {
                if candidates.is_empty() {
                    ui.label(theme::muted("没有符合类型约束的对象。"));
                }
                for candidate in candidates {
                    ui.selectable_value(
                        &mut selected,
                        Some(candidate.target.clone()),
                        format!(
                            "{} · {}:{}",
                            candidate.display, candidate.target.kind, candidate.target.id
                        ),
                    );
                }
            });
    });
    if selected != old {
        if let Some(target) = selected {
            set_property(values, key, PropertyValue::Ref(target));
        }
    }
}

fn matches_field_value(
    field: &ProjectTemplateField,
    value: &PropertyValue,
    catalog: &Catalog,
) -> bool {
    match field.field_type.as_str() {
        "text" => matches!(value, PropertyValue::Str(_)),
        "number" => matches!(value, PropertyValue::Num(number) if number.is_finite()),
        "boolean" => matches!(value, PropertyValue::Bool(_)),
        "enum" => matches!(value, PropertyValue::Str(text) if field.choices.contains(text)),
        "object_ref" => matches!(value, PropertyValue::Ref(target)
            if field.target.as_ref().is_some_and(|expected| expected.kind == target.kind)
                && field.target_entity_type.as_deref().is_none_or(|expected| catalog.entities.get(&target.id).is_some_and(|entity| entity.entity_type == expected))),
        "group" => true,
        _ => false,
    }
}

fn set_property(values: &mut Vec<(String, PropertyValue)>, key: &str, value: PropertyValue) {
    if let Some((_, current)) = values.iter_mut().find(|(name, _)| name == key) {
        *current = value;
    } else {
        values.push((key.to_owned(), value));
    }
}

fn value_summary(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Str(value) => format!("字符串 `{value}`"),
        PropertyValue::Num(value) => format!("数值 `{value}`"),
        PropertyValue::Bool(value) => format!("布尔值 `{value}`"),
        PropertyValue::Ref(target) => format!("引用 `{}:{}`", target.kind, target.id),
    }
}

fn target_label(catalog: &Catalog, target: &TargetRef) -> String {
    catalog
        .object(target)
        .map(|object| format!("{} · {}:{}", object.display, target.kind, target.id))
        .unwrap_or_else(|| format!("未解析引用 · {}:{}", target.kind, target.id))
}
