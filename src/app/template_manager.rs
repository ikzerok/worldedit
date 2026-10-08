//! 可视模板草稿与高级 JSON 共用 core 原文、投影及受保护的预览/应用。
use super::WorldeditApp;
use crate::theme;
use worldline_core::project_templates::*;
mod catalog;
mod controls;
mod workflow;
use controls::*;
use workflow::*;
mod design;
mod impact;
mod input;
mod inputs;
mod state;
use inputs::{FieldInput, MetadataInput};
pub(super) use state::ManagerState;
use state::{find_field, Action, Mode};

impl WorldeditApp {
    pub(super) fn template_manager_tab(&mut self, ctx: &egui::Context) {
        let Some(snapshot) = self.snapshot.as_ref() else {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label("正在准备当前工程模板目录…");
            });
            return;
        };
        let index = &snapshot.template_index;
        let content = &snapshot.result;
        let mut state = std::mem::take(&mut self.template_manager);
        let _input_scope = input::begin(&mut state, ctx);
        if !state.composition_busy() {
            synchronize_clean_selection(&mut state, &self.project, content, index);
            refresh_projection(&mut state, &self.project, content, self.version);
        }
        let mut action = None;
        let mut edit = None;
        let mut mutation = None;
        let mut apply = false;
        let mut cancel = false;
        egui::CentralPanel::default()
            .frame(theme::panel().fill(theme::BG()))
            .show(ctx, |ui| {
                ui.heading("模板管理");
                ui.label(theme::muted(
                    "定义字段 → 试填表单 → 核对实例影响 → 明确应用；保存全部才写入作品目录。",
                ));
                toolbar(
                    ui,
                    &mut state,
                    index,
                    &self.project,
                    self.map_revision,
                    &mut action,
                    &mut mutation,
                    &mut apply,
                    &mut cancel,
                );
                ui.separator();
                let wide = ui.available_width() >= 1060.0;
                if wide {
                    egui::SidePanel::left("template-designer-directory")
                        .resizable(true)
                        .default_width(240.0)
                        .width_range(190.0..=340.0)
                        .show_inside(ui, |ui| {
                            if let Some(next) = catalog::show(ui, &mut state, index) {
                                action = Some(next);
                            }
                        });
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show_inside(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt("template-manager-editor")
                                .show(ui, |ui| {
                                    edit = editor(
                                        ui,
                                        &mut state,
                                        index,
                                        content,
                                        self.project.compile_options(),
                                    );
                                });
                        });
                } else {
                    ui.toggle_value(&mut state.catalog_open, "模板目录 · 选择内置或工程模板");
                    if state.catalog_open {
                        egui::ScrollArea::vertical()
                            .id_salt("template-narrow-catalog")
                            .max_height(220.0)
                            .show(ui, |ui| {
                                if let Some(next) = catalog::show(ui, &mut state, index) {
                                    action = Some(next);
                                }
                            });
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("template-manager-editor")
                        .show(ui, |ui| {
                            edit = editor(
                                ui,
                                &mut state,
                                index,
                                content,
                                self.project.compile_options(),
                            );
                        });
                }
            });
        if let Some(next) = action.take().and_then(|action| state.request(action)) {
            perform_action(&mut state, next, &self.project, content, index);
        }
        confirmation_dialogs(ctx, &mut state, &mut action, &mut edit);
        if let Some(next) = action {
            perform_action(&mut state, next, &self.project, content, index);
        }
        if let Some(edit) = edit {
            apply_visual_edit(&mut state, &self.project, content, edit);
        }
        if cancel {
            state.preview = None;
            state.error = None;
        }
        if let Some(mutation) = mutation {
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
                    state.impact_page = 0;
                    state.error = None;
                    self.io_error = None;
                }
                Err(error) => {
                    state.preview = None;
                    state.error = Some(error.clone());
                    self.io_error = Some(error);
                }
            }
        }
        if apply {
            self.apply_template_preview(&mut state);
        }
        self.template_manager = state;
    }

    fn apply_template_preview(&mut self, state: &mut ManagerState) {
        let Some(preview) = state.preview.clone() else {
            return;
        };
        let (id, deleting) = match &preview.mutation {
            ProjectTemplateMutation::Import { id, .. }
            | ProjectTemplateMutation::Replace { id, .. }
            | ProjectTemplateMutation::RepairInvalid { id, .. } => (id.clone(), false),
            ProjectTemplateMutation::Delete { id } => (id.clone(), true),
        };
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
                if deleting {
                    state.discard();
                } else if let Some(snapshot) = &self.snapshot {
                    let trial = std::mem::take(&mut state.trial_values);
                    let ids = state.reserved_field_ids.clone();
                    let keys = state.reserved_keys.clone();
                    let mode = state.mode;
                    match self.project.template_draft(
                        ProjectTemplateDraftSource::Existing { id: id.clone() },
                        &snapshot.result,
                    ) {
                        Ok(projection) => {
                            let _ = state.install(projection, false);
                            state.selected_id = Some(id);
                            state.reserved_field_ids.extend(ids);
                            state.reserved_field_ids.sort();
                            state.reserved_field_ids.dedup();
                            state.reserved_keys.extend(keys);
                            state.reserved_keys.sort();
                            state.reserved_keys.dedup();
                            state.mode = mode;
                            state.trial_values = trial;
                        }
                        Err(error) => state.error = Some(error),
                    }
                }
            }
            Err(error) => {
                state.error = Some(error.clone());
                self.io_error = Some(error);
            }
        }
    }
}

#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod tests;
