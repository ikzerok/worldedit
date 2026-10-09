//! Exercise the renderer gate, not just the identity predicate: stopped work is not accepted work.
use super::focus_tests::{click_label, ctrl_a, draw};
use super::workbench_tests::fixture;
use super::*;

const OLD: &str = "原繁體譯文";

fn apply_translation(project: &mut Project, text: &str) {
    let selection = LocalizationSelection {
        schema_version: 1,
        source_locale: "en".into(),
        target_locale: "zh-Hant".into(),
        string_ids: vec!["line0".into()],
    };
    let mut exchange = project
        .preview_localization_export(&selection)
        .unwrap()
        .exchange;
    exchange.entries[0].translation_parts =
        Some(vec![LocalizationPart::Text { text: text.into() }]);
    let plan = project
        .preview_localization_import_candidate(&selection, &exchange)
        .unwrap();
    assert!(plan.can_apply);
    project
        .apply_localization_import_candidate(&selection, &exchange, &plan.plan_digest)
        .unwrap();
}

fn translated() -> (Project, LocalizationUiState) {
    let (mut project, mut state) = fixture(1);
    apply_translation(&mut project, OLD);
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    assert_eq!(
        state
            .workbench
            .page
            .as_ref()
            .unwrap()
            .target_locale
            .as_deref(),
        Some("zh-Hant")
    );
    (project, state)
}

fn inputs(state: &LocalizationUiState) -> serde_json::Value {
    let drafts: Vec<_> = state
        .workbench
        .drafts
        .iter()
        .map(|(key, value)| {
            (
                key,
                &value.source_locale,
                &value.target_locale,
                &value.source_baseline,
                &value.edit,
            )
        })
        .collect();
    serde_json::json!({ "drafts": drafts, "ids": state.workbench.id_inputs, "json": state.exchange_json })
}

fn failed_or_cancelled(
    project: &Project,
    state: &mut LocalizationUiState,
    failed: bool,
    version: u64,
) {
    if failed {
        state.workbench.source_prefix = "../outside".into();
    }
    state.workbench.invalidate();
    catalog::refresh(project, state, version);
    assert!(state.jobs.catalog_pending());
    if failed {
        jobs::settle(project, state, version);
        assert_eq!(
            state.workbench.query_error.as_deref(),
            Some("来源前缀必须是工作区内相对路径")
        );
    } else {
        // Exactly the action used by the Cancel button, before a fast fixture worker can complete.
        state.jobs.cancel();
    }
    assert!(!state.jobs.catalog_pending());
}

fn stopped_new_locale(failed: bool) {
    for existing_fr in [false, true] {
        let (mut project, mut state) = translated();
        let ctx = egui::Context::default();
        if existing_fr {
            let entry = &state.workbench.page.as_ref().unwrap().entries[0];
            state.workbench.drafts.insert(
                editing::draft_key("fr", "line0"),
                editing::DraftBuffer {
                    source_locale: "en".into(),
                    target_locale: "fr".into(),
                    source_baseline: state
                        .workbench
                        .page
                        .as_ref()
                        .unwrap()
                        .source_baseline
                        .clone(),
                    edit: worldline_core::localization::LocalizationEdit {
                        id: "line0".into(),
                        source_revision: entry.source_revision.clone().unwrap(),
                        translation_parts: vec![LocalizationPart::Text {
                            text: "Existing French draft".into(),
                        }],
                    },
                },
            );
        }
        state.target_locale = "fr".into();
        failed_or_cancelled(&project, &mut state, failed, 1);
        draw(&ctx, &mut project, &mut state, vec![]);
        let before = inputs(&state);
        let baseline = project.content_baseline();
        let key = editing::draft_key("zh-Hant", "line0");
        let id = egui::Id::new(("localization-part-text", &project.root, &key, 0usize));
        let requested_key = editing::draft_key("fr", "line0");
        let requested_id = egui::Id::new((
            "localization-part-text",
            &project.root,
            &requested_key,
            0usize,
        ));
        assert!(
            ctx.read_response(requested_id)
                .is_none_or(|response| !response.enabled()),
            "an unaccepted target locale must not have an enabled editor"
        );
        let enabled = ctx
            .read_response(id)
            .expect("retained field stays visible")
            .enabled();
        click_label(&ctx, &mut project, &mut state, OLD);
        draw(
            &ctx,
            &mut project,
            &mut state,
            vec![egui::Event::Text("错误串入".into())],
        );
        click_label(&ctx, &mut project, &mut state, "保留此译文并待复核");
        let unsafe_plan_accepted = if enabled {
            plans::preview_edits(&project, &mut state);
            jobs::settle(&project, &mut state, 1);
            matches!(&state.workbench.preview, Some(plans::Preview::Edits { plan, .. }) if plan.can_apply)
        } else {
            false
        };
        assert!(!enabled, "stopped new locale exposed old zh editor; failed={failed}, existing_fr={existing_fr}, core accepts contaminated draft={unsafe_plan_accepted}, drafts={}", inputs(&state));
        assert_eq!(inputs(&state), before);
        assert_eq!(project.content_baseline(), baseline);
        let output = draw(&ctx, &mut project, &mut state, vec![]);
        let rendered = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(rendered.contains("保留的 zh-Hant 目录只读"));
        assert!(
            rendered.contains("译文 · zh-Hant") && !rendered.contains("译文 · fr"),
            "a retained zh translation must not be labelled as French: {rendered}"
        );
        assert_eq!(
            state
                .workbench
                .page
                .as_ref()
                .unwrap()
                .target_locale
                .as_deref(),
            Some("zh-Hant")
        );
    }
}

#[test]
fn localization_retained_page_guard_cancelled_new_locale_blocks_real_input_and_keep_button() {
    stopped_new_locale(false);
}

#[test]
fn localization_retained_page_guard_failed_new_locale_blocks_real_input_and_keeps_existing_fr_draft(
) {
    stopped_new_locale(true);
}

#[test]
fn localization_retained_page_guard_same_identity_cancel_and_failure_keep_real_typing_focus() {
    let (mut project, mut state) = translated();
    let ctx = egui::Context::default();
    click_label(&ctx, &mut project, &mut state, OLD);
    ctrl_a(&ctx, &mut project, &mut state);
    let key = editing::draft_key("zh-Hant", "line0");
    let id = egui::Id::new(("localization-part-text", &project.root, &key, 0usize));
    for (failed, text) in [(false, "续"), (true, "写😀")] {
        failed_or_cancelled(&project, &mut state, failed, 1);
        draw(&ctx, &mut project, &mut state, vec![]);
        assert!(ctx.read_response(id).unwrap().enabled());
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
        draw(
            &ctx,
            &mut project,
            &mut state,
            vec![egui::Event::Text(text.into())],
        );
    }
    assert_eq!(
        state.workbench.drafts[&key].edit.translation_parts,
        [LocalizationPart::Text {
            text: "续写😀".into()
        }]
    );
}

fn assert_stopped_identity_disabled(
    project: &mut Project,
    state: &mut LocalizationUiState,
    version: u64,
) {
    let ctx = egui::Context::default();
    jobs::pump(&ctx, project, state, version);
    failed_or_cancelled(project, state, false, version);
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1680.0, 1300.0),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(ui, project, state, version);
            });
        },
    );
    assert!(!output.shapes.is_empty());
    let key = editing::draft_key("zh-Hant", "line0");
    let id = egui::Id::new(("localization-part-text", &project.root, &key, 0usize));
    assert!(
        !ctx.read_response(id).unwrap().enabled(),
        "stopped work must not relabel a retained page as an accepted result"
    );
}

#[test]
fn localization_retained_page_guard_cancelled_new_version_cannot_overwrite_newer_sidecar() {
    let (mut project, mut state) = translated();
    let source_baseline = state
        .workbench
        .page
        .as_ref()
        .unwrap()
        .source_baseline
        .clone();
    apply_translation(&mut project, "较新的已应用繁體譯文");
    let latest = project
        .query_localization_catalog(&worldline_core::localization::LocalizationCatalogQuery {
            target_locale: Some("zh-Hant".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        latest.source_baseline, source_baseline,
        "source identity alone cannot detect a changed translation"
    );
    let baseline = project.content_baseline();
    assert_stopped_identity_disabled(&mut project, &mut state, 2);
    assert_eq!(project.content_baseline(), baseline);
}

#[test]
fn localization_retained_page_guard_unaccepted_workspace_cannot_enable_actual_editor() {
    let (_, mut state) = translated();
    let (mut other, _) = fixture(1);
    assert_stopped_identity_disabled(&mut other, &mut state, 1);
    assert!(state.workbench.drafts.is_empty());
}
