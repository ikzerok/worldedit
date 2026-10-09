use super::*;
use std::collections::BTreeMap;
use worldline_core::localization::{
    LocalizationCatalogEntry, LocalizationCatalogPage, LocalizationCatalogQuery, LocalizationStatus,
};

pub(super) const PAGE_SIZE: usize = 40;
#[derive(Default)]
pub(super) struct Workbench {
    pub page: Option<LocalizationCatalogPage>,
    pub search: String,
    pub source_prefix: String,
    pub kind: Option<String>,
    pub status: Option<LocalizationStatus>,
    pub offset: usize,
    pub focus_id: Option<String>,
    pub runtime_revision: Option<String>,
    pub runtime_revision_mismatch: bool,
    pub selected: Option<String>,
    pub detail: bool,
    pub drafts: BTreeMap<String, editing::DraftBuffer>,
    pub id_inputs: BTreeMap<String, String>,
    pub preview: Option<plans::Preview>,
    cached: Option<(u64, LocalizationCatalogQuery)>,
    page_version: Option<u64>,
    page_offset: usize,
    pub query_error: Option<String>,
    pub query_count: u64,
}
impl Workbench {
    pub fn invalidate(&mut self) {
        self.cached = None;
    }
    pub fn has_input(&self) -> bool {
        !self.drafts.is_empty()
            || self.id_inputs.values().any(|s| !s.is_empty())
            || self.preview.is_some()
    }
    pub fn selected_entry(&self) -> Option<&LocalizationCatalogEntry> {
        self.page
            .as_ref()?
            .entries
            .iter()
            .find(|e| Some(&e.unit_key) == self.selected.as_ref())
    }
}

pub(super) fn status_label(status: LocalizationStatus) -> &'static str {
    match status {
        LocalizationStatus::MissingId => "缺 ID",
        LocalizationStatus::DuplicateId => "重复 ID",
        LocalizationStatus::InvalidTranslation => "无效译文",
        LocalizationStatus::StaleSource => "源文已变",
        LocalizationStatus::MissingTranslation => "缺译",
        LocalizationStatus::Translated => "已译",
        LocalizationStatus::OrphanTranslation => "孤立译文",
    }
}
const STATUSES: [LocalizationStatus; 7] = [
    LocalizationStatus::MissingId,
    LocalizationStatus::DuplicateId,
    LocalizationStatus::MissingTranslation,
    LocalizationStatus::Translated,
    LocalizationStatus::StaleSource,
    LocalizationStatus::InvalidTranslation,
    LocalizationStatus::OrphanTranslation,
];

fn query(state: &LocalizationUiState) -> LocalizationCatalogQuery {
    let w = &state.workbench;
    LocalizationCatalogQuery {
        target_locale: (!state.target_locale.trim().is_empty())
            .then(|| state.target_locale.trim().into()),
        source_prefix: (!w.source_prefix.trim().is_empty()).then(|| w.source_prefix.trim().into()),
        search: w.search.clone(),
        kind: w.kind.clone(),
        statuses: w.status.into_iter().collect(),
        string_ids: w.focus_id.iter().cloned().collect(),
        offset: w.offset,
        limit: PAGE_SIZE,
        ..Default::default()
    }
}

pub(super) fn refresh(_project: &Project, state: &mut LocalizationUiState, version: u64) {
    if state.workbench.page_version.is_some_and(|v| v != version) {
        state.workbench.offset = 0;
    }
    let query = query(state);
    if state.workbench.cached.as_ref() == Some(&(version, query.clone())) {
        return;
    }
    let w = &mut state.workbench;
    w.query_count += 1;
    w.preview = None;
    let mut request = query.clone();
    if w.page_version == Some(version) && w.offset > 0 {
        if let Some(page) = &w.page {
            request.expected_content_baseline = Some(page.content_baseline.clone());
            request.expected_source_baseline = Some(page.source_baseline.clone());
        }
    }
    w.cached = Some((version, query.clone()));
    state.jobs.submit(
        crate::localization_job::Task::Catalog { query: request },
        jobs::Intent::Catalog { query },
    );
}

pub(super) fn install(
    state: &mut LocalizationUiState,
    page: LocalizationCatalogPage,
    version: u64,
    query: LocalizationCatalogQuery,
) {
    if state.source_locale.is_empty() {
        if let Some(locale) = &page.source_locale {
            state.source_locale = locale.clone();
        }
    }
    let w = &mut state.workbench;
    if !page
        .entries
        .iter()
        .any(|e| Some(&e.unit_key) == w.selected.as_ref())
    {
        w.selected = page.entries.first().map(|e| e.unit_key.clone());
    }
    if let Some(revision) = &w.runtime_revision {
        if let Some(entry) = page
            .entries
            .iter()
            .find(|entry| entry.source_revision.as_ref() == Some(revision))
        {
            w.selected = Some(entry.unit_key.clone());
        }
    }
    w.runtime_revision_mismatch = w.runtime_revision.as_ref().is_some_and(|revision| {
        !page.entries.iter().any(|entry| {
            Some(&entry.unit_key) == w.selected.as_ref()
                && entry.source_revision.as_ref() == Some(revision)
        })
    });
    w.page_offset = query.offset;
    w.cached = Some((version, query));
    w.query_error = None;
    w.page_version = Some(version);
    w.page = Some(page);
}

pub(super) fn show(
    ui: &mut Ui,
    project: &Project,
    state: &mut LocalizationUiState,
    version: u64,
    compact: bool,
) {
    refresh(project, state, version);
    ui.horizontal_wrapped(|ui| {
        let pending = state
            .workbench
            .drafts
            .values()
            .filter(|d| d.target_locale == state.target_locale.trim())
            .count();
        if crate::theme::add_enabled(
            ui,
            pending > 0 && !state.jobs.pending(),
            crate::theme::primary(&format!("预览 {pending} 项译文")),
        )
        .reveal_focus(ui)
        .clicked()
        {
            plans::preview_edits(project, state);
        }
        if ui.button("刷新目录").reveal_focus(ui).clicked() {
            state.workbench.invalidate();
        }
        ui.label(crate::theme::muted("当前已应用稿 · 正文草稿不会自动应用"));
    });
    let waiting = state.jobs.catalog_pending();
    let editing_enabled = can_edit_retained_page(project, state, version);
    // Dynamic notices and retained inputs cannot renumber the following navigation/actions.
    ui.scope(|ui| {
    show_status(ui, state);
    if let Some(error) = &state.workbench.query_error {
        ui.colored_label(crate::theme::ERROR(), format!("上次目录核对失败：{error}"));
    }
    super::retained::show(ui, project, state);
    if !editing_enabled {
        if let Some(page) = &state.workbench.page {
            ui.colored_label(crate::theme::WARNING(), format!(
                "保留的 {} 目录只读；当前目标语言 {}、工作区或工程版本尚未核对。请刷新目录；未提交输入仍保留。",
                page.target_locale.as_deref().unwrap_or("源文"), state.target_locale.trim()
            ));
        }
    }
    if waiting {
        if let Some(page) = &state.workbench.page {
            ui.small(format!(
                "正在更新；仍显示 {} 的先前稳定页",
                page.target_locale.as_deref().unwrap_or("源文目录")
            ));
        }
    }
    });
    if compact {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut state.workbench.detail, false, "字符串目录")
                .reveal_focus(ui);
            ui.selectable_value(&mut state.workbench.detail, true, "当前源文与译文")
                .reveal_focus(ui);
        });
        if state.workbench.detail {
            ui.add_enabled_ui(editing_enabled, |ui| editing::show(ui, project, state));
        } else {
            filters(ui, project, state);
            list(ui, state);
        }
    } else {
        let size = ui.available_size();
        let directory_width = (size.x * 0.28).clamp(250.0, 350.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(directory_width, size.y),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_max_width(directory_width);
                    egui::ScrollArea::vertical()
                        .id_salt("localization-directory")
                        .show(ui, |ui| {
                            filters(ui, project, state);
                            list(ui, state);
                        });
                },
            );
            ui.separator();
            ui.allocate_ui_with_layout(
                ui.available_size(),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("localization-editor")
                        .show(ui, |ui| {
                            ui.add_enabled_ui(editing_enabled, |ui| {
                                editing::show(ui, project, state)
                            });
                        });
                },
            );
        });
    }
}

pub(super) fn can_edit_retained_page(
    project: &Project,
    state: &LocalizationUiState,
    version: u64,
) -> bool {
    state
        .jobs
        .accepted_matches(jobs::AcceptedKind::Catalog, &project.root, version)
        && state.workbench.page.as_ref().is_some_and(|page| {
            page.target_locale.as_deref().unwrap_or_default() == state.target_locale.trim()
        })
}

fn filters(ui: &mut Ui, project: &Project, state: &mut LocalizationUiState) {
    let w = &mut state.workbench;
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        changed |= ui
            .add(
                egui::TextEdit::singleline(&mut w.search)
                    .desired_width(180.0)
                    .id(egui::Id::new(("localization-search", &project.root)))
                    .hint_text("搜索源文、译文或 ID"),
            )
            .reveal_focus(ui)
            .changed();
        if w.focus_id.is_some() && ui.button("显示全部条目").reveal_focus(ui).clicked() {
            w.focus_id = None;
            w.runtime_revision = None;
            w.runtime_revision_mismatch = false;
            changed = true;
        }
    });
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("localization-status")
            .selected_text(w.status.map(status_label).unwrap_or("全部状态"))
            .show_ui(ui, |ui| {
                changed |= ui
                    .selectable_value(&mut w.status, None, "全部状态")
                    .reveal_focus(ui)
                    .changed();
                for status in STATUSES {
                    let count = w
                        .page
                        .as_ref()
                        .and_then(|p| p.status_counts.get(&status))
                        .copied()
                        .unwrap_or(0);
                    changed |= ui
                        .selectable_value(
                            &mut w.status,
                            Some(status),
                            format!("{} · {count}", status_label(status)),
                        )
                        .reveal_focus(ui)
                        .changed();
                }
            })
            .response
            .reveal_focus(ui);
        egui::ComboBox::from_id_salt("localization-kind")
            .selected_text(w.kind.as_deref().unwrap_or("全部类型"))
            .show_ui(ui, |ui| {
                changed |= ui
                    .selectable_value(&mut w.kind, None, "全部类型")
                    .reveal_focus(ui)
                    .changed();
                for (kind, label) in [("text", "正文"), ("say", "台词"), ("choice", "选项")] {
                    changed |= ui
                        .selectable_value(&mut w.kind, Some(kind.into()), label)
                        .reveal_focus(ui)
                        .changed();
                }
            })
            .response
            .reveal_focus(ui);
    });
    changed |= ui
        .add(
            egui::TextEdit::singleline(&mut w.source_prefix)
                .desired_width(f32::INFINITY)
                .id(egui::Id::new(("localization-source-filter", &project.root)))
                .hint_text("来源文件前缀（工作区相对路径）"),
        )
        .reveal_focus(ui)
        .changed();
    if changed {
        w.offset = 0;
        w.invalidate();
    }
}

fn list(ui: &mut Ui, state: &mut LocalizationUiState) {
    let waiting = state.jobs.catalog_pending();
    let w = &mut state.workbench;
    let Some(page) = &w.page else { return };
    ui.label(format!(
        "匹配 {} 项 / 工程 {} 项",
        page.total, page.all_total
    ));
    if page.read_only {
        ui.colored_label(
            crate::theme::WARNING(),
            "当前工程只读；输入可保留，应用受阻",
        );
    }
    let end = (w.page_offset + page.entries.len()).min(page.total);
    ui.horizontal_wrapped(|ui| {
        if crate::theme::add_enabled(
            ui,
            w.page_offset > 0 && !waiting,
            egui::Button::new("上一页"),
        )
        .reveal_focus(ui)
        .clicked()
        {
            w.offset = w.page_offset.saturating_sub(PAGE_SIZE);
            w.cached = None;
        }
        ui.label(format!(
            "{}–{} / {}",
            if end == 0 { 0 } else { w.page_offset + 1 },
            end,
            page.total
        ));
        if crate::theme::add_enabled(
            ui,
            page.next_offset.is_some() && !waiting,
            egui::Button::new("下一页"),
        )
        .reveal_focus(ui)
        .clicked()
        {
            w.offset = page.next_offset.unwrap_or(w.offset);
            w.cached = None;
        }
    });
    if page.entries.is_empty() {
        ui.label("这个范围没有字符串；可调整筛选条件");
    }
    for entry in &page.entries {
        ui.push_id(&entry.unit_key, |ui| {
            let title = entry.id.as_deref().unwrap_or("待分配稳定 ID");
            let summary: String = entry
                .source_parts
                .iter()
                .filter_map(|part| match part {
                    LocalizationPart::Text { text } => Some(text.as_str()),
                    LocalizationPart::Link { label, .. } => Some(label.as_str()),
                    _ => None,
                })
                .flat_map(str::chars)
                .take(88)
                .collect();
            let label = format!("{} · {}\n{}", status_label(entry.status), title, summary);
            if crate::theme::add_enabled(
                ui,
                !waiting,
                egui::Button::new(label)
                    .wrap()
                    .selected(w.selected.as_ref() == Some(&entry.unit_key))
                    .min_size(egui::vec2(ui.available_width(), 56.0)),
            )
            .reveal_focus(ui)
            .clicked()
            {
                w.selected = Some(entry.unit_key.clone());
                w.detail = true;
            }
            if let Some(source) = &entry.source {
                ui.small(format!("{}:{} · {}", source.file, source.line, source.kind));
            }
        });
    }
    show_diagnostics(
        ui,
        &page.diagnostics,
        &mut state.navigation,
        navigation::Container::Catalog(page),
    );
}
