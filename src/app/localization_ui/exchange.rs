use super::*;

pub(super) fn preview_export(_project: &Project, state: &mut LocalizationUiState) {
    state.export_plan = None;
    state.status = None;
    state.jobs.submit(
        crate::localization_job::Task::ExportPreview {
            selection: state.selection(),
        },
        jobs::Intent::Export,
    );
}

pub(super) fn show_export_plan(
    ui: &mut Ui,
    plan: &LocalizationExportPlan,
    navigation: &mut Option<navigation::Request>,
    view: &mut exchange_view::View,
) {
    ui.group(|ui| {
        ui.label(if plan.can_export {
            "导出状态：可导出"
        } else {
            "导出状态：已阻止"
        });
        ui.label(format!("字符串协议版本：{}", plan.schema_version));
        ui.label(format!("源码基线：{}", plan.exchange.source_baseline));
        exchange_view::show(
            ui,
            &plan.exchange.entries,
            &plan.plan_digest,
            view,
            navigation,
            |entry| navigation::Request::export_entry(plan, entry),
        );
        show_diagnostics(
            ui,
            &plan.diagnostics,
            navigation,
            navigation::Container::Export(plan),
        );
    });
}

pub(super) fn show_exchange_entries(
    ui: &mut Ui,
    exchange: &LocalizationExchange,
    navigation: &mut Option<navigation::Request>,
    plan: &LocalizationImportPlan,
    view: &mut exchange_view::View,
) {
    ui.group(|ui| {
        ui.strong(format!(
            "交换包 · {} → {} · schema {}",
            exchange.source_locale, exchange.target_locale, exchange.schema_version
        ));
        exchange_view::show(
            ui,
            &exchange.entries,
            &plan.plan_digest,
            view,
            navigation,
            |entry| navigation::Request::import_entry(plan, entry),
        );
    });
}

pub(super) fn show_parts(ui: &mut Ui, parts: &[LocalizationPart]) {
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

pub(super) fn preview_import(_project: &Project, state: &mut LocalizationUiState) {
    if state.json_input.has_pending() {
        state.status = Some(Err("请先确认或取消大粘贴；当前完整 JSON 尚未改变".into()));
        return;
    }
    state.import_plan = None;
    state.import_exchange = None;
    state.confirm_apply = false;
    state.exchange_submitted = false;
    if state.exchange_json.len() > MAX_LOCALIZATION_JSON_BYTES {
        state.status = Some(Err("本地化交换文件超过 8 MiB 预算，未解析或修改工程".into()));
        return;
    }
    state.jobs.submit(
        crate::localization_job::Task::ImportPreview {
            selection: state.selection(),
            json: state.exchange_json.clone(),
        },
        jobs::Intent::Import,
    );
}

pub(super) fn show_import_plan(
    ui: &mut Ui,
    plan: &LocalizationImportPlan,
    navigation: &mut Option<navigation::Request>,
    container: navigation::Container<'_>,
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
        show_diagnostics(ui, &plan.diagnostics, navigation, container);
    });
}

pub(super) fn show_diagnostics(
    ui: &mut Ui,
    diagnostics: &[LocalizationDiagnostic],
    navigation: &mut Option<navigation::Request>,
    container: navigation::Container<'_>,
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
                    *navigation = navigation::Request::diagnostic(container, diagnostic);
                }
            }
        });
    }
}

pub(super) fn show_status(ui: &mut Ui, state: &LocalizationUiState) {
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
pub(super) fn choose_import_file(_ui: &mut Ui, state: &mut LocalizationUiState) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("本地化交换包（JSON）", &["json"])
        .pick_file()
    else {
        return;
    };
    match read_import_file(&path) {
        Ok(bytes) => state.load_import_file(&path, bytes),
        Err(error) => state.set_import_failure(error),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn read_import_file(path: &Path) -> Result<Vec<u8>, String> {
    let metadata = std::fs::metadata(path).map_err(|e| format!("无法读取本地化交换文件：{e}"))?;
    if metadata.len() > MAX_LOCALIZATION_JSON_BYTES as u64 {
        return Err("本地化交换文件超过 8 MiB 预算，未读取".into());
    }
    let file = std::fs::File::open(path).map_err(|e| format!("无法读取本地化交换文件：{e}"))?;
    read_import_bytes(file)
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn read_import_bytes(reader: impl std::io::Read) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    reader
        .take(MAX_LOCALIZATION_JSON_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("无法读取本地化交换文件：{e}"))?;
    if bytes.len() > MAX_LOCALIZATION_JSON_BYTES {
        return Err("本地化交换文件在读取时超过 8 MiB 预算；原输入保留".into());
    }
    Ok(bytes)
}

#[cfg(target_arch = "wasm32")]
pub(super) fn choose_import_file(ui: &mut Ui, _state: &mut LocalizationUiState) {
    crate::web::select_files(
        ui.ctx(),
        false,
        ".json,application/json",
        crate::web::FileAction::LocalizationImport,
    );
}

pub(super) fn export_exchange(ui: &mut Ui, project: &Project, state: &mut LocalizationUiState) {
    let selection = state.selection();
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("本地化交换包（JSON）", &["json"])
            .set_file_name(format!(
                "localization-{}-{}.json",
                selection.source_locale, selection.target_locale
            ))
            .save_file()
        else {
            return;
        };
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
        let current = project.preview_localization_export(&selection);
        let result = match current {
            Ok(current) if current.plan_digest == plan.plan_digest && current.can_export => {
                serde_json::to_vec(&current.exchange)
                    .map_err(|error| format!("无法序列化本地化交换包：{error}"))
                    .and_then(|bytes| {
                        crate::web::download(
                            &format!(
                                "localization-{}-{}.json",
                                selection.source_locale, selection.target_locale
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

pub(super) fn confirm_and_apply(
    ctx: &egui::Context,
    project: &mut Project,
    state: &mut LocalizationUiState,
) -> bool {
    let mut apply = false;
    egui::Window::new("确认本地化导入")
        .collapsible(false)
        .resizable(false)
        .max_width((ctx.screen_rect().width() - 64.0).max(120.0))
        .max_height((ctx.screen_rect().height() - 64.0).max(1.0))
        .show(ctx, |ui| {
            ui.set_width((ctx.screen_rect().width() - 64.0).clamp(120.0, 660.0));
            egui::ScrollArea::vertical()
                .id_salt("localization-import-confirm-scroll")
                .max_height((ctx.screen_rect().height() - 96.0).max(1.0))
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("取消导入").clicked() {
                            state.confirm_apply = false;
                        }
                        if ui.button("确认并原子导入").clicked() {
                            apply = true;
                        }
                    });
                    ui.label(
                        "核心预览已通过。确认后只更新内存工程，可一次撤销；保存工程后才写入磁盘。",
                    );
                    if let Some(plan) = &state.import_plan {
                        ui.label(format!(
                            "{} · {} 个字符串 · {}",
                            plan.target_locale,
                            plan.affected_ids.len(),
                            plan.sidecar_path
                        ));
                    }
                });
        });
    if !apply {
        return false;
    }
    let selection = state.selection();
    let before = project.clone();
    let result = match (&state.import_plan, &state.import_exchange) {
        (Some(plan), Some(exchange)) => project
            .apply_localization_import_candidate(&selection, exchange, &plan.plan_digest)
            .map_err(|e| e.to_string()),
        _ => Err("导入预览已失效，请重新预览".into()),
    };
    state.confirm_apply = false;
    match result {
        Ok(result) => {
            state.applied_before = Some(before);
            state.workbench.invalidate();
            state.exchange_submitted = true;
            state.config_submitted = true;
            state.import_plan = None;
            state.import_exchange = None;
            state.status = Some(Ok(format!(
                "已应用 {} 个字符串到内存；可撤销，保存工程后落盘。新基线 {}。",
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
