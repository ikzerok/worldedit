use super::*;

pub(super) fn refresh_projection(
    state: &mut ManagerState,
    project: &worldline_core::project::Project,
    content: &worldline_core::CompileResult,
    version: u64,
) {
    if state.editor.is_empty() {
        return;
    }
    if state.projection_version == Some(version) && state.projection.is_some() {
        return;
    }
    let projection = project.project_template_draft_projection(&state.draft(), content);
    if !state.metadata.as_ref().is_some_and(MetadataInput::changed) {
        state.metadata = projection.template.as_ref().map(MetadataInput::new);
    }
    if !state.properties.as_ref().is_some_and(FieldInput::changed) {
        state.properties = state.selected_field.as_deref().and_then(|id| {
            find_field(&projection.template.as_ref()?.fields, id).map(FieldInput::new)
        });
    }
    state.projection = Some(projection);
    state.projection_version = Some(version);
}
pub(super) fn perform_action(
    state: &mut ManagerState,
    action: Action,
    project: &worldline_core::project::Project,
    content: &worldline_core::CompileResult,
    index: &ProjectTemplateIndex,
) {
    let (source, selection, is_new, mode) = match action {
        Action::New => (ProjectTemplateDraftSource::New, None, true, Mode::Design),
        Action::Copy(id) => (
            ProjectTemplateDraftSource::Copy { id },
            None,
            true,
            Mode::Design,
        ),
        Action::Select(id) if !index.projects.contains_key(&id) => {
            state.discard();
            state.selected_id = Some(id);
            state.mode = Mode::Design;
            return;
        }
        Action::Select(id) => (
            ProjectTemplateDraftSource::Existing { id: id.clone() },
            Some(id),
            false,
            Mode::Design,
        ),
        Action::Reload(id) => (
            ProjectTemplateDraftSource::Existing { id: id.clone() },
            Some(id),
            false,
            Mode::Json,
        ),
    };
    match project.template_draft(source, content) {
        Ok(projection) => match state.install(projection, is_new) {
            Ok(()) => {
                state.selected_id = selection;
                state.mode = mode;
            }
            Err(error) => {
                if !state.has_unsubmitted_work() {
                    state.discard();
                    state.selected_id = selection;
                    state.mode = Mode::Json;
                }
                state.error = Some(error);
            }
        },
        Err(error) => state.error = Some(error),
    }
}
pub(super) fn apply_visual_edit(
    state: &mut ManagerState,
    project: &worldline_core::project::Project,
    content: &worldline_core::CompileResult,
    edit: ProjectTemplateDraftEdit,
) {
    if state.properties.as_ref().is_some_and(FieldInput::changed)
        && matches!(
            edit,
            ProjectTemplateDraftEdit::AddField { .. }
                | ProjectTemplateDraftEdit::MoveField { .. }
                | ProjectTemplateDraftEdit::DeleteField { .. }
        )
    {
        state.error = Some("请先更新字段草稿或还原字段输入，再调整字段结构；输入已保留".into());
        return;
    }
    let metadata = state.metadata.clone().filter(|input| {
        input.changed() && !matches!(edit, ProjectTemplateDraftEdit::SetMetadata { .. })
    });
    let properties = state.properties.clone().filter(|input| {
        input.changed() && !matches!(edit, ProjectTemplateDraftEdit::UpdateField { .. })
    });
    if state.ime_composing {
        state.error = Some("请先完成或取消输入法组合；当前输入已保留".into());
        return;
    }
    match project.edit_template_draft(&state.draft(), &edit, content) {
        Ok(projection) => {
            state.accept_edit(projection);
            if metadata.is_some() {
                state.metadata = metadata;
            }
            if properties.is_some() {
                state.properties = properties;
            }
        }
        Err(error) => state.error = Some(error),
    }
}
pub(super) fn confirmation_dialogs(
    ctx: &egui::Context,
    state: &mut ManagerState,
    action: &mut Option<Action>,
    edit: &mut Option<ProjectTemplateDraftEdit>,
) {
    *action = None;
    if state.pending.is_some() {
        egui::Window::new("保留当前模板输入？")
            .id(egui::Id::new("template-discard-confirm"))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(
                    "当前模板定义或属性输入尚未应用。继续编辑，或明确丢弃后切换；试填会重置。",
                );
                ui.horizontal_wrapped(|ui| {
                    if ui.button("继续编辑当前模板").clicked() {
                        state.pending = None;
                    }
                    if theme::add_enabled(
                        ui,
                        !state.ime_composing,
                        egui::Button::new("丢弃模板输入并继续"),
                    )
                    .clicked()
                    {
                        *action = state.pending.take();
                    }
                });
            });
    }
    if let Some(id) = state.remove_field.clone() {
        egui::Window::new("删除字段定义？")
            .id(egui::Id::new("template-delete-field"))
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!(
                    "删除 {id} 的定义；如为分组，其子字段定义一并删除。实例属性保持原样。"
                ));
                ui.horizontal_wrapped(|ui| {
                    if ui.button("取消删除字段").clicked() {
                        state.remove_field = None;
                    }
                    if theme::add_enabled(
                        ui,
                        !state.ime_composing,
                        egui::Button::new("确认删除字段定义"),
                    )
                    .clicked()
                    {
                        *edit = Some(ProjectTemplateDraftEdit::DeleteField {
                            field_id: id.clone(),
                        });
                        state.remove_field = None;
                    }
                });
            });
    }
}
pub(super) fn original_changed(state: &ManagerState, index: &ProjectTemplateIndex) -> bool {
    state.existing_id.as_ref().is_some_and(|id| {
        index
            .projects
            .get(id)
            .is_none_or(|document| document.source_bytes != state.baseline_editor.as_bytes())
    })
}
pub(super) fn synchronize_clean_selection(
    state: &mut ManagerState,
    project: &worldline_core::project::Project,
    content: &worldline_core::CompileResult,
    index: &ProjectTemplateIndex,
) {
    if state.has_unsubmitted_work() || !original_changed(state, index) {
        return;
    }
    let Some(id) = state.existing_id.clone() else {
        return;
    };
    if !index.projects.contains_key(&id) {
        state.discard();
        return;
    }
    let mode = state.mode;
    let trial = std::mem::take(&mut state.trial_values);
    perform_action(state, Action::Select(id), project, content, index);
    state.mode = mode;
    state.trial_values = trial;
}
