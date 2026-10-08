use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn toolbar(
    ui: &mut egui::Ui,
    state: &mut ManagerState,
    index: &ProjectTemplateIndex,
    project: &worldline_core::project::Project,
    revision: worldline_core::presentation_commands::Revision,
    action: &mut Option<Action>,
    mutation: &mut Option<ProjectTemplateMutation>,
    apply: &mut bool,
    cancel: &mut bool,
) {
    let compact = ui.available_width() < 850.0;
    ui.horizontal_wrapped(|ui| {
        if ui.button("新建空白模板").clicked() {
            *action = Some(Action::New);
        }
        if let Some(id) = state.selected_id.clone() {
            if theme::add_enabled(
                ui,
                index.builtins.iter().any(|t| t.id == id)
                    || index.projects.get(&id).is_some_and(|d| !d.read_only),
                egui::Button::new("复制为新模板 JSON"),
            )
            .clicked()
            {
                *action = Some(Action::Copy(id));
            }
        }
        if compact {
            ui.menu_button("模板操作…", |ui| {
                secondary_actions(ui, state, index, action)
            });
        } else {
            secondary_actions(ui, state, index, action);
        }
    });
    ui.horizontal_wrapped(|ui| {
        let stale = original_changed(state, index);
        let ready = !state.editor.trim().is_empty()
            && !state.has_property_input()
            && !state.ime_composing
            && !stale;
        if theme::add_enabled(ui, ready, theme::primary("预览导入 / 替换")).clicked() {
            let result = project.template_mutation_from_draft(&state.draft());
            set_mutation(state, result, mutation);
        }
        if state.mode == Mode::Json {
            if theme::add_enabled(ui, ready, egui::Button::new("按 JSON 身份导入 / 替换"))
                .on_hover_text("显式使用 JSON 中的 ID；改变 ID 会导入另一模板，原注册保持不变")
                .clicked()
            {
                let result = project.template_mutation_from_bytes(state.editor.as_bytes());
                set_mutation(state, result, mutation);
            }
            let broken = state
                .existing_id
                .as_ref()
                .and_then(|id| index.projects.get(id))
                .is_some_and(|document| {
                    !document.diagnostics.is_empty()
                        && document
                            .diagnostics
                            .iter()
                            .all(|diagnostic| diagnostic.code == "TPL001")
                });
            if broken
                && theme::add_enabled(ui, ready, egui::Button::new("修复坏原文（完整替换）"))
                    .clicked()
            {
                let result = project.template_repair_mutation_from_draft(&state.draft());
                set_mutation(state, result, mutation);
            }
        }
        if let Some(id) = state
            .selected_id
            .as_ref()
            .filter(|id| index.projects.get(*id).is_some_and(|d| !d.read_only))
        {
            if theme::add_enabled(
                ui,
                !state.has_unsubmitted_work(),
                egui::Button::new("预览停用模板（保留对象资料）"),
            )
            .clicked()
            {
                *mutation = Some(ProjectTemplateMutation::Delete { id: id.clone() });
            }
        }
        if let Some(preview) = &state.preview {
            if ui.button("取消预览").clicked() {
                *cancel = true;
            }
            let label = if matches!(preview.mutation, ProjectTemplateMutation::Delete { .. }) {
                "确认停用模板"
            } else if matches!(
                preview.mutation,
                ProjectTemplateMutation::RepairInvalid { .. }
            ) {
                "确认完整替换坏原文"
            } else {
                "应用预览中的模板变更"
            };
            if theme::add_enabled(
                ui,
                preview.complete && !state.ime_composing && !state.has_property_input(),
                theme::primary(label),
            )
            .clicked()
            {
                *apply = true;
            }
        }
    });
    if original_changed(state, index) {
        ui.colored_label(
            theme::GOLD(),
            "原模板已变化，输入已保留。请复制草稿后重新载入并合并，不能覆盖较新的模板。",
        );
    }
    if state
        .preview
        .as_ref()
        .is_some_and(|preview| preview.expected_revision != revision)
    {
        ui.colored_label(
            theme::GOLD(),
            "预览已过期；应用会被 core 拒绝，请重新预览。",
        );
    }
}

pub(super) fn editor(
    ui: &mut egui::Ui,
    state: &mut ManagerState,
    index: &ProjectTemplateIndex,
    content: &worldline_core::CompileResult,
    options: worldline_core::CompileOptions,
) -> Option<ProjectTemplateDraftEdit> {
    if let Some(error) = &state.error {
        ui.colored_label(theme::ERROR(), error);
    }
    if state.has_property_input() {
        ui.colored_label(
            theme::GOLD(),
            "还有属性输入未更新到草稿；预览前请先更新或还原。",
        );
    }
    catalog::details(ui, state, index);
    if state.is_new {
        ui.colored_label(theme::GOLD(), "新模板草稿 · 尚未应用");
    } else if state.has_unsubmitted_work() {
        ui.colored_label(theme::GOLD(), "模板定义有未应用输入");
    }
    ui.horizontal_wrapped(|ui| {
        for (mode, title) in [
            (Mode::Design, "可视字段"),
            (Mode::Trial, "试填预览"),
            (Mode::Json, "高级 JSON"),
        ] {
            if ui.selectable_label(state.mode == mode, title).clicked() {
                if state.composition_busy() {
                    return;
                }
                if state.has_property_input() && state.mode == Mode::Design && mode != Mode::Design
                {
                    state.error = Some("请先更新或还原可视属性输入；输入会一直保留".into());
                } else {
                    state.mode = mode;
                }
            }
        }
    });
    let template = state
        .projection
        .as_ref()
        .filter(|projection| projection.editable)
        .and_then(|projection| projection.template.clone());
    let mut edit = None;
    match state.mode {
        Mode::Json => json_editor(ui, state, index),
        Mode::Design | Mode::Trial => {
            if let Some(template) = template {
                if state.mode == Mode::Design {
                    edit = design::show(ui, state, &template, &content.analysis.catalog, options);
                } else {
                    ui.heading(format!("表单试填 · {}", template.title));
                    ui.label(theme::muted("与实际资料表单共用渲染。以下值只供试填，绝不写入对象；默认提示须显式选择。"));
                    if ui.button("清空试填值").clicked() {
                        state.trial_values.clear();
                    }
                    crate::app::templates::project_template_preview(
                        ui,
                        &template,
                        &content.analysis.catalog,
                        &mut state.trial_values,
                    );
                }
            } else {
                ui.label(theme::muted(
                    "尚无可编辑投影。选择后复制内置模板、新建模板，或在高级 JSON 中修复输入。",
                ));
                if let Some(projection) = &state.projection {
                    for diagnostic in &projection.diagnostics {
                        ui.colored_label(
                            theme::GOLD(),
                            format!("{} · {}", diagnostic.code, diagnostic.message),
                        );
                    }
                }
            }
        }
    }
    if let Some(preview) = &state.preview {
        impact::show(ui, preview, &mut state.impact_page);
    }
    edit
}
fn json_editor(ui: &mut egui::Ui, state: &mut ManagerState, index: &ProjectTemplateIndex) {
    ui.label(theme::muted(
        "JSON 与可视编辑共享同一草稿。切换模式不改原字节；错误输入会完整保留。",
    ));
    let locked = state
        .existing_id
        .as_ref()
        .and_then(|id| index.projects.get(id))
        .is_some_and(|document| {
            document.read_only
                && !document
                    .diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.code == "TPL001")
        });
    ui.add_enabled_ui(!locked, |ui| {
        let response = ui.add(
            egui::TextEdit::multiline(&mut state.editor)
                .id(egui::Id::new("template-manager-json"))
                .desired_rows(16)
                .desired_width(f32::INFINITY)
                .hint_text("粘贴或编辑模板 JSON"),
        );
        if !locked {
            input::register(&response);
        }
        if response.changed() {
            state.invalidate();
            state.metadata = None;
            state.properties = None;
        }
    });
    if locked {
        ui.label("未来格式或必需能力只读，可复制原文；不能在此降级或覆盖。");
    }
    if ui.button("复制当前草稿原文").clicked() {
        ui.ctx().copy_text(state.editor.clone());
    }
}
fn set_mutation(
    state: &mut ManagerState,
    result: Result<ProjectTemplateMutation, String>,
    output: &mut Option<ProjectTemplateMutation>,
) {
    match result {
        Ok(mutation) => *output = Some(mutation),
        Err(error) => state.error = Some(error),
    }
}

fn secondary_actions(
    ui: &mut egui::Ui,
    state: &mut ManagerState,
    index: &ProjectTemplateIndex,
    action: &mut Option<Action>,
) {
    if let Some(id) = state.selected_id.clone() {
        if index.projects.contains_key(&id) && ui.button("载入所选模板 JSON").clicked() {
            *action = Some(Action::Reload(id.clone()));
            ui.close();
        }
        if ui.button("导出 JSON 到剪贴板").clicked() {
            let text = if let Some(document) = index.projects.get(&id) {
                String::from_utf8(document.source_bytes.clone()).ok()
            } else {
                index
                    .builtins
                    .iter()
                    .find(|template| template.id == id)
                    .and_then(|template| serde_json::to_string_pretty(template).ok())
            };
            match text {
                Some(text) => {
                    ui.ctx().copy_text(text);
                    state.error = Some("模板 JSON 已复制到剪贴板。".into());
                }
                None => {
                    state.error = Some(
                        "原文不是有效 UTF-8，不能转为剪贴板文本；原字节仍在完整工程备份中".into(),
                    )
                }
            }
            ui.close();
        }
    }
    if theme::add_enabled(
        ui,
        !state.draft_history.is_empty() && !state.has_property_input(),
        egui::Button::new("撤销草稿编辑"),
    )
    .clicked()
    {
        if let Some(source) = state.draft_history.pop() {
            state.editor = source;
            state.invalidate();
            state.metadata = None;
            state.properties = None;
        }
        ui.close();
    }
}
