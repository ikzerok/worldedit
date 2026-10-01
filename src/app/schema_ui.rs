//! 持续资料约束的源码编辑与 core 影响预览，不另写解析器。
use super::WorldeditApp;
use crate::theme;
use std::path::PathBuf;
use worldline_core::{schemas::SchemaEditPreview, source_edit::SourceEditRequest};

#[derive(Default)]
pub(super) struct SchemaUiState {
    pub open: bool,
    pub return_focus: Option<egui::Id>,
    path: Option<PathBuf>,
    source: String,
    original: String,
    baseline: String,
    preview: Option<SchemaEditPreview>,
    error: Option<String>,
    discard_confirm: bool,
}

impl SchemaUiState {
    pub(super) fn draft_path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    pub(super) fn has_unsubmitted_work(&self) -> bool {
        self.source != self.original
    }
    pub(super) fn close(&mut self, ctx: &egui::Context) {
        self.open = false;
        if let Some(id) = self.return_focus.take() {
            ctx.memory_mut(|memory| memory.request_focus(id));
        }
    }
}

impl WorldeditApp {
    pub(super) fn open_schema_editor(&mut self, ctx: &egui::Context) {
        self.schema_ui.return_focus = ctx.memory(|memory| memory.focused());
        self.schema_ui.open = true;
        if !self.schema_ui.has_unsubmitted_work() {
            self.load_schema_file(
                self.schema_ui
                    .path
                    .clone()
                    .unwrap_or_else(|| self.active_file.clone()),
            );
        }
    }

    fn load_schema_file(&mut self, path: PathBuf) {
        match self.project.document(&path) {
            Ok(source) => {
                self.schema_ui.source = source.to_owned();
                self.schema_ui.original = source.to_owned();
                self.schema_ui.path = Some(path);
                self.schema_ui.baseline = self.project.content_baseline();
                self.schema_ui.preview = None;
                self.schema_ui.error = None;
            }
            Err(error) => self.schema_ui.error = Some(error),
        }
    }

    pub(super) fn schema_editor_window(&mut self, ctx: &egui::Context) {
        if !self.schema_ui.open {
            return;
        }
        if !self.schema_ui.has_unsubmitted_work()
            && self.schema_ui.baseline != self.project.content_baseline()
        {
            self.load_schema_file(
                self.schema_ui
                    .path
                    .clone()
                    .unwrap_or_else(|| self.active_file.clone()),
            );
        }
        let index = self.project.schema_index();
        let writable = self.project.language_version_kind().supports_language_112()
            && self.project.authoring_diagnostics().is_empty();
        let files: Vec<_> = self
            .project
            .documents
            .iter()
            .filter(|(_, document)| !document.is_deleted())
            .map(|(path, _)| path.clone())
            .collect();
        let root = self.project.root.clone();
        let mut state = std::mem::take(&mut self.schema_ui);
        let mut open = state.open;
        let mut switch = None;
        let mut do_preview = false;
        let mut do_apply = false;
        let mut discard = false;
        egui::Window::new("持续资料约束 · schema")
            .id(egui::Id::new("persistent-schema-editor"))
            .open(&mut open)
            .default_size(egui::vec2(900.0, 600.0))
            .max_size(
                (ctx.screen_rect().size() - egui::vec2(40.0, 90.0)).max(egui::vec2(320.0, 220.0)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("schema-window-scroll")
                    .show(ui, |ui| {
                        ui.label(
                            "显式 schema + bind 持续校验资料；不会替实例填值、强转或自动改键。",
                        );
                        if !writable {
                            ui.colored_label(
                                theme::ERROR(),
                                "需显式语言1.12及以上且工作区能力受支持；当前仅可查看，不会自动升级。",
                            );
                        }
                        ui.horizontal_wrapped(|ui| {
                            ui.label("源码文件");
                            egui::ComboBox::from_id_salt("schema-source-file")
                                .selected_text(
                                    state
                                        .path
                                        .as_ref()
                                        .map(|path| {
                                            path.strip_prefix(&root)
                                                .unwrap_or(path)
                                                .display()
                                                .to_string()
                                        })
                                        .unwrap_or_default(),
                                )
                                .show_ui(ui, |ui| {
                                    for path in &files {
                                        let label = path
                                            .strip_prefix(&root)
                                            .unwrap_or(path)
                                            .display()
                                            .to_string();
                                        if ui
                                            .add_enabled(
                                                !state.has_unsubmitted_work(),
                                                egui::Button::selectable(
                                                    state.path.as_ref() == Some(path),
                                                    label,
                                                ),
                                            )
                                            .clicked()
                                        {
                                            switch = Some(path.clone());
                                        }
                                    }
                                });
                            if state.has_unsubmitted_work() {
                                ui.label(theme::muted("未应用草稿；切换文件前先应用或明确丢弃"));
                            }
                        });
                        egui::CollapsingHeader::new(format!(
                            "当前约束索引 · {} schema / {} 绑定",
                            index.schemas.len(),
                            index.bindings.len()
                        ))
                        .show(ui, |ui| {
                            for schema in &index.schemas {
                                ui.label(format!(
                                    "{} → {}{} · {}",
                                    schema.id,
                                    schema.kind,
                                    schema
                                        .entity_type
                                        .as_ref()
                                        .map(|value| format!("/{value}"))
                                        .unwrap_or_default(),
                                    if schema.closed {
                                        "closed"
                                    } else {
                                        "开放字段集"
                                    }
                                ));
                                for field in &schema.fields {
                                    ui.label(format!(
                                        "  {} · {} · {:?}{}",
                                        field.id,
                                        field.key,
                                        field.value_type,
                                        if field.required { " · required" } else { "" }
                                    ));
                                }
                            }
                            for binding in &index.bindings {
                                ui.label(format!(
                                    "{}:{} → {}",
                                    binding.target.kind, binding.target.id, binding.schema_id
                                ));
                            }
                        });
                        ui.label(
                            "完整源码草稿（稳定 schema / field ID 由作者保留；变更后先预览影响）",
                        );
                        let source_id = egui::Id::new("schema-source-editor");
                        super::writing_workspace::prepare_text_undo(
                            ui.ctx(),
                            source_id,
                            &state.source,
                        );
                        let response = ui.add_enabled(
                            writable,
                            egui::TextEdit::multiline(&mut state.source)
                                .id(source_id)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .desired_rows(13),
                        );
                        super::writing_workspace::remember_text_undo(
                            ui.ctx(),
                            source_id,
                            &state.source,
                        );
                        if response.changed() {
                            state.preview = None;
                            state.error = None;
                        }
                        if state.baseline != self.project.content_baseline() {
                            ui.colored_label(
                                theme::ERROR(),
                                "工程基线已变化；旧草稿保留，应用将被拒绝。",
                            );
                        }
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add_enabled(
                                    writable && state.path.is_some(),
                                    egui::Button::new("预览约束影响"),
                                )
                                .clicked()
                            {
                                do_preview = true;
                            }
                            if ui
                                .add_enabled(
                                    writable
                                        && state.preview.as_ref().is_some_and(|plan| plan.changed),
                                    egui::Button::new("应用约束草稿"),
                                )
                                .clicked()
                            {
                                do_apply = true;
                            }
                            if ui.button("取消影响预览").clicked() {
                                state.preview = None;
                            }
                            if ui
                                .add_enabled(
                                    state.has_unsubmitted_work(),
                                    egui::Button::new("丢弃约束草稿"),
                                )
                                .clicked()
                            {
                                state.discard_confirm = true;
                            }
                        });
                        if state.discard_confirm {
                            ui.colored_label(
                                theme::ERROR(),
                                "仅丢弃此窗口未应用输入，工程原文保持不变。",
                            );
                            ui.horizontal(|ui| {
                                if ui.button("确认丢弃约束草稿").clicked() {
                                    discard = true;
                                }
                                if ui.button("保留输入").clicked() {
                                    state.discard_confirm = false;
                                }
                            });
                        }
                        if let Some(error) = &state.error {
                            ui.colored_label(theme::ERROR(), error);
                        }
                        if let Some(plan) = &state.preview {
                            draw_plan(ui, plan);
                        }
                        ui.separator();
                        ui.label("当前工程诊断");
                        for diagnostic in &index.diagnostics {
                            ui.colored_label(
                                theme::ERROR(),
                                format!(
                                    "{}:{} {} {}",
                                    diagnostic.file,
                                    diagnostic.span.line,
                                    diagnostic.code,
                                    diagnostic.message
                                ),
                            );
                        }
                    });
            });
        state.open = open;
        if !open {
            state.close(ctx);
        }
        let request = state
            .path
            .as_ref()
            .and_then(|path| path.strip_prefix(&self.project.root).ok())
            .map(|path| SourceEditRequest {
                schema_version: 1,
                path: path.to_owned(),
                expected_baseline: state.baseline.clone(),
                source: state.source.clone(),
            });
        if do_preview {
            if let Some(request) = &request {
                match self.project.preview_schema_edit(request) {
                    Ok(plan) => {
                        state.preview = Some(plan);
                        state.error = None;
                    }
                    Err(error) => state.error = Some(error),
                }
            }
        }
        if do_apply {
            if let (Some(request), Some(plan)) = (&request, &state.preview) {
                let before = self.project.clone();
                match self.project.apply_schema_edit(request, &plan.plan_digest) {
                    Ok(_) => {
                        self.remember(before);
                        self.recompile();
                        state.original = state.source.clone();
                        state.baseline = self.project.content_baseline();
                        state.preview = None;
                        state.error = None;
                        self.message = Some("约束草稿已应用；可撤销，尚需保存工程".into());
                    }
                    Err(error) => state.error = Some(error),
                }
            }
        }
        if discard {
            switch = state.path.clone();
            state.discard_confirm = false;
        }
        self.schema_ui = state;
        if let Some(path) = switch {
            self.load_schema_file(path);
        }
    }
}

fn draw_plan(ui: &mut egui::Ui, plan: &SchemaEditPreview) {
    ui.separator();
    ui.label(format!(
        "影响预览：{} 项字段/约束变化，{} 个实例",
        plan.field_changes.len(),
        plan.instance_impacts.len()
    ));
    if !plan.complete {
        ui.colored_label(
            theme::ERROR(),
            "源码或绑定尚不完整，影响列表不能视为全部；允许保留错误草稿，发布前必须修复。",
        );
    }
    for change in &plan.field_changes {
        ui.label(format!(
            "{} / {} · {}",
            change.schema_id,
            change.field_id.as_deref().unwrap_or("schema"),
            change.change
        ));
        if let Some(before) = &change.before {
            ui.monospace(format!(
                "原：{} {:?} required={}",
                before.key, before.value_type, before.required
            ));
        }
        if let Some(after) = &change.after {
            ui.monospace(format!(
                "新：{} {:?} required={}",
                after.key, after.value_type, after.required
            ));
        }
    }
    for impact in &plan.instance_impacts {
        ui.label(format!(
            "{}:{} · {:?} → {:?} · 诊断 {} → {}",
            impact.target.kind,
            impact.target.id,
            impact.before_schema_ids,
            impact.after_schema_ids,
            impact.before_diagnostics.len(),
            impact.after_diagnostics.len()
        ));
        for diagnostic in &impact.after_diagnostics {
            ui.colored_label(
                theme::ERROR(),
                format!("{} {}", diagnostic.code, diagnostic.message),
            );
        }
    }
    for diagnostic in &plan.after_diagnostics {
        ui.colored_label(
            theme::ERROR(),
            format!(
                "{}:{} {} {}",
                diagnostic.file, diagnostic.span.line, diagnostic.code, diagnostic.message
            ),
        );
    }
}

#[cfg(test)]
mod tests;
