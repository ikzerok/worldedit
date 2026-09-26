//! 工程模板浏览和显式生命周期操作；写入只通过 core 的预览/应用 API。
use super::WorldeditApp;
use crate::theme;
use egui::RichText;
use serde_json::{json, Value};
use worldline_core::content_templates::ContentTemplate;
use worldline_core::project_templates::{
    ProjectTemplateMutation, ProjectTemplatePreview, TemplateCommand,
};

#[derive(Default)]
pub(super) struct ManagerState {
    selected_id: Option<String>,
    editor: String,
    preview: Option<ProjectTemplatePreview>,
    error: Option<String>,
}

impl WorldeditApp {
    pub(super) fn template_manager_tab(&mut self, ctx: &egui::Context) {
        let index = self.project.template_index();
        let current_revision = self.map_revision;
        let current_baseline = self.project.content_baseline();
        let mut state = std::mem::take(&mut self.template_manager);
        let mut requested_mutation = None;
        let mut requested_apply = false;
        let mut requested_cancel = false;
        let mut copied_json = None;

        egui::CentralPanel::default()
            .frame(theme::panel().fill(crate::theme::BG))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("template-manager-page")
                    .show(ui, |ui| {
                ui.heading("模板管理");
                ui.label(theme::muted(
                    "浏览只读；导入、替换与停用须先查看 core 影响预览，再明确应用。",
                ));
                ui.add_space(10.0);
                ui.columns(2, |columns| {
                    let left = &mut columns[0];
                    left.heading(format!("内置模板 · {} 项", index.builtins.len()));
                    egui::ScrollArea::vertical()
                        .id_salt("template-manager-builtins")
                        .max_height(340.0)
                        .show(left, |ui| {
                            for template in &index.builtins {
                                let id = template.id.clone();
                                if ui
                                    .selectable_label(
                                        state.selected_id.as_deref() == Some(&id),
                                        format!("{} · {}", template.title, template.id),
                                    )
                                    .clicked()
                                {
                                    state.selected_id = Some(id);
                                    state.preview = None;
                                }
                            }
                        });
                    left.add_space(8.0);
                    left.heading(format!("工程模板 · {} 项", index.projects.len()));
                    egui::ScrollArea::vertical()
                        .id_salt("template-manager-projects")
                        .max_height(340.0)
                        .show(left, |ui| {
                            for (id, document) in &index.projects {
                                let title = document
                                    .template
                                    .as_ref()
                                    .map(|template| template.title.as_str())
                                    .unwrap_or("无法读取模板文档");
                                let readonly = if document.read_only {
                                    " · 只读"
                                } else {
                                    ""
                                };
                                if ui
                                    .selectable_label(
                                        state.selected_id.as_deref() == Some(id),
                                        format!("{title} · {id}{readonly}"),
                                    )
                                    .clicked()
                                {
                                    state.selected_id = Some(id.clone());
                                    state.preview = None;
                                    if state.editor.trim().is_empty() {
                                        state.editor = String::from_utf8_lossy(&document.source_bytes)
                                            .into_owned();
                                    }
                                }
                            }
                        });

                    let right = &mut columns[1];
                    right.heading("模板信息与草稿");
                    match state.selected_id.as_deref() {
                        Some(id) if id.starts_with("project:") => {
                            if let Some(document) = index.projects.get(id) {
                                right.label(RichText::new(id).strong());
                                if let Some(template) = &document.template {
                                    right.label(&template.title);
                                    let schema_version = document
                                        .source_document
                                        .as_ref()
                                        .and_then(|source| source.get("schema_version"))
                                        .and_then(Value::as_u64);
                                    right.label(theme::muted(format!(
                                        "来源：工程 · {}",
                                        schema_version
                                            .map(|version| format!("schema v{version}"))
                                            .unwrap_or_else(|| "版本未知".into())
                                    )));
                                    right.label(theme::muted(format!(
                                        "适用对象：{}{} · {} 个字段",
                                        template.applies_to.kind,
                                        template
                                            .applies_to_entity_type
                                            .as_deref()
                                            .map(|kind| format!(" · {kind}"))
                                            .unwrap_or_default(),
                                        template.fields.len()
                                    )));
                                }
                                if document.read_only {
                                    right.colored_label(
                                        theme::GOLD,
                                        "只读模板：版本、必需能力或注册状态不受支持。",
                                    );
                                }
                                for diagnostic in &document.diagnostics {
                                    right.colored_label(
                                        theme::GOLD,
                                        format!(
                                            "{} · {}:{} · {}",
                                            diagnostic.code,
                                            diagnostic.file,
                                            diagnostic.span.line,
                                            diagnostic.message
                                        ),
                                    );
                                }
                            }
                        }
                        Some(id) => {
                            if let Some(template) = index.builtins.iter().find(|item| item.id == id)
                            {
                                right.label(RichText::new(&template.title).strong());
                                right.label(theme::muted(format!(
                                    "来源：内置 · schema v{}",
                                    template.schema_version
                                )));
                                right.label(theme::muted(format!(
                                    "适用对象：{}{} · {} 个提示字段",
                                    template.applies_to.kind,
                                    template
                                        .applies_to
                                        .entity_type
                                        .as_deref()
                                        .map(|kind| format!(" · {kind}"))
                                        .unwrap_or_default(),
                                    template.fields.len()
                                )));
                                right.label(theme::muted("内置模板只读；可复制为工程模板。"));
                            }
                        }
                        None => {
                            right.label(theme::muted("选择模板查看详情，或新建 / 导入模板 JSON。"));
                        }
                    }

                    right.horizontal_wrapped(|ui| {
                        if ui.button("新建空白模板").clicked() {
                            state.selected_id = None;
                            state.editor = blank_template_json(&index);
                            state.preview = None;
                            state.error = None;
                        }
                        let selected = state.selected_id.clone();
                        if let Some(id) = selected.as_deref() {
                            if let Some(document) = index.projects.get(id) {
                                if ui
                                    .add_enabled(
                                        !document.read_only,
                                        egui::Button::new("载入所选模板 JSON"),
                                    )
                                    .clicked()
                                {
                                    state.editor = String::from_utf8_lossy(&document.source_bytes)
                                        .into_owned();
                                    state.preview = None;
                                }
                            }
                            let copyable = selected_template_value(&index, id).is_some();
                            if ui
                                .add_enabled(copyable, egui::Button::new("复制为新模板 JSON"))
                                .clicked()
                            {
                                if let Some(document) = copied_template_json(&index, id) {
                                    state.editor = document;
                                    state.selected_id = None;
                                    state.preview = None;
                                    state.error = None;
                                }
                            }
                            if ui.button("导出 JSON 到剪贴板").clicked() {
                                copied_json = selected_template_json(&index, id);
                            }
                            if let Some(document) = index.projects.get(id) {
                                if ui
                                    .add_enabled(
                                        !document.read_only,
                                        egui::Button::new("预览停用模板（保留对象资料）"),
                                    )
                                    .clicked()
                                {
                                    requested_mutation = Some(ProjectTemplateMutation::Delete {
                                        id: id.to_owned(),
                                    });
                                }
                            }
                        }
                    });

                    right.label(theme::muted(
                        "JSON 草稿只在确认应用预览后写入。停用会解除注册并移除模板文档，不改动实例属性。",
                    ));
                    let response = right.add(
                        egui::TextEdit::multiline(&mut state.editor)
                            .id(egui::Id::new("template-manager-json"))
                            .desired_rows(16)
                            .desired_width(f32::INFINITY)
                            .hint_text("粘贴或编辑模板 JSON"),
                    );
                    if response.changed() {
                        state.preview = None;
                        state.error = None;
                    }
                    if right.button("预览导入 / 替换").clicked() {
                        match editor_mutation(&state.editor, &index) {
                            Ok(mutation) => requested_mutation = Some(mutation),
                            Err(error) => state.error = Some(error),
                        }
                    }

                    if let Some(error) = &state.error {
                        right.colored_label(theme::ERROR, error);
                    }
                    if let Some(preview) = &state.preview {
                        right.separator();
                        right.heading(preview_title(&preview.mutation));
                        if preview.expected_revision != current_revision
                            || preview.expected_baseline != current_baseline
                        {
                            right.colored_label(
                                theme::GOLD,
                                "预览已过期；应用会被 core 拒绝，请重新预览。",
                            );
                        }
                        right.label("字段变化");
                        if preview.field_changes.is_empty() {
                            right.label(theme::muted("没有字段结构变化。"));
                        }
                        for change in &preview.field_changes {
                            right.label(format!(
                                "{} · {} · {} → {} · {:?} → {:?}",
                                change.change,
                                change.field_id,
                                change.old_key.as_deref().unwrap_or("—"),
                                change.new_key.as_deref().unwrap_or("—"),
                                change.old_type,
                                change.new_type
                            ));
                        }
                        right.label(format!("实例影响 · {} 个对象", preview.instances.len()));
                        for instance in &preview.instances {
                            right.label(format!(
                                "{}:{}",
                                instance.target.kind, instance.target.id
                            ));
                            for field in &instance.fields {
                                right.label(format!(
                                    "{} · {} · {} · {:?}",
                                    field.template_state, field.key, field.field_id, field.state
                                ));
                            }
                        }
                        for diagnostic in &preview.diagnostics {
                            right.colored_label(
                                if diagnostic.severity == worldline_core::Severity::Error {
                                    theme::ERROR
                                } else {
                                    theme::GOLD
                                },
                                format!("{} · {}", diagnostic.code, diagnostic.message),
                            );
                        }
                        right.label(theme::muted(
                            "预览不会改写实例值；字段类型变化不会转换或清除现有资料。",
                        ));
                        right.horizontal(|ui| {
                            if ui.button("取消预览").clicked() {
                                requested_cancel = true;
                            }
                            if ui.button(apply_label(&preview.mutation)).clicked() {
                                requested_apply = true;
                            }
                        });
                    }
                });
                if !index.diagnostics.is_empty() {
                    ui.separator();
                    ui.label(RichText::new("工程模板诊断").strong());
                    for diagnostic in &index.diagnostics {
                        ui.colored_label(
                            theme::ERROR,
                            format!("{} · {}", diagnostic.code, diagnostic.message),
                        );
                    }
                }
                    });
            });

        if requested_cancel {
            state.preview = None;
            state.error = None;
        }
        if let Some(text) = copied_json {
            ctx.copy_text(text);
            state.error = Some("模板 JSON 已复制到剪贴板。".into());
        }
        if let Some(mutation) = requested_mutation {
            let command = TemplateCommand {
                expected_revision: self.map_revision,
                expected_baseline: self.project.content_baseline(),
                mutation,
                check_integrity: true,
            };
            match self
                .project
                .preview_template_mutation(self.map_revision, &command)
            {
                Ok(preview) => {
                    state.preview = Some(preview);
                    state.error = None;
                }
                Err(error) => {
                    state.preview = None;
                    state.error = Some(error.clone());
                    self.io_error = Some(error);
                }
            }
        }
        if requested_apply {
            if let Some(preview) = state.preview.as_ref().cloned() {
                let deleting = matches!(preview.mutation, ProjectTemplateMutation::Delete { .. });
                let before = self.project.clone();
                match self
                    .project
                    .apply_template_mutation(&mut self.map_revision, preview)
                {
                    Ok(_) => {
                        self.remember(before);
                        self.recompile();
                        self.io_error = None;
                        self.message = Some("工程模板变更已应用；保存全部可写入作品目录".into());
                        state.preview = None;
                        state.error = None;
                        if deleting {
                            state.selected_id = None;
                            state.editor.clear();
                        }
                    }
                    Err(error) => {
                        state.error = Some(error.clone());
                        self.io_error = Some(error);
                    }
                }
            }
        }
        self.template_manager = state;
    }
}

fn preview_title(mutation: &ProjectTemplateMutation) -> &'static str {
    match mutation {
        ProjectTemplateMutation::Import { .. } => "导入影响预览",
        ProjectTemplateMutation::Replace { .. } => "替换影响预览",
        ProjectTemplateMutation::Delete { .. } => "停用影响预览",
    }
}

fn apply_label(mutation: &ProjectTemplateMutation) -> &'static str {
    match mutation {
        ProjectTemplateMutation::Delete { .. } => "确认停用模板",
        _ => "应用预览中的模板变更",
    }
}

fn editor_mutation(
    editor: &str,
    index: &worldline_core::project_templates::ProjectTemplateIndex,
) -> Result<ProjectTemplateMutation, String> {
    let document: Value =
        serde_json::from_str(editor).map_err(|error| format!("JSON 无效：{error}"))?;
    let id = document
        .get("id")
        .and_then(Value::as_str)
        .ok_or("模板 JSON 缺少字符串 id")?;
    let mutation = if index.projects.contains_key(id) {
        ProjectTemplateMutation::Replace {
            id: id.to_owned(),
            document: editor.as_bytes().to_vec(),
        }
    } else {
        ProjectTemplateMutation::Import {
            id: id.to_owned(),
            document: editor.as_bytes().to_vec(),
        }
    };
    Ok(mutation)
}

fn blank_template_json(index: &worldline_core::project_templates::ProjectTemplateIndex) -> String {
    let id = (1..)
        .map(|number| format!("project:new_template_{number}"))
        .find(|id| !index.projects.contains_key(id))
        .unwrap_or_else(|| "project:new_template".into());
    serde_json::to_string_pretty(&json!({
        "schema_version": 1,
        "id": id,
        "title": "新工程模板",
        "applies_to": {"kind": "entity", "entity_type": "place"},
        "fields": []
    }))
    .unwrap_or_default()
}

fn selected_template_value(
    index: &worldline_core::project_templates::ProjectTemplateIndex,
    id: &str,
) -> Option<Value> {
    if let Some(document) = index.projects.get(id) {
        if document.read_only {
            return None;
        }
        let mut value = document.source_document.clone()?;
        let object = value.as_object_mut()?;
        object.insert("id".into(), Value::String(next_copy_id(index)));
        if let Some(title) = object
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_owned)
        {
            object.insert("title".into(), Value::String(format!("{title} 副本")));
        }
        return Some(value);
    }
    index
        .builtins
        .iter()
        .find(|template| template.id == id)
        .map(|template| builtin_project_document(template, next_copy_id(index)))
}

fn selected_template_json(
    index: &worldline_core::project_templates::ProjectTemplateIndex,
    id: &str,
) -> Option<String> {
    if let Some(document) = index.projects.get(id) {
        return String::from_utf8(document.source_bytes.clone()).ok();
    }
    let template = index.builtins.iter().find(|template| template.id == id)?;
    serde_json::to_string_pretty(&builtin_project_document(template, template.id.clone())).ok()
}

fn copied_template_json(
    index: &worldline_core::project_templates::ProjectTemplateIndex,
    id: &str,
) -> Option<String> {
    serde_json::to_string_pretty(&selected_template_value(index, id)?).ok()
}

fn next_copy_id(index: &worldline_core::project_templates::ProjectTemplateIndex) -> String {
    (1..)
        .map(|number| format!("project:template_copy_{number}"))
        .find(|id| !index.projects.contains_key(id))
        .unwrap_or_else(|| "project:template_copy".into())
}

fn builtin_project_document(template: &ContentTemplate, id: String) -> Value {
    let applies_to = if let Some(entity_type) = template.applies_to.entity_type.as_deref() {
        json!({"kind": template.applies_to.kind, "entity_type": entity_type})
    } else {
        json!({"kind": template.applies_to.kind})
    };
    let fields = template
        .fields
        .iter()
        .map(|field| {
            let field_type = match field.widget.as_str() {
                "multiline" | "text" => "text",
                "number" => "number",
                "boolean" => "boolean",
                other => other,
            };
            json!({
                "id": field.key,
                "key": field.key,
                "label": field.label,
                "type": field_type,
                "required": field.required
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": 1,
        "id": id,
        "title": format!("{} 副本", template.title),
        "applies_to": applies_to,
        "fields": fields
    })
}
