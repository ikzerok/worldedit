use super::*;
use worldline_core::localization::{LocalizationEdit, LocalizationStatus};

pub(super) fn fixture(count: usize) -> (Project, LocalizationUiState) {
    let root = std::env::temp_dir().join(format!(
        "localization-workbench-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut project = Project::new(&root);
    let entry = project.entry.clone();
    let mut text = "event start\n".to_owned();
    for index in 0..count {
        text.push_str(&format!("  源文 {index} 👋 #wl-localization:line{index}\n"));
    }
    text.push_str("  -> END\n");
    project.set_text(&entry, text).unwrap();
    project
        .create_authoring_document(
            &root.join(".world/project.json"),
            br#"{
        "schema_version":1,"language_version":"1.10","required_features":["content.localization.v1"]
    }"#
            .to_vec(),
        )
        .unwrap();
    let state = LocalizationUiState {
        source_locale: "en".into(),
        target_locale: "zh-Hant".into(),
        ..Default::default()
    };
    (project, state)
}
pub(super) fn stage(state: &mut LocalizationUiState, text: &str) {
    let page = state.workbench.page.as_ref().unwrap();
    let entry = &page.entries[0];
    let id = entry.id.clone().unwrap();
    state.workbench.drafts.insert(
        editing::draft_key("zh-Hant", &id),
        editing::DraftBuffer {
            source_locale: "en".into(),
            target_locale: "zh-Hant".into(),
            source_baseline: page.source_baseline.clone(),
            edit: LocalizationEdit {
                id,
                source_revision: entry.source_revision.clone().unwrap(),
                translation_parts: vec![LocalizationPart::Text { text: text.into() }],
            },
        },
    );
}
pub(super) fn frame(
    ctx: &egui::Context,
    project: &mut Project,
    state: &mut LocalizationUiState,
    size: egui::Vec2,
    events: Vec<egui::Event>,
) -> (bool, egui::FullOutput) {
    let mut applied = false;
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                applied = show(ui, project, state, 1);
            });
        },
    );
    (applied, output)
}
fn find(shape: &egui::Shape, text: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(t) if t.galley.text() == text => Some(t.pos + t.galley.size() * 0.5),
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| find(s, text)),
        _ => None,
    }
}
pub(super) fn click(
    ctx: &egui::Context,
    project: &mut Project,
    state: &mut LocalizationUiState,
    label: &str,
) -> bool {
    let size = egui::vec2(1200.0, 900.0);
    for _ in 0..2 {
        frame(ctx, project, state, size, vec![]);
    }
    let (_, output) = frame(ctx, project, state, size, vec![]);
    let point = output
        .shapes
        .iter()
        .rev()
        .find_map(|s| find(&s.shape, label).filter(|p| s.clip_rect.contains(*p)))
        .expect(label);
    frame(
        ctx,
        project,
        state,
        size,
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(
        ctx,
        project,
        state,
        size,
        vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    )
    .0
}

#[test]
fn localization_workbench_counts_pages_and_stable_frames_use_one_core_query() {
    let (project, mut state) = fixture(85);
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    let page = state.workbench.page.as_ref().unwrap();
    assert_eq!(
        (page.total, page.all_total, page.entries.len()),
        (85, 85, 40)
    );
    assert_eq!(
        page.status_counts
            .get(&LocalizationStatus::MissingTranslation),
        Some(&85)
    );
    for _ in 0..100 {
        catalog::refresh(&project, &mut state, 1);
        jobs::settle(&project, &mut state, 1);
    }
    assert_eq!(state.workbench.query_count, 1);
    state.workbench.offset = 40;
    state.workbench.invalidate();
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    assert_eq!(state.workbench.page.as_ref().unwrap().entries.len(), 40);
    state.workbench.offset = 80;
    state.workbench.invalidate();
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    let page = state.workbench.page.as_ref().unwrap();
    assert_eq!(page.entries.len(), 5);
    assert_eq!(page.next_offset, None);
    state.workbench.search = "源文 84".into();
    state.workbench.offset = 0;
    state.workbench.invalidate();
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    let page = state.workbench.page.as_ref().unwrap();
    assert_eq!((page.total, page.all_total), (1, 85));
    assert_eq!(page.entries[0].id.as_deref(), Some("line84"));
}

#[test]
fn localization_workbench_drafts_survive_filter_page_locale_and_exact_runtime_return() {
    let (project, mut state) = fixture(85);
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    stage(&mut state, "中文译文 👋\n第二行");
    assert!(state.has_unsubmitted_work());
    state.workbench.offset = 40;
    state.workbench.invalidate();
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    state.target_locale = "fr".into();
    state.workbench.offset = 0;
    state.workbench.invalidate();
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    assert_eq!(state.workbench.drafts.len(), 1);
    assert!(state.has_unsubmitted_work());
    state.open_translation("zh-Hant", Some("line84"));
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    let entry = state.workbench.selected_entry().unwrap();
    assert_eq!(entry.id.as_deref(), Some("line84"));
    assert!(state.has_unsubmitted_work());
    assert_eq!(
        state
            .workbench
            .drafts
            .values()
            .next()
            .unwrap()
            .edit
            .translation_parts,
        [LocalizationPart::Text {
            text: "中文译文 👋\n第二行".into()
        }]
    );
}

#[test]
fn localization_workbench_cancel_apply_undo_redo_save_and_reopen() {
    let (mut project, mut state) = fixture(1);
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    stage(&mut state, "Hello 👋\nSecond line");
    let before = project.content_baseline();
    let before_sources = project.sources();
    let manifest = project.root.join(".world/project.json");
    let before_manifest = project
        .authoring_document(&manifest)
        .unwrap()
        .bytes()
        .to_vec();
    plans::preview_edits(&project, &mut state);
    jobs::settle(&project, &mut state, 1);
    assert!(
        matches!(&state.workbench.preview, Some(plans::Preview::Edits { plan, .. }) if plan.can_apply)
    );
    let ctx = egui::Context::default();
    assert!(!click(&ctx, &mut project, &mut state, "取消预览，保留输入"));
    assert_eq!(project.content_baseline(), before);
    assert_eq!(state.workbench.drafts.len(), 1);
    plans::preview_edits(&project, &mut state);
    jobs::settle(&project, &mut state, 1);
    assert!(click(
        &ctx,
        &mut project,
        &mut state,
        "应用到工程（可撤销）"
    ));
    let applied = project.clone();
    let after = project.content_baseline();
    assert_ne!(after, before);
    assert!(!project
        .root
        .join(".world/localization/zh-Hant.json")
        .exists());
    assert!(project.restore(state.take_applied_before().unwrap()));
    // Project keeps recoverable tombstones for newly created documents; active content must match.
    assert_eq!(project.sources(), before_sources);
    assert_eq!(
        project.authoring_document(&manifest).unwrap().bytes(),
        before_manifest
    );
    assert!(project
        .authoring_document(&project.root.join(".world/localization/zh-Hant.json"))
        .unwrap()
        .is_deleted());
    assert!(project.restore(applied));
    assert_eq!(project.content_baseline(), after);
    project.save().unwrap();
    let reopened = Project::open(&project.entry).unwrap();
    assert_eq!(reopened.content_baseline(), project.content_baseline());
    std::fs::remove_dir_all(&project.root).unwrap();
}

#[test]
fn localization_workbench_stale_preview_retains_input_and_never_writes() {
    let (mut project, mut state) = fixture(1);
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    stage(&mut state, "Translation");
    plans::preview_edits(&project, &mut state);
    jobs::settle(&project, &mut state, 1);
    let entry = project.entry.clone();
    let changed = project
        .document(&entry)
        .unwrap()
        .replace("源文 0", "修改后的源文");
    project.set_text(&entry, changed).unwrap();
    let baseline = project.content_baseline();
    let ctx = egui::Context::default();
    assert!(!click(
        &ctx,
        &mut project,
        &mut state,
        "应用到工程（可撤销）"
    ));
    assert_eq!(project.content_baseline(), baseline);
    assert_eq!(state.workbench.drafts.len(), 1);
    assert!(state.status.as_ref().is_some_and(Result::is_err));
    assert!(!project.root.exists());
}

#[test]
fn localization_workbench_id_plan_cancel_and_apply_changes_only_one_source() {
    let (mut project, mut state) = fixture(1);
    let entry_path = project.entry.clone();
    project
        .set_text(&entry_path, "event start\n  待译正文 👋\n  -> END\n".into())
        .unwrap();
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    let entry = state.workbench.selected_entry().unwrap().clone();
    assert_eq!(entry.status, LocalizationStatus::MissingId);
    state
        .workbench
        .id_inputs
        .insert(entry.unit_key.clone(), "greeting".into());
    plans::preview_id(&project, &mut state, &entry);
    jobs::settle(&project, &mut state, 1);
    let ctx = egui::Context::default();
    assert!(click(
        &ctx,
        &mut project,
        &mut state,
        "应用到工程（可撤销）"
    ));
    assert!(project
        .document(&entry_path)
        .unwrap()
        .contains("#wl-localization:greeting"));
    assert!(state.workbench.id_inputs.is_empty());
    assert!(!project.root.exists());
}

#[test]
fn localization_workbench_narrow_200_percent_has_reachable_primary_actions() {
    let (mut project, mut state) = fixture(2);
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(2.0);
    // Settle the scale/area measurement before asserting final, visible geometry.
    for _ in 0..3 {
        frame(
            &ctx,
            &mut project,
            &mut state,
            egui::vec2(420.0, 360.0),
            vec![],
        );
    }
    jobs::settle(&project, &mut state, 1);
    let (_, output) = frame(
        &ctx,
        &mut project,
        &mut state,
        egui::vec2(420.0, 360.0),
        vec![],
    );
    for label in [
        "译文目录",
        "高级 JSON 交换",
        "字符串目录",
        "当前源文与译文",
        "刷新目录",
    ] {
        assert!(
            output
                .shapes
                .iter()
                .any(|s| find(&s.shape, label).is_some_and(|p| s.clip_rect.contains(p))),
            "{label}; visible shapes: {:?}",
            output
                .shapes
                .iter()
                .filter_map(|s| match &s.shape {
                    egui::Shape::Text(t) => Some((t.galley.text(), t.pos, s.clip_rect)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        );
    }
    assert!(state.workbench.query_error.is_none());
}
