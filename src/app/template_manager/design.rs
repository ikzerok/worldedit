use super::*;
use worldline_core::catalog::Catalog;
use worldline_core::TargetRef;
mod properties;

pub(super) fn show(
    ui: &mut egui::Ui,
    state: &mut ManagerState,
    template: &ProjectTemplate,
    catalog: &Catalog,
    options: worldline_core::CompileOptions,
) -> Option<ProjectTemplateDraftEdit> {
    let language = options.language_version;
    let allow_refs =
        language.supports_entities() && (state.existing_id.is_none() || options.object_refs);
    let mut edit = metadata(ui, state, template);
    ui.separator();
    ui.heading("字段大纲");
    ui.label(theme::muted(
        "稳定身份不随位置改变；选择字段编辑属性，使用上下移或移动到分组。",
    ));
    ui.horizontal_wrapped(|ui| {
        for kind in ["text", "number", "boolean", "enum", "object_ref", "group"] {
            let allowed = kind != "object_ref" || allow_refs;
            if theme::add_enabled(
                ui,
                allowed,
                egui::Button::new(format!("＋ {}", type_label(kind))),
            )
            .clicked()
            {
                let parent = state
                    .selected_field
                    .as_deref()
                    .and_then(|id| find_field(&template.fields, id))
                    .filter(|field| field.field_type == "group");
                edit = Some(ProjectTemplateDraftEdit::AddField {
                    parent_id: parent.map(|field| field.id.clone()),
                    index: parent.map_or(template.fields.len(), |field| field.fields.len()),
                    field_type: field_type(kind),
                });
            }
        }
    });
    if !allow_refs {
        ui.label(theme::muted(
            "对象引用尚不可新增；请在工程 → 语言与资料能力中显式启用所需能力，草稿会保留。",
        ));
    }
    let wide = ui.available_width() >= 760.0;
    if wide {
        ui.columns(2, |columns| {
            outline(
                &mut columns[0],
                state,
                template,
                &template.fields,
                None,
                &mut edit,
            );
            property_panel(
                &mut columns[1],
                state,
                template,
                catalog,
                options,
                &mut edit,
            );
        });
    } else {
        egui::CollapsingHeader::new(format!("全部字段 · {} 项", count_fields(&template.fields)))
            .default_open(true)
            .show(ui, |ui| {
                outline(ui, state, template, &template.fields, None, &mut edit);
            });
        ui.separator();
        property_panel(ui, state, template, catalog, options, &mut edit);
    }
    edit
}
fn metadata(
    ui: &mut egui::Ui,
    state: &mut ManagerState,
    template: &ProjectTemplate,
) -> Option<ProjectTemplateDraftEdit> {
    let input = state
        .metadata
        .get_or_insert_with(|| MetadataInput::new(template));
    let mut edit = None;
    ui.heading("模板信息");
    ui.label(theme::muted(format!("稳定模板 ID · {}", template.id)));
    ui.label("模板名称");
    super::input::single(ui, "template-meta-title", &mut input.title);
    ui.horizontal_wrapped(|ui| {
        ui.label("适用对象");
        egui::ComboBox::from_id_salt("template-applies-kind")
            .selected_text(&input.kind)
            .show_ui(ui, |ui| {
                for kind in ["world", "character", "entity"] {
                    ui.selectable_value(&mut input.kind, kind.into(), kind);
                }
            });
        if input.kind == "entity" {
            ui.label("实体分类（可空）");
            super::input::single(ui, "template-meta-entity-type", &mut input.entity_type);
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui.button("更新模板信息草稿").clicked() {
            edit = Some(ProjectTemplateDraftEdit::SetMetadata {
                title: input.title.clone(),
                applies_to: ProjectTemplateDraftTarget {
                    kind: input.kind.clone(),
                    entity_type: (input.kind == "entity" && !input.entity_type.is_empty())
                        .then(|| input.entity_type.clone()),
                },
            });
        }
        if ui.button("还原模板信息输入").clicked() {
            *input = MetadataInput::new(template);
        }
    });
    edit
}
fn outline(
    ui: &mut egui::Ui,
    state: &mut ManagerState,
    template: &ProjectTemplate,
    fields: &[ProjectTemplateField],
    parent: Option<&str>,
    edit: &mut Option<ProjectTemplateDraftEdit>,
) {
    for (position, field) in fields.iter().enumerate() {
        ui.push_id(("template-outline", &field.id), |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .selectable_label(
                        state.selected_field.as_deref() == Some(&field.id),
                        format!("{} · {}", field.label, type_label(&field.field_type)),
                    )
                    .clicked()
                {
                    state.select_field(template, &field.id);
                }
                if theme::add_enabled(ui, position > 0, egui::Button::new("↑"))
                    .on_hover_text("上移字段")
                    .clicked()
                {
                    *edit = Some(ProjectTemplateDraftEdit::MoveField {
                        field_id: field.id.clone(),
                        parent_id: parent.map(str::to_owned),
                        index: position - 1,
                    });
                }
                if theme::add_enabled(ui, position + 1 < fields.len(), egui::Button::new("↓"))
                    .on_hover_text("下移字段")
                    .clicked()
                {
                    *edit = Some(ProjectTemplateDraftEdit::MoveField {
                        field_id: field.id.clone(),
                        parent_id: parent.map(str::to_owned),
                        index: position + 1,
                    });
                }
            });
            ui.label(theme::muted(format!(
                "{}{}",
                field.id,
                field
                    .key
                    .as_ref()
                    .map(|key| format!(" · key: {key}"))
                    .unwrap_or_default()
            )));
            if !field.fields.is_empty() {
                ui.indent((&field.id, "children"), |ui| {
                    outline(ui, state, template, &field.fields, Some(&field.id), edit)
                });
            }
        });
    }
    if fields.is_empty() {
        ui.label(theme::muted("还没有字段，从上方添加一个。"));
    }
}
fn property_panel(
    ui: &mut egui::Ui,
    state: &mut ManagerState,
    template: &ProjectTemplate,
    catalog: &Catalog,
    options: worldline_core::CompileOptions,
    edit: &mut Option<ProjectTemplateDraftEdit>,
) {
    let Some(input) = state.properties.as_mut() else {
        ui.label(theme::muted("选择大纲中的字段查看和编辑属性。"));
        return;
    };
    let (apply, reset) = properties::properties(
        ui,
        input,
        catalog,
        options,
        state.existing_id.is_none() || options.object_refs,
    );
    if reset {
        input.reset();
    }
    if apply {
        match input.value() {
            Ok(field) => {
                *edit = Some(ProjectTemplateDraftEdit::UpdateField {
                    field_id: field.id.clone(),
                    properties: properties_from(&field),
                });
            }
            Err(error) => state.error = Some(error),
        }
    }
    ui.separator();
    let id = input.field.id.clone();
    egui::ComboBox::from_id_salt("template-move-parent")
        .selected_text(state.move_parent.as_deref().unwrap_or("模板根层"))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut state.move_parent, None, "模板根层");
            group_options(ui, &template.fields, &id, &mut state.move_parent);
        });
    if ui.button("移动到所选分组末尾").clicked() {
        let fields = state
            .move_parent
            .as_deref()
            .and_then(|id| find_field(&template.fields, id))
            .map_or(&template.fields, |field| &field.fields);
        let count = fields.len() - usize::from(fields.iter().any(|field| field.id == id));
        *edit = Some(ProjectTemplateDraftEdit::MoveField {
            field_id: id.clone(),
            parent_id: state.move_parent.clone(),
            index: count,
        });
    }
    if ui
        .button(if input.field.field_type == "group" {
            "删除分组及全部字段…"
        } else {
            "删除字段…"
        })
        .clicked()
    {
        state.remove_field = Some(id);
    }
}
fn group_options(
    ui: &mut egui::Ui,
    fields: &[ProjectTemplateField],
    excluded: &str,
    selected: &mut Option<String>,
) {
    for field in fields {
        if field.field_type == "group" && field.id != excluded {
            ui.selectable_value(
                selected,
                Some(field.id.clone()),
                format!("{} · {}", field.label, field.id),
            );
            group_options(ui, &field.fields, excluded, selected);
        }
    }
}
fn properties_from(field: &ProjectTemplateField) -> ProjectTemplateFieldProperties {
    ProjectTemplateFieldProperties {
        label: field.label.clone(),
        key: field.key.clone(),
        field_type: field_type(&field.field_type),
        required: field.required,
        choices: field.choices.clone(),
        default: field.default.clone(),
        target: field
            .target
            .as_ref()
            .map(|target| ProjectTemplateDraftTarget {
                kind: target.kind.clone(),
                entity_type: field.target_entity_type.clone(),
            }),
    }
}
fn field_type(kind: &str) -> ProjectTemplateFieldType {
    match kind {
        "number" => ProjectTemplateFieldType::Number,
        "boolean" => ProjectTemplateFieldType::Boolean,
        "enum" => ProjectTemplateFieldType::Enum,
        "object_ref" => ProjectTemplateFieldType::ObjectRef,
        "group" => ProjectTemplateFieldType::Group,
        _ => ProjectTemplateFieldType::Text,
    }
}
pub(super) fn type_label(kind: &str) -> &str {
    match kind {
        "text" => "文本",
        "number" => "数字",
        "boolean" => "布尔",
        "enum" => "枚举",
        "object_ref" => "对象引用",
        "group" => "分组",
        other => other,
    }
}
fn count_fields(fields: &[ProjectTemplateField]) -> usize {
    fields
        .iter()
        .map(|field| 1 + count_fields(&field.fields))
        .sum()
}
