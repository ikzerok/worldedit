use egui::{RichText, Ui};
#[cfg(test)]
mod appearance_tests;
use std::path::Path;
use worldline_core::localization::{
    LocalizationDiagnostic, LocalizationExchange, LocalizationExportPlan, LocalizationImportPlan,
    LocalizationPart, LocalizationSelection, LocalizationSource,
};
use worldline_core::project::Project;

#[derive(Default)]
pub(super) struct LocalizationUiState {
    pub(super) source_locale: String,
    pub(super) target_locale: String,
    pub(super) string_ids: String,
    pub(super) exchange_json: String,
    pub(super) export_plan: Option<LocalizationExportPlan>,
    pub(super) import_plan: Option<LocalizationImportPlan>,
    import_exchange: Option<LocalizationExchange>,
    status: Option<Result<String, String>>,
    confirm_apply: bool,
    exchange_submitted: bool,
    config_submitted: bool,
    navigation: Option<LocalizationSource>,
}

impl LocalizationUiState {
    pub(super) fn has_unsubmitted_work(&self) -> bool {
        ((!self.source_locale.trim().is_empty()
            || !self.target_locale.trim().is_empty()
            || !self.string_ids.trim().is_empty())
            && !self.config_submitted)
            || (!self.exchange_json.trim().is_empty() && !self.exchange_submitted)
            || self.confirm_apply
    }

    pub(super) fn take_navigation(&mut self) -> Option<LocalizationSource> {
        self.navigation.take()
    }

    pub(super) fn set_import_failure(&mut self, message: String) {
        self.import_plan = None;
        self.import_exchange = None;
        self.confirm_apply = false;
        self.exchange_submitted = false;
        self.status = Some(Err(message));
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn load_browser_files(&mut self, files: crate::web::Files) {
        if files.len() != 1 {
            self.set_import_failure("请选择一个 JSON 本地化交换文件".into());
            return;
        }
        let Some((path, bytes)) = files.into_iter().next() else {
            self.set_import_failure("未选择本地化交换文件".into());
            return;
        };
        self.load_import_file(&path, bytes);
    }

    fn load_import_file(&mut self, path: &Path, bytes: Vec<u8>) {
        if !path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            self.set_import_failure("本地化交换文件必须使用 .json 扩展名".into());
            return;
        }
        match String::from_utf8(bytes) {
            Ok(json) => {
                self.exchange_json = json;
                self.import_plan = None;
                self.import_exchange = None;
                self.confirm_apply = false;
                self.exchange_submitted = false;
                self.status = Some(Ok(format!("已载入交换文件：{}", path.display())));
            }
            Err(error) => self.set_import_failure(format!("本地化交换文件不是 UTF-8：{error}")),
        }
    }

    fn selection(&self) -> LocalizationSelection {
        LocalizationSelection {
            schema_version: 1,
            source_locale: self.source_locale.trim().to_owned(),
            target_locale: self.target_locale.trim().to_owned(),
            string_ids: self
                .string_ids
                .lines()
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
                .collect(),
        }
    }
}

/// Render the project-backed panel. Returns true only after a successful core apply.
pub(super) fn show(ui: &mut Ui, project: &mut Project, state: &mut LocalizationUiState) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    let mut applied = false;
    #[cfg(target_arch = "wasm32")]
    let applied = false;
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading("本地化工作台");
        ui.label("只处理明确选择的字符串 ID；提取、校验与持久化均由 worldline-core 完成。");
        ui.horizontal(|ui| {
            ui.label("源语言");
            let source_changed = ui
                .add(egui::TextEdit::singleline(&mut state.source_locale).desired_width(110.0))
                .changed();
            ui.label("目标语言");
            let target_changed = ui
                .add(egui::TextEdit::singleline(&mut state.target_locale).desired_width(110.0))
                .changed();
            if source_changed || target_changed {
                invalidate_selection(state);
            }
        });
        ui.label("导出白名单（每行一个 ID；不会自动扩展到引用或相邻台词）");
        let ids_changed = ui
            .add(
                egui::TextEdit::multiline(&mut state.string_ids)
                    .desired_rows(3)
                    .hint_text("例如：welcome\nreply"),
            )
            .changed();
        if ids_changed {
            invalidate_selection(state);
        }
        ui.horizontal(|ui| {
            if ui.button("预览导出").clicked() {
                preview_export(project, state);
            }
            let can_export = state
                .export_plan
                .as_ref()
                .is_some_and(|plan| plan.can_export);
            if crate::theme::add_enabled(ui, can_export, egui::Button::new("导出 UTF-8 JSON…"))
                .clicked()
            {
                export_exchange(ui, project, state);
            }
        });
        if let Some(plan) = &state.export_plan {
            show_export_plan(ui, plan, &mut state.navigation);
        }

        ui.separator();
        ui.heading("导入译文");
        #[cfg(target_arch = "wasm32")]
        ui.label("当前配对 core 未提供 wasm 原子应用接口；Web 仅可预览，不会写入工程。");
        ui.horizontal(|ui| {
            if ui.button("选择 JSON 交换文件…").clicked() {
                choose_import_file(ui, state);
            }
            if ui.button("预览导入").clicked() {
                preview_import(project, state);
            }
        });
        let exchange_changed = ui
            .add(
                egui::TextEdit::multiline(&mut state.exchange_json)
                    .code_editor()
                    .desired_rows(10)
                    .hint_text("选择 .json 交换文件，或在此粘贴/编辑 UTF-8 JSON"),
            )
            .changed();
        if exchange_changed {
            state.import_plan = None;
            state.import_exchange = None;
            state.confirm_apply = false;
            state.exchange_submitted = false;
            state.status = None;
        }
        if state.exchange_json.is_empty() {
            state.import_plan = None;
            state.import_exchange = None;
            state.confirm_apply = false;
            state.exchange_submitted = false;
        }
        if let Some(exchange) = &state.import_exchange {
            show_exchange_entries(ui, exchange, &mut state.navigation);
        }
        if let Some(plan) = &state.import_plan {
            show_import_plan(ui, plan, &mut state.navigation);
            if plan.can_apply {
                #[cfg(not(target_arch = "wasm32"))]
                if ui.button("复核通过 · 确认导入…").clicked() {
                    state.confirm_apply = true;
                }
                #[cfg(target_arch = "wasm32")]
                {
                    crate::theme::add_enabled(
                        ui,
                        false,
                        egui::Button::new("浏览器端暂不可应用译文"),
                    );
                }
            }
        }
        show_status(ui, state);
    });

    #[cfg(not(target_arch = "wasm32"))]
    if state.confirm_apply {
        applied = confirm_and_apply(ui.ctx(), project, state);
    }
    #[cfg(target_arch = "wasm32")]
    if state.confirm_apply {
        state.confirm_apply = false;
    }
    applied
}

fn invalidate_selection(state: &mut LocalizationUiState) {
    state.config_submitted = false;
    state.export_plan = None;
    state.import_plan = None;
    state.import_exchange = None;
    state.confirm_apply = false;
    state.status = None;
}

fn preview_export(project: &Project, state: &mut LocalizationUiState) {
    match project.preview_localization_export(&state.selection()) {
        Ok(plan) => {
            state.status = Some(Ok(if plan.can_export {
                "导出预览通过；尚未创建文件。".into()
            } else {
                "导出预览包含诊断，不能导出。".into()
            }));
            state.export_plan = Some(plan);
        }
        Err(message) => {
            state.export_plan = None;
            state.status = Some(Err(message));
        }
    }
}

fn show_export_plan(
    ui: &mut Ui,
    plan: &LocalizationExportPlan,
    navigation: &mut Option<LocalizationSource>,
) {
    ui.group(|ui| {
        ui.label(if plan.can_export {
            "导出状态：可导出"
        } else {
            "导出状态：已阻止"
        });
        ui.label(format!("字符串协议版本：{}", plan.schema_version));
        ui.label(format!("源码基线：{}", plan.exchange.source_baseline));
        for entry in &plan.exchange.entries {
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(&entry.id);
                    if ui
                        .link(format!(
                            "定位来源 {}:{} · {}",
                            entry.source.file, entry.source.line, entry.source.kind
                        ))
                        .clicked()
                    {
                        *navigation = Some(entry.source.clone());
                    }
                });
                ui.label("源文：");
                show_parts(ui, &entry.source_parts);
            });
        }
        show_diagnostics(ui, &plan.diagnostics, navigation);
    });
}

fn show_exchange_entries(
    ui: &mut Ui,
    exchange: &LocalizationExchange,
    navigation: &mut Option<LocalizationSource>,
) {
    ui.group(|ui| {
        ui.strong(format!(
            "交换包 · {} → {} · schema {}",
            exchange.source_locale, exchange.target_locale, exchange.schema_version
        ));
        for entry in &exchange.entries {
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(&entry.id);
                    if ui
                        .link(format!(
                            "定位来源 {}:{} · {}",
                            entry.source.file, entry.source.line, entry.source.kind
                        ))
                        .clicked()
                    {
                        *navigation = Some(entry.source.clone());
                    }
                    ui.label(if entry.translation_parts.is_some() {
                        "译文已提供"
                    } else {
                        "缺译文"
                    });
                });
                ui.label("源文：");
                show_parts(ui, &entry.source_parts);
                if let Some(parts) = &entry.translation_parts {
                    ui.label("译文：");
                    show_parts(ui, parts);
                }
            });
        }
    });
}

fn show_parts(ui: &mut Ui, parts: &[LocalizationPart]) {
    ui.horizontal_wrapped(|ui| {
        for part in parts {
            match part {
                LocalizationPart::Text { text } => {
                    ui.label(text);
                }
                LocalizationPart::Placeholder { token } => {
                    ui.label("占位符");
                    ui.colored_label(crate::theme::resolved(ui.ctx()).colors.info, token);
                }
                LocalizationPart::Link { label, .. } => {
                    ui.label("链接");
                    ui.colored_label(crate::theme::resolved(ui.ctx()).colors.info, label);
                }
            }
        }
    });
}

fn preview_import(project: &Project, state: &mut LocalizationUiState) {
    state.import_plan = None;
    state.import_exchange = None;
    state.confirm_apply = false;
    state.exchange_submitted = false;
    let exchange = match LocalizationExchange::from_json_bytes(state.exchange_json.as_bytes()) {
        Ok(exchange) => exchange,
        Err(message) => {
            state.status = Some(Err(message));
            return;
        }
    };
    match project.preview_localization_import(&state.selection(), &exchange) {
        Ok(plan) => {
            state.status = Some(Ok(if plan.can_apply {
                "导入预览通过；工程尚未修改，请复核后确认。".into()
            } else {
                "已生成导入预览，但核心诊断阻止应用。".into()
            }));
            state.import_exchange = Some(exchange);
            state.import_plan = Some(plan);
        }
        Err(message) => state.status = Some(Err(message)),
    }
}

fn show_import_plan(
    ui: &mut Ui,
    plan: &LocalizationImportPlan,
    navigation: &mut Option<LocalizationSource>,
) {
    ui.group(|ui| {
        ui.label(if plan.can_apply {
            "核心校验：通过"
        } else {
            "核心校验：阻止应用"
        });
        ui.label(format!(
            "目标语言 {} · sidecar {} · 影响 {} 个 ID",
            plan.target_locale,
            plan.sidecar_path,
            plan.affected_ids.len()
        ));
        show_diagnostics(ui, &plan.diagnostics, navigation);
    });
}

fn show_diagnostics(
    ui: &mut Ui,
    diagnostics: &[LocalizationDiagnostic],
    navigation: &mut Option<LocalizationSource>,
) {
    if diagnostics.is_empty() {
        ui.label("诊断：无");
        return;
    }
    ui.strong("诊断");
    for diagnostic in diagnostics {
        ui.group(|ui| {
            ui.colored_label(
                crate::theme::resolved(ui.ctx()).colors.danger,
                format!("{} · {}", diagnostic.code, diagnostic.message),
            );
            if let Some(id) = &diagnostic.id {
                ui.label(format!("ID：{id}"));
            }
            if let Some(source) = &diagnostic.source {
                if ui
                    .link(format!(
                        "定位诊断 {}:{} · {}",
                        source.file, source.line, source.kind
                    ))
                    .clicked()
                {
                    *navigation = Some(source.clone());
                }
            }
        });
    }
}

fn show_status(ui: &mut Ui, state: &LocalizationUiState) {
    if let Some(status) = &state.status {
        match status {
            Ok(message) => {
                ui.label(
                    RichText::new(message).color(crate::theme::resolved(ui.ctx()).colors.success),
                );
            }
            Err(message) => {
                ui.label(
                    RichText::new(message).color(crate::theme::resolved(ui.ctx()).colors.danger),
                );
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn choose_import_file(_ui: &mut Ui, state: &mut LocalizationUiState) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("本地化交换包（JSON）", &["json"])
        .pick_file()
    else {
        return;
    };
    match std::fs::read(&path) {
        Ok(bytes) => state.load_import_file(&path, bytes),
        Err(error) => state.set_import_failure(format!("无法读取本地化交换文件：{error}")),
    }
}

#[cfg(target_arch = "wasm32")]
fn choose_import_file(ui: &mut Ui, _state: &mut LocalizationUiState) {
    crate::web::select_files(
        ui.ctx(),
        false,
        ".json,application/json",
        crate::web::FileAction::LocalizationImport,
    );
}

fn export_exchange(ui: &mut Ui, project: &Project, state: &mut LocalizationUiState) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("本地化交换包（JSON）", &["json"])
            .set_file_name(format!(
                "localization-{}-{}.json",
                state.source_locale.trim(),
                state.target_locale.trim()
            ))
            .save_file()
        else {
            return;
        };
        let selection = state.selection();
        let result = state
            .export_plan
            .as_ref()
            .map(|plan| project.export_localization(&selection, &plan.plan_digest, &path));
        state.status = Some(match result {
            Some(Ok(_)) => Ok(format!("已导出 UTF-8 交换文件：{}", path.display())),
            Some(Err(message)) => Err(message),
            None => Err("请先生成导出预览".into()),
        });
    }
    #[cfg(target_arch = "wasm32")]
    {
        let Some(plan) = &state.export_plan else {
            state.status = Some(Err("请先生成导出预览".into()));
            return;
        };
        let selection = state.selection();
        let current = project.preview_localization_export(&selection);
        let result = match current {
            Ok(current) if current.plan_digest == plan.plan_digest && current.can_export => {
                serde_json::to_vec(&current.exchange)
                    .map_err(|error| format!("无法序列化本地化交换包：{error}"))
                    .and_then(|bytes| {
                        crate::web::download(
                            &format!(
                                "localization-{}-{}.json",
                                state.source_locale.trim(),
                                state.target_locale.trim()
                            ),
                            &bytes,
                            "application/json;charset=utf-8",
                        )
                    })
            }
            Ok(_) => Err("本地化导出预览已过期或包含诊断，请重新预览".into()),
            Err(message) => Err(message),
        };
        state.status = Some(result.map(|()| "已交给浏览器下载 UTF-8 JSON 交换文件。".into()));
    }
    let _ = ui;
}

#[cfg(not(target_arch = "wasm32"))]
fn confirm_and_apply(
    ctx: &egui::Context,
    project: &mut Project,
    state: &mut LocalizationUiState,
) -> bool {
    let mut apply = false;
    egui::Window::new("确认本地化导入")
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("核心预览已通过。确认后将通过 Project 事务更新目标语言 sidecar。");
            if let Some(plan) = &state.import_plan {
                ui.label(format!(
                    "{} · {} 个字符串 · {}",
                    plan.target_locale,
                    plan.affected_ids.len(),
                    plan.sidecar_path
                ));
            }
            ui.horizontal(|ui| {
                if ui.button("取消导入").clicked() {
                    state.confirm_apply = false;
                }
                if ui.button("确认并原子导入").clicked() {
                    apply = true;
                }
            });
        });
    if !apply {
        return false;
    }
    let selection = state.selection();
    let result = match (&state.import_plan, &state.import_exchange) {
        (Some(plan), Some(exchange)) => {
            project.apply_localization_import(&selection, exchange, &plan.plan_digest)
        }
        _ => Err("导入预览已失效，请重新预览".into()),
    };
    state.confirm_apply = false;
    match result {
        Ok(result) => {
            state.exchange_submitted = true;
            state.config_submitted = true;
            state.import_plan = None;
            state.import_exchange = None;
            state.status = Some(Ok(format!(
                "已原子应用 {} 个字符串；新工程基线 {}。",
                result.plan.affected_ids.len(),
                result.new_baseline
            )));
            true
        }
        Err(message) => {
            state.status = Some(Err(message));
            false
        }
    }
}
