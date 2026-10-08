use super::*;

pub(super) fn properties(
    ui: &mut egui::Ui,
    input: &mut FieldInput,
    catalog: &Catalog,
    options: worldline_core::CompileOptions,
    allow_refs: bool,
) -> (bool, bool) {
    ui.heading("字段属性");
    ui.label(theme::muted(format!("稳定字段 ID · {}", input.field.id)));
    ui.label("显示名称");
    super::super::input::single(
        ui,
        ("template-field-label", &input.field.id),
        &mut input.field.label,
    );
    let is_group = input.field.field_type == "group";
    if is_group {
        ui.label("分组只组织表单，不生成实例属性。删除分组也会删除其字段定义。");
    } else {
        ui.label("实例属性 key");
        super::super::input::single(
            ui,
            ("template-field-key", &input.field.id),
            input.field.key.get_or_insert_default(),
        );
        let mut selected = input.field.field_type.clone();
        egui::ComboBox::from_id_salt("template-field-type")
            .selected_text(type_label(&selected))
            .show_ui(ui, |ui| {
                for kind in ["text", "number", "boolean", "enum", "object_ref"] {
                    ui.add_enabled_ui(
                        kind != "object_ref"
                            || (options.language_version.supports_entities() && allow_refs),
                        |ui| {
                            ui.selectable_value(&mut selected, kind.to_owned(), type_label(kind));
                        },
                    );
                }
            });
        if selected != input.field.field_type {
            input.change_type(&selected);
        }
        ui.checkbox(&mut input.field.required, "标为必填提示（不自动填写）");
        ui.label(theme::muted(
            "更换类型会清除草稿中的旧选项、引用约束和默认提示；不改实例。",
        ));
        if input.field.field_type == "enum" {
            ui.label("枚举选项 · 每行一个（不能空白或重复）");
            super::super::input::multiline(
                ui,
                ("template-field-choices", &input.field.id),
                &mut input.choices_text,
                4,
            );
        }
        if input.field.field_type == "object_ref" {
            reference_constraint(ui, input, options);
        }
        ui.checkbox(&mut input.has_default, "提供默认提示（须作者显式选择）");
        if input.has_default {
            default_input(ui, input, catalog);
        }
    }
    ui.horizontal_wrapped(|ui| {
        let apply = ui.button("更新字段草稿").clicked();
        let reset = ui.button("还原字段输入").clicked();
        (apply, reset)
    })
    .inner
}

fn reference_constraint(
    ui: &mut egui::Ui,
    input: &mut FieldInput,
    options: worldline_core::CompileOptions,
) {
    let target = input
        .field
        .target
        .get_or_insert_with(|| TargetRef::new("entity", ""));
    let old = target.kind.clone();
    egui::ComboBox::from_id_salt("template-field-ref-kind")
        .selected_text(format!("引用类型 · {}", target.kind))
        .show_ui(ui, |ui| {
            for kind in ["entity", "relation", "character"] {
                ui.add_enabled_ui(
                    kind != "character"
                        || worldline_core::catalog::is_object_reference_kind(kind, options),
                    |ui| {
                        ui.selectable_value(&mut target.kind, kind.into(), kind);
                    },
                );
            }
        });
    if target.kind != old {
        input.default_ref = None;
        input.field.target_entity_type = None;
    }
    if target.kind == "entity" {
        let mut entity_type = input.field.target_entity_type.clone().unwrap_or_default();
        ui.label("限制实体分类（可留空）");
        if super::super::input::single(
            ui,
            ("template-field-target-type", &input.field.id),
            &mut entity_type,
        )
        .changed()
        {
            input.field.target_entity_type = (!entity_type.is_empty()).then_some(entity_type);
        }
    }
    if target.kind == "character" {
        ui.label(theme::muted(
            "人物引用要求语言 1.13 及现有双能力保护；由 core 核对，不自动升级语言。",
        ));
    }
}

fn default_input(ui: &mut egui::Ui, input: &mut FieldInput, catalog: &Catalog) {
    match input.field.field_type.as_str() {
        "text" => {
            super::super::input::multiline(
                ui,
                ("template-field-default", &input.field.id),
                &mut input.default_text,
                2,
            );
        }
        "number" | "enum" => {
            super::super::input::single(
                ui,
                ("template-field-default", &input.field.id),
                &mut input.default_text,
            );
        }
        "boolean" => {
            ui.checkbox(&mut input.default_bool, "默认值为 true");
        }
        "object_ref" => {
            let kind = input
                .field
                .target
                .as_ref()
                .map(|target| target.kind.as_str())
                .unwrap_or("entity");
            crate::app::object_picker::object_picker_typed(
                ui,
                ("template-draft-default", &input.field.id),
                "默认引用对象",
                &mut input.default_ref,
                catalog,
                &crate::app::object_picker::filter(
                    &[kind],
                    input.field.target_entity_type.as_deref(),
                ),
            );
        }
        _ => {}
    }
}
