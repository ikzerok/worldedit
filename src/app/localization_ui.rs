//! 目录与逐项译文制作；语义、查询、计划及事务仅使用 core。
use egui::{RichText, Ui};
use std::path::Path;
use worldline_core::localization::{
    LocalizationDiagnostic, LocalizationExchange, LocalizationExportPlan, LocalizationImportPlan,
    LocalizationPart, LocalizationSelection, MAX_LOCALIZATION_JSON_BYTES,
};
use worldline_core::project::Project;
mod catalog;
mod editing;
mod exchange;
mod exchange_view;
mod focus;
mod jobs;
mod json_input;
mod navigation;
mod plans;
mod retained;
use exchange::*;
use focus::RevealFocus;
#[cfg(test)]
mod appearance_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod capability_tests;
#[cfg(test)]
mod focus_tests;
#[cfg(test)]
mod input_guard_tests;
#[cfg(test)]
mod job_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod json_focus_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod keyboard_visibility_tests;
#[cfg(test)]
mod query_status_tests;
#[cfg(test)]
mod retained_page_guard_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod short_viewport_tests;
#[cfg(test)]
mod workbench_tests;

#[derive(Default)]
pub(super) struct LocalizationUiState {
    pub(super) source_locale: String,
    pub(super) target_locale: String,
    pub(super) string_ids: String,
    pub(super) exchange_json: String,
    json_input: json_input::JsonInput,
    export_view: exchange_view::View,
    import_view: exchange_view::View,
    pub(super) export_plan: Option<LocalizationExportPlan>,
    pub(super) import_plan: Option<LocalizationImportPlan>,
    pub(super) advanced: bool,
    import_exchange: Option<LocalizationExchange>,
    // Authoring operations only; catalog failures belong to workbench.query_error.
    status: Option<Result<String, String>>,
    confirm_apply: bool,
    exchange_submitted: bool,
    config_submitted: bool,
    config_edited: bool,
    exchange_locales: Option<(String, String)>,
    navigation: Option<navigation::Request>,
    workbench: catalog::Workbench,
    jobs: jobs::Jobs,
    applied_before: Option<Project>,
    #[cfg(not(target_arch = "wasm32"))]
    import_path: String,
    #[cfg(not(target_arch = "wasm32"))]
    export_path: String,
    pub(super) enable_requested: bool,
    pub(super) return_to_play: bool,
    pub(super) return_requested: bool,
}

impl LocalizationUiState {
    pub(super) fn has_unsubmitted_work(&self) -> bool {
        let (source_locale, target_locale) = self.exchange_locales();
        (((self.config_edited
            && (!source_locale.trim().is_empty() || !target_locale.trim().is_empty()))
            || !self.string_ids.trim().is_empty())
            && !self.config_submitted)
            || (!self.exchange_json.trim_start().is_empty() && !self.exchange_submitted)
            || self.json_input.has_pending()
            || self.confirm_apply
            || self.workbench.has_input()
    }

    pub(super) fn take_navigation(&mut self) -> Option<navigation::Request> {
        self.navigation.take()
    }

    pub(super) fn take_applied_before(&mut self) -> Option<Project> {
        self.applied_before.take()
    }

    pub(super) fn clear_operation_status(&mut self) {
        self.status = None;
    }

    pub(super) fn open_translation(&mut self, locale: &str, id: Option<&str>) {
        self.jobs.cancel();
        self.workbench.preview = None;
        self.import_plan = None;
        self.export_plan = None;
        self.confirm_apply = false;
        self.target_locale = locale.into();
        self.advanced = false;
        self.workbench.focus_id = id.map(str::to_owned);
        self.workbench.runtime_revision = None;
        self.workbench.runtime_revision_mismatch = false;
        self.workbench.search.clear();
        self.workbench.source_prefix.clear();
        self.workbench.status = None;
        self.workbench.kind = None;
        self.workbench.offset = 0;
        self.workbench.detail = true;
        self.workbench.invalidate();
        self.return_to_play = true;
    }

    pub(super) fn expect_runtime_revision(&mut self, revision: String) {
        self.workbench.runtime_revision = Some(revision);
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
            return;
        };
        self.load_import_file(&path, bytes);
    }

    fn load_import_file(&mut self, path: &Path, bytes: Vec<u8>) {
        if !path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        {
            self.set_import_failure("本地化交换文件必须使用 .json 扩展名".into());
            return;
        }
        if bytes.len() > MAX_LOCALIZATION_JSON_BYTES {
            self.set_import_failure("本地化交换文件超过 8 MiB 预算".into());
            return;
        }
        match String::from_utf8(bytes) {
            Ok(json) => {
                self.exchange_json = json;
                self.json_input.reset();
                self.invalidate_import();
                self.advanced = true;
                self.status = Some(Ok(format!("已载入交换文件：{}", path.display())));
            }
            Err(error) => self.set_import_failure(format!("本地化交换文件不是 UTF-8：{error}")),
        }
    }

    #[cfg(test)]
    pub(super) fn has_pending_work(&self) -> bool {
        self.jobs.pending()
    }

    #[cfg(test)]
    pub(super) fn settle_pending_for_test(&mut self, project: &Project, version: u64) {
        jobs::settle(project, self, version);
    }

    fn cancel_preview(&mut self) {
        self.workbench.preview = None;
        self.jobs.cancel_preview();
    }

    fn invalidate_import(&mut self) {
        self.invalidate_import_preview();
        self.exchange_submitted = false;
    }

    fn invalidate_import_preview(&mut self) {
        self.jobs.cancel_preview();
        self.import_plan = None;
        self.import_exchange = None;
        self.confirm_apply = false;
        self.status = None;
    }

    fn exchange_locales(&self) -> (&str, &str) {
        self.exchange_locales.as_ref().map_or(
            (self.source_locale.as_str(), self.target_locale.as_str()),
            |(source, target)| (source.as_str(), target.as_str()),
        )
    }

    fn selection(&self) -> LocalizationSelection {
        let (source_locale, target_locale) = self.exchange_locales();
        LocalizationSelection {
            schema_version: 1,
            source_locale: source_locale.trim().to_owned(),
            target_locale: target_locale.trim().to_owned(),
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

/// `version` is the editor's applied-project generation; stable frames never hash all source bytes.
pub(super) fn show(
    ui: &mut Ui,
    project: &mut Project,
    state: &mut LocalizationUiState,
    version: u64,
) -> bool {
    jobs::pump(ui.ctx(), project, state, version);
    let compact = ui.available_width() < 760.0 || ui.available_height() < 360.0;
    focus::begin(ui.ctx(), compact);
    if compact {
        egui::ScrollArea::vertical()
            .id_salt((
                "localization-workbench",
                state.advanced,
                state.workbench.detail,
            ))
            .auto_shrink([false, false])
            .show(ui, |ui| contents(ui, project, state, version, true));
    } else {
        contents(ui, project, state, version, false);
    }
    let mut applied = false;
    if state.confirm_apply {
        applied |= confirm_and_apply(ui.ctx(), project, state);
    }
    applied |= plans::confirm(ui.ctx(), project, state);
    jobs::pump(ui.ctx(), project, state, version);
    applied
}

fn contents(
    ui: &mut Ui,
    project: &Project,
    state: &mut LocalizationUiState,
    version: u64,
    compact: bool,
) {
    ui.heading("本地化工作台");
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut state.advanced, false, "译文目录")
            .reveal_focus(ui);
        ui.selectable_value(&mut state.advanced, true, "高级 JSON 交换")
            .reveal_focus(ui);
        if state.return_to_play && ui.button("返回当前体验").reveal_focus(ui).clicked() {
            state.return_requested = true;
        }
        ui.label(crate::theme::muted("预览 → 应用到工程 → 保存"));
    });
    let advanced_mode = state.advanced;
    if advanced_mode && state.exchange_locales.is_none() {
        // Browsing may seed an exchange form, but only an author edit makes it a draft.
        state.exchange_locales = Some((state.source_locale.clone(), state.target_locale.clone()));
    }
    let (source_locale, target_locale) = if advanced_mode {
        let (source, target) = state.exchange_locales.as_mut().unwrap();
        (source, target)
    } else {
        (&mut state.source_locale, &mut state.target_locale)
    };
    let mut changed = false;
    let mut locale_row = |ui: &mut Ui| {
        if compact {
            ui.set_max_width(ui.available_width());
        }
        let mut locale_fields = |ui: &mut Ui| {
            ui.label("源语言");
            changed |= ui
                .add(
                    egui::TextEdit::singleline(&mut *source_locale)
                        .id(egui::Id::new((
                            "localization-source-locale",
                            &project.root,
                            advanced_mode,
                        )))
                        .desired_width(85.0)
                        .hint_text("en"),
                )
                .reveal_focus(ui)
                .changed();
            if compact {
                ui.end_row();
            }
            ui.label("目标语言");
            changed |= ui
                .add(
                    egui::TextEdit::singleline(&mut *target_locale)
                        .id(egui::Id::new((
                            "localization-target-locale",
                            &project.root,
                            advanced_mode,
                        )))
                        .desired_width(100.0)
                        .hint_text("zh-Hant"),
                )
                .reveal_focus(ui)
                .changed();
        };
        if compact {
            egui::Grid::new(("localization-locale-fields", advanced_mode))
                .num_columns(2)
                .show(ui, |ui| locale_fields(ui));
        } else {
            locale_fields(ui);
        }
        if let Some(page) = &state.workbench.page {
            if !page.available_locales.is_empty() {
                let mut combo = egui::ComboBox::from_id_salt("localization-known-locales")
                    .selected_text("已有语言");
                if compact {
                    combo = combo.width(120.0);
                }
                let response = combo
                    .show_ui(ui, |ui| {
                        for locale in &page.available_locales {
                            changed |= ui
                                .selectable_value(target_locale, locale.clone(), locale)
                                .reveal_focus(ui)
                                .changed();
                        }
                    })
                    .response
                    .reveal_focus(ui);
                if compact {
                    focus::reveal_opening_combo(ui, &response);
                }
            }
        }
    };
    if compact {
        ui.scope(&mut locale_row);
    } else {
        ui.horizontal_wrapped(&mut locale_row);
    }
    if changed {
        state.config_edited |= advanced_mode;
        invalidate_selection(state, advanced_mode);
    }
    ui.scope(|ui| jobs::status(ui, state));
    ui.scope(|ui| {
        if !project
            .required_features()
            .iter()
            .any(|f| f == "content.localization.v1")
        {
            ui.horizontal_wrapped(|ui| {
                ui.label("目录可查看；编辑前须显式启用本地化能力");
                if ui.button("查看语言与资料能力…").reveal_focus(ui).clicked() {
                    state.enable_requested = true;
                }
            });
        }
    });
    ui.separator();
    if state.advanced {
        if compact {
            advanced(ui, project, state);
        } else {
            egui::ScrollArea::vertical()
                .id_salt("localization-exchange")
                .show(ui, |ui| advanced(ui, project, state));
        }
    } else {
        catalog::show(ui, project, state, version, compact);
    }
}

fn invalidate_selection(state: &mut LocalizationUiState, authored: bool) {
    if authored {
        state.config_submitted = false;
    }
    state.export_plan = None;
    state.invalidate_import_preview();
    state.workbench.invalidate();
    state.cancel_preview();
}

fn advanced(ui: &mut Ui, project: &Project, state: &mut LocalizationUiState) {
    ui.label("只交换明确选择的 ID；不会自动扩展到引用或相邻台词。");
    ui.label("导出白名单（每行一个 ID）");
    if ui
        .add(
            egui::TextEdit::multiline(&mut state.string_ids)
                .id(egui::Id::new(("localization-whitelist", &project.root)))
                .desired_rows(3)
                .hint_text("welcome\nreply"),
        )
        .reveal_focus(ui)
        .changed()
    {
        invalidate_selection(state, true);
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button("预览导出").reveal_focus(ui).clicked() {
            preview_export(project, state);
        }
        let can_export = state.export_plan.as_ref().is_some_and(|p| p.can_export);
        if crate::theme::add_enabled(ui, can_export, egui::Button::new("导出 UTF-8 JSON…"))
            .reveal_focus(ui)
            .clicked()
        {
            export_exchange(ui, project, state);
        }
    });
    ui.scope(|ui| {
        if let Some(plan) = &state.export_plan {
            show_export_plan(ui, plan, &mut state.navigation, &mut state.export_view);
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    ui.scope(|ui| path_controls(ui, project, state));
    ui.separator();
    ui.strong("导入译文 · 先预览，后内存应用");
    ui.horizontal_wrapped(|ui| {
        if ui.button("选择 JSON 交换文件…").reveal_focus(ui).clicked() {
            choose_import_file(ui, state);
        }
        if ui.button("预览导入").reveal_focus(ui).clicked() {
            preview_import(project, state);
        }
    });
    match json_input::show(
        ui,
        &project.root,
        &mut state.exchange_json,
        &mut state.json_input,
    ) {
        json_input::Change::Edited => state.invalidate_import(),
        json_input::Change::Pending => state.invalidate_import_preview(),
        json_input::Change::None => {}
    }
    if let (Some(exchange), Some(plan)) = (&state.import_exchange, &state.import_plan) {
        show_exchange_entries(
            ui,
            exchange,
            &mut state.navigation,
            plan,
            &mut state.import_view,
        );
    }
    if let Some(plan) = &state.import_plan {
        show_import_plan(
            ui,
            plan,
            &mut state.navigation,
            navigation::Container::Import(plan),
        );
        if plan.can_apply && ui.button("复核通过 · 确认导入…").reveal_focus(ui).clicked()
        {
            state.confirm_apply = true;
        }
    }
    show_status(ui, state);
}

#[cfg(not(target_arch = "wasm32"))]
fn path_controls(ui: &mut Ui, project: &Project, state: &mut LocalizationUiState) {
    let paths = egui::CollapsingHeader::new("明确文件路径").show(ui, |ui| {
        ui.label("导出只接受工作区外的新 .json 文件；已有目标会被拒绝。");
        ui.add(
            egui::TextEdit::singleline(&mut state.export_path)
                .id(egui::Id::new(("localization-export-path", &project.root)))
                .hint_text("新导出文件完整路径"),
        )
        .reveal_focus(ui);
        if ui.button("导出到新路径").reveal_focus(ui).clicked() {
            let result = state
                .export_plan
                .as_ref()
                .ok_or_else(|| "请先预览导出".to_owned())
                .and_then(|plan| {
                    project.export_localization(
                        &state.selection(),
                        &plan.plan_digest,
                        Path::new(&state.export_path),
                    )
                })
                .map(|_| format!("已导出：{}", state.export_path));
            state.status = Some(result);
        }
        ui.add(
            egui::TextEdit::singleline(&mut state.import_path)
                .id(egui::Id::new(("localization-import-path", &project.root)))
                .hint_text("已有 UTF-8 JSON 完整路径"),
        )
        .reveal_focus(ui);
        if ui.button("从此路径载入").reveal_focus(ui).clicked() {
            let path = std::path::PathBuf::from(&state.import_path);
            match read_import_file(&path) {
                Ok(bytes) => state.load_import_file(&path, bytes),
                Err(error) => state.set_import_failure(error),
            }
        }
    });
    paths.header_response.reveal_focus(ui);
}

pub(super) fn status_text(
    status: worldline_core::localization::LocalizationStatus,
) -> &'static str {
    catalog::status_label(status)
}
