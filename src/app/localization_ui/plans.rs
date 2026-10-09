use super::*;
use worldline_core::localization::{
    LocalizationCatalogEntry, LocalizationEditDraft, LocalizationIdAssignment, LocalizationIdDraft,
    LocalizationIdPlan,
};

pub(super) enum Preview {
    Edits {
        draft: LocalizationEditDraft,
        plan: LocalizationImportPlan,
        keys: Vec<String>,
    },
    Id {
        draft: LocalizationIdDraft,
        plan: LocalizationIdPlan,
        key: String,
    },
}

pub(super) fn preview_edits(_project: &Project, state: &mut LocalizationUiState) {
    let locale = state.target_locale.trim();
    let drafts: Vec<_> = state
        .workbench
        .drafts
        .iter()
        .filter(|(_, d)| d.target_locale == locale)
        .collect();
    let Some((_, first)) = drafts.first() else {
        return;
    };
    let draft = LocalizationEditDraft {
        schema_version: 1,
        source_locale: state.source_locale.trim().into(),
        target_locale: locale.into(),
        source_baseline: first.source_baseline.clone(),
        edits: drafts.iter().map(|(_, d)| d.edit.clone()).collect(),
    };
    // Mixed source snapshots remain visible instead of silently rebasing older inputs.
    if drafts.iter().any(|(_, d)| {
        d.source_baseline != draft.source_baseline || d.source_locale != draft.source_locale
    }) {
        state.status = Some(Err(
            "这些译文输入来自不同源文基线或源语言，请逐项复核当前源文再预览".into(),
        ));
        return;
    }
    let keys = drafts.iter().map(|(key, _)| (*key).clone()).collect();
    state.status = None;
    state.jobs.submit(
        crate::localization_job::Task::EditPreview { draft },
        jobs::Intent::Edits { keys },
    );
}

pub(super) fn preview_id(
    _project: &Project,
    state: &mut LocalizationUiState,
    entry: &LocalizationCatalogEntry,
) {
    let Some(source) = &entry.source else { return };
    let Some(revision) = &entry.source_revision else {
        return;
    };
    let Some(page) = &state.workbench.page else {
        return;
    };
    let draft = LocalizationIdDraft {
        schema_version: 1,
        source_baseline: page.source_baseline.clone(),
        assignments: vec![LocalizationIdAssignment {
            source: source.clone(),
            source_revision: revision.clone(),
            expected_id: entry.id.clone(),
            id: state
                .workbench
                .id_inputs
                .get(&entry.unit_key)
                .cloned()
                .unwrap_or_default(),
        }],
    };
    state.status = None;
    state.jobs.submit(
        crate::localization_job::Task::IdPreview { draft },
        jobs::Intent::Id {
            key: entry.unit_key.clone(),
        },
    );
}

pub(super) fn confirm(
    ctx: &egui::Context,
    project: &mut Project,
    state: &mut LocalizationUiState,
) -> bool {
    let Some(preview) = &state.workbench.preview else {
        return false;
    };
    let mut cancel = false;
    let mut apply = false;
    let (valid, title) = match preview {
        Preview::Edits { plan, .. } => (plan.can_apply, "译文应用预览"),
        Preview::Id { plan, .. } => (plan.can_apply, "稳定 ID 修改预览"),
    };
    egui::Modal::new(egui::Id::new("localization-workbench-preview")).show(ctx, |ui| {
        ui.set_width((ctx.screen_rect().width() - 64.0).clamp(120.0, 660.0));
        egui::ScrollArea::vertical()
            .id_salt("localization-plan-scroll")
            .max_height((ctx.screen_rect().height() - 64.0).max(1.0))
            .show(ui, |ui| {
                ui.heading(title);
                ui.horizontal_wrapped(|ui| {
                    apply |= crate::theme::add_enabled(
                        ui,
                        valid,
                        crate::theme::primary("应用到工程（可撤销）"),
                    )
                    .clicked();
                    cancel |= ui.button("取消预览，保留输入").clicked();
                });
                ui.label("只修改内存；保存工程后才落盘。取消不会改变源码或译文。");
                match preview {
                    Preview::Edits { plan, .. } => show_import_plan(
                        ui,
                        plan,
                        &mut state.navigation,
                        navigation::Container::Edits(plan),
                    ),
                    Preview::Id { plan, .. } => {
                        ui.label(format!("{} 个源码文件", plan.changes.len()));
                        for change in &plan.changes {
                            ui.strong(&change.file);
                            egui::CollapsingHeader::new("查看修改前后源码")
                                .id_salt(&change.file)
                                .show(ui, |ui| {
                                    ui.label("修改前");
                                    ui.add(egui::Label::new(&change.before).wrap());
                                    ui.label("修改后");
                                    ui.add(egui::Label::new(&change.after).wrap());
                                });
                        }
                        show_diagnostics(
                            ui,
                            &plan.diagnostics,
                            &mut state.navigation,
                            navigation::Container::Id(plan),
                        );
                        ui.label("旧交换包在 ID 修改后必须重新导出；旧 ID 译文保留");
                    }
                }
            });
    });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel = true;
    }
    if cancel {
        state.workbench.preview = None;
        return false;
    }
    if !apply {
        return false;
    }
    let before = project.clone();
    let preview = state.workbench.preview.take().unwrap();
    let result = match preview {
        Preview::Edits { draft, plan, keys } => project
            .apply_localization_edit(&draft, &plan.plan_digest)
            .map(|_| {
                for key in keys {
                    state.workbench.drafts.remove(&key);
                }
            }),
        Preview::Id { draft, plan, key } => project
            .apply_localization_ids(&draft, &plan.plan_digest)
            .map(|_| {
                state.workbench.id_inputs.remove(&key);
            }),
    };
    match result {
        Ok(()) => {
            state.applied_before = Some(before);
            state.workbench.invalidate();
            state.status = Some(Ok("已一次应用到工程；可撤销，保存后落盘".into()));
            true
        }
        Err(e) => {
            state.status = Some(Err(e.to_string()));
            false
        }
    }
}
