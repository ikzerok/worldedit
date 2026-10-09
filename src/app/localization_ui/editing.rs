use super::*;
use worldline_core::localization::{
    LocalizationCatalogEntry, LocalizationEdit, LocalizationStatus,
};

#[derive(Clone)]
pub(super) struct DraftBuffer {
    pub source_locale: String,
    pub target_locale: String,
    pub source_baseline: String,
    pub edit: LocalizationEdit,
}

pub(super) fn draft_key(locale: &str, id: &str) -> String {
    format!("{locale}\u{1f}{id}")
}

pub(super) fn show(ui: &mut Ui, project: &Project, state: &mut LocalizationUiState) {
    let Some(entry) = state.workbench.selected_entry().cloned() else {
        ui.label("从目录选择一个字符串，开始制作译文");
        return;
    };
    if state.workbench.runtime_revision_mismatch {
        ui.colored_label(crate::theme::WARNING(), "这条试演输出的源修订与已应用目录不同。原正文草稿保留；请先返回正文，明确应用后重新体验，再修正此译文。");
        ui.strong("当前已应用源文 · 只读核对");
        show_parts(ui, &entry.source_parts);
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.strong(entry.id.as_deref().unwrap_or("尚无稳定 ID"));
        ui.label(catalog::status_label(entry.status));
        if let Some(source) = &entry.source {
            if ui
                .link(format!("源文 {}:{}", source.file, source.line))
                .reveal_focus(ui)
                .clicked()
            {
                state.navigation = state
                    .workbench
                    .page
                    .as_ref()
                    .and_then(|page| navigation::Request::catalog_entry(page, &entry));
            }
        }
    });
    if let Some(source) = &entry.source {
        ui.small(format!("{} · {}", source.kind, source.file));
    }
    if entry.status == LocalizationStatus::OrphanTranslation {
        ui.colored_label(
            crate::theme::WARNING(),
            "当前源码没有这个 ID；保留旧译文，不猜测关联对象",
        );
    }
    if matches!(
        entry.status,
        LocalizationStatus::MissingId | LocalizationStatus::DuplicateId
    ) {
        id_editor(ui, project, state, &entry);
    } else {
        let identity = egui::CollapsingHeader::new("稳定身份与来源").show(ui, |ui| {
            if let Some(revision) = &entry.source_revision {
                ui.label(format!("源修订 {revision}"));
            }
            if let Some(path) = &entry.sidecar_path {
                ui.label(format!("译文文件 {path}"));
            }
            if let Some(pointer) = &entry.translation_pointer {
                ui.label(format!("条目 {pointer}"));
            }
            id_editor(ui, project, state, &entry);
        });
        identity.header_response.reveal_focus(ui);
    }
    ui.separator();
    let editable = entry.id.is_some()
        && entry.source_revision.is_some()
        && entry.status != LocalizationStatus::DuplicateId;
    if editable && ui.available_width() >= 780.0 {
        ui.columns(2, |columns| {
            columns[0].strong("源文 · 只读对照");
            show_parts(&mut columns[0], &entry.source_parts);
            translation_editor(&mut columns[1], project, state, &entry);
        });
        return;
    }
    ui.strong("源文 · 只读对照");
    ui.group(|ui| {
        ui.set_min_width(ui.available_width().max(0.0));
        show_parts(ui, &entry.source_parts);
    });
    if entry.id.is_none()
        || entry.source_revision.is_none()
        || entry.status == LocalizationStatus::DuplicateId
    {
        if let Some(parts) = &entry.translation_parts {
            ui.strong("现有译文");
            show_parts(ui, parts);
        }
        return;
    }
    translation_editor(ui, project, state, &entry);
}

fn id_editor(
    ui: &mut Ui,
    project: &Project,
    state: &mut LocalizationUiState,
    entry: &LocalizationCatalogEntry,
) {
    if entry.source.is_none() {
        return;
    }
    let input = state
        .workbench
        .id_inputs
        .entry(entry.unit_key.clone())
        .or_default();
    ui.label("显式分配或更换稳定 ID");
    let changed = ui
        .add(
            egui::TextEdit::singleline(input)
                .id(egui::Id::new((
                    "localization-stable-id",
                    &project.root,
                    &entry.unit_key,
                )))
                .desired_width(ui.available_width())
                .hint_text("例如 chapter01_welcome"),
        )
        .reveal_focus(ui)
        .changed();
    if changed {
        state.cancel_preview();
    }
    if ui.button("预览此 ID 修改").reveal_focus(ui).clicked() {
        plans::preview_id(project, state, entry);
    }
    ui.small("ID 随源文保存；更换后旧交换包需要重新导出，旧 ID 的译文不会自动迁移");
}

fn translation_editor(
    ui: &mut Ui,
    project: &Project,
    state: &mut LocalizationUiState,
    entry: &LocalizationCatalogEntry,
) {
    let id = entry.id.as_deref().unwrap_or_default();
    let locale = state
        .workbench
        .page
        .as_ref()
        .and_then(|page| page.target_locale.as_deref())
        .unwrap_or_default()
        .to_owned();
    let key = draft_key(&locale, id);
    let existing = state.workbench.drafts.get(&key).cloned();
    let mut parts = existing
        .as_ref()
        .map(|d| d.edit.translation_parts.clone())
        .or_else(|| entry.translation_parts.clone())
        .unwrap_or_else(|| entry.source_parts.clone());
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!(
            "译文 · {}",
            if locale.is_empty() {
                "先填写目标语言"
            } else {
                &locale
            }
        ));
        if existing.is_some() {
            ui.label("未提交输入");
        }
    });
    let old = existing.as_ref().is_some_and(|draft| {
        Some(&draft.edit.source_revision) != entry.source_revision.as_ref()
            || state
                .workbench
                .page
                .as_ref()
                .is_some_and(|p| p.source_baseline != draft.source_baseline)
            || draft.source_locale != state.source_locale.trim()
    });
    ui.scope(|ui| {
        if old {
            ui.colored_label(
                crate::theme::WARNING(),
                "源文已变化；保留你的译文输入。复核后才能绑定当前修订",
            );
            if ui
                .button("我已对照当前源文，重新绑定此稿")
                .reveal_focus(ui)
                .clicked()
            {
                if let Some(draft) = state.workbench.drafts.get_mut(&key) {
                    draft.source_locale = state.source_locale.trim().into();
                    draft.edit.source_revision = entry.source_revision.clone().unwrap_or_default();
                    draft.source_baseline = state
                        .workbench
                        .page
                        .as_ref()
                        .map(|p| p.source_baseline.clone())
                        .unwrap_or_default();
                    state.cancel_preview();
                }
            }
        }
        if entry.status == LocalizationStatus::StaleSource {
            ui.colored_label(
                crate::theme::WARNING(),
                "现有译文的源修订已过期；修改或明确复核后重新预览应用",
            );
        }
    });
    ui.small("文字和链接标签可编辑；token 身份不可改。上下移动整段调整译文顺序");
    let mut changed = ui
        .scope(|ui| parts_editor(ui, project, &mut parts, &key))
        .inner;
    ui.horizontal_wrapped(|ui| {
        if ui.button("添加文字段").reveal_focus(ui).clicked() {
            parts.push(LocalizationPart::Text {
                text: String::new(),
            });
            changed = true;
        }
        if ui.button("保留此译文并待复核").reveal_focus(ui).clicked() {
            changed = true;
        }
        if ui.button("从当前源文重新起稿").reveal_focus(ui).clicked() {
            parts = entry.source_parts.clone();
            changed = true;
        }
    });
    if changed {
        let buffer = existing.unwrap_or_else(|| DraftBuffer {
            source_locale: state.source_locale.trim().into(),
            target_locale: locale,
            source_baseline: state
                .workbench
                .page
                .as_ref()
                .map(|p| p.source_baseline.clone())
                .unwrap_or_default(),
            edit: LocalizationEdit {
                id: id.into(),
                source_revision: entry.source_revision.clone().unwrap_or_default(),
                translation_parts: Vec::new(),
            },
        });
        state.workbench.drafts.insert(
            key.clone(),
            DraftBuffer {
                edit: LocalizationEdit {
                    translation_parts: parts,
                    ..buffer.edit
                },
                ..buffer
            },
        );
        state.cancel_preview();
        state.status = None;
    }
    if state.workbench.drafts.contains_key(&key)
        && ui.button("放弃此条未提交输入").reveal_focus(ui).clicked()
    {
        state.workbench.drafts.remove(&key);
        state.cancel_preview();
    }
}

fn parts_editor(
    ui: &mut Ui,
    project: &Project,
    parts: &mut Vec<LocalizationPart>,
    key: &str,
) -> bool {
    let mut changed = false;
    let mut reorder = None;
    let mut remove = None;
    let len = parts.len();
    for (index, part) in parts.iter_mut().enumerate() {
        ui.push_id((key, index), |ui| {
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.small(format!("段 {}", index + 1));
                    if crate::theme::add_enabled(ui, index > 0, egui::Button::new("↑").small())
                        .reveal_focus(ui)
                        .clicked()
                    {
                        reorder = Some((index, index - 1));
                    }
                    if crate::theme::add_enabled(
                        ui,
                        index + 1 < len,
                        egui::Button::new("↓").small(),
                    )
                    .reveal_focus(ui)
                    .clicked()
                    {
                        reorder = Some((index, index + 1));
                    }
                    if matches!(part, LocalizationPart::Text { .. })
                        && ui.small_button("删除文字段").reveal_focus(ui).clicked()
                    {
                        remove = Some(index);
                    }
                });
                match part {
                    LocalizationPart::Text { text } => {
                        changed |= ui
                            .add(
                                egui::TextEdit::multiline(text)
                                    .id(egui::Id::new((
                                        "localization-part-text",
                                        &project.root,
                                        key,
                                        index,
                                    )))
                                    .desired_rows(3)
                                    .desired_width(f32::INFINITY)
                                    .hint_text("输入译文，支持多行和 emoji"),
                            )
                            .reveal_focus(ui)
                            .changed();
                    }
                    LocalizationPart::Placeholder { token } => {
                        ui.colored_label(
                            crate::theme::resolved(ui.ctx()).colors.info,
                            format!("受保护占位符 · {token}"),
                        );
                    }
                    LocalizationPart::Link { token, label } => {
                        ui.colored_label(
                            crate::theme::resolved(ui.ctx()).colors.info,
                            format!("受保护链接 · {token}"),
                        );
                        changed |= ui
                            .add(
                                egui::TextEdit::singleline(label)
                                    .id(egui::Id::new((
                                        "localization-part-link",
                                        &project.root,
                                        key,
                                        index,
                                    )))
                                    .desired_width(f32::INFINITY)
                                    .hint_text("译文链接标签"),
                            )
                            .reveal_focus(ui)
                            .changed();
                    }
                }
            });
        });
    }
    if reorder.is_some() || remove.is_some() {
        // Index-based text fields must not transfer a live cursor to another typed part.
        if let Some(focus) = ui.memory(|m| m.focused()) {
            ui.memory_mut(|m| m.surrender_focus(focus));
        }
    }
    if let Some((a, b)) = reorder {
        parts.swap(a, b);
        changed = true;
    }
    if let Some(index) = remove {
        parts.remove(index);
        changed = true;
    }
    if parts.is_empty() {
        ui.label("明确的空译文；是否有效由 core 预览校验");
    }
    changed
}
