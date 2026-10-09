//! A local sidecar transaction must hand the existing full draft to its new baseline.
use super::body_apply_discard::disk_files;
use super::cross_feature_undo::{click_localization, localization_frame};
use super::history_cases::{
    apply_link, apply_locale_to, assert_buffers, dirty_buffers, locale_fixture, open_body,
};
use super::*;

fn prepare_typed_locale(ctx: &egui::Context, app: &mut WorldeditApp) {
    app.tab = Tab::Localization;
    app.localization_ui.source_locale = "en".into();
    app.localization_ui.open_translation("fr", Some("body"));
    localization_frame(ctx, app, Vec::new());
    app.localization_ui
        .settle_pending_for_test(&app.project, app.version);
    localization_frame(ctx, app, Vec::new());
    let id = egui::Id::new((
        "localization-part-text",
        &app.project.root,
        "fr\u{1f}body",
        0usize,
    ));
    let point = ctx
        .read_response(id)
        .expect("real typed translation editor")
        .rect
        .center();
    for pressed in [true, false] {
        localization_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    for pressed in [true, false] {
        localization_frame(
            ctx,
            app,
            vec![Event::Key {
                key: egui::Key::A,
                physical_key: Some(egui::Key::A),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
        );
    }
    localization_frame(ctx, app, vec![Event::Text("REF_A1_FR 中文😀".into())]);
    assert!(!click_localization(ctx, app, "预览 1 项译文"));
}

fn exercise_sidecar_handoff(typed: bool, source_mode: bool) {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    let path = app.project.entry.clone();
    let sidecar = root.join(".world/localization/fr.json");
    let disk_before = disk_files(&root);
    let project_sources = app.project.sources();
    let originals = dirty_buffers(&ctx, &mut app);
    apply_link(&ctx, &mut app, false);
    let linked = app.manuscript.writing_buffers();
    let linked_source = app.manuscript.writing_buffers[&path].source().to_owned();
    let before_locale = app.project.content_baseline();
    assert_eq!(app.history.len(), 0);
    assert_eq!(app.search_state.undo.len(), 1);

    // Real A1 preview/confirmation and its host remember path. No Save, Undo,
    // discard, buffer replacement or test-side rebase may repair the handoff.
    if typed {
        prepare_typed_locale(&ctx, &mut app);
        assert!(click_localization(&ctx, &mut app, "应用到工程（可撤销）"));
    } else {
        apply_locale_to(&ctx, &mut app, "fr");
    }
    assert_ne!(app.project.content_baseline(), before_locale);
    assert_eq!(app.project.sources(), project_sources);
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.search_state.undo.len(), 1);
    assert_buffers(&app, &linked);
    let translation = app
        .project
        .authoring_document(&sidecar)
        .unwrap()
        .bytes()
        .to_vec();
    assert!(!sidecar.exists());

    open_body(&ctx, &mut app);
    assert_buffers(&app, &linked);
    let rendered = labels(&frame(&ctx, &mut app, Vec::new()));
    assert!(!rendered.contains("STALE_DRAFT"), "{rendered}");
    app.manuscript.reader_open = false;
    if source_mode {
        click(&ctx, &mut app, "源码");
    }
    click(
        &ctx,
        &mut app,
        if source_mode {
            "应用源码草稿（可含诊断）"
        } else {
            "应用正文草稿"
        },
    );
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), 2);
    assert_eq!(app.project.document(&path).unwrap(), linked_source);
    assert_eq!(disk_files(&root), disk_before);

    for _ in 0..2 {
        app.edit_undo(false); // Body Apply restores D1 at the A1 node.
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        assert_eq!(app.project.sources(), project_sources);
        assert_buffers(&app, &linked);
        app.edit_undo(false); // A1 restores D1 at the C draft node.
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        assert_buffers(&app, &linked);
        assert!(app
            .project
            .authoring_document(&sidecar)
            .unwrap()
            .is_deleted());
        app.edit_undo(false); // C restores the complete original full drafts.
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        assert_buffers(&app, &originals);
        assert!(app.history.is_empty());
        app.edit_undo(true);
        assert_buffers(&app, &linked);
        app.edit_undo(true);
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        assert_buffers(&app, &linked);
        assert_eq!(
            app.project.authoring_document(&sidecar).unwrap().bytes(),
            translation
        );
        app.edit_undo(true);
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        assert_eq!(app.project.document(&path).unwrap(), linked_source);
        assert_eq!(app.history.len(), 2);
        assert!(app.redo.is_empty());
        assert!(app.search_state.redo.is_empty());
        let other: Vec<_> = linked
            .iter()
            .filter(|buffer| buffer.path() != path.as_path())
            .cloned()
            .collect();
        assert_buffers(&app, &other);
        assert_eq!(disk_files(&root), disk_before);
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reference_then_typed_locale_handoff_keeps_body_apply_and_three_step_history_live() {
    exercise_sidecar_handoff(true, false);
}

#[test]
fn reference_then_typed_locale_handoff_keeps_source_apply_and_three_step_history_live() {
    exercise_sidecar_handoff(true, true);
}

#[test]
fn reference_then_import_locale_handoff_keeps_body_apply_and_three_step_history_live() {
    exercise_sidecar_handoff(false, false);
}

#[test]
fn reference_then_import_locale_handoff_keeps_source_apply_and_three_step_history_live() {
    exercise_sidecar_handoff(false, true);
}

#[test]
fn typed_locale_handoff_cancel_preserves_complete_drafts_and_both_histories() {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    let disk_before = disk_files(&root);
    dirty_buffers(&ctx, &mut app);
    apply_link(&ctx, &mut app, false);
    let linked = app.manuscript.writing_buffers();
    let baseline = app.project.content_baseline();
    let node = app.history_state.current;
    prepare_typed_locale(&ctx, &mut app);
    assert!(!click_localization(&ctx, &mut app, "取消预览，保留输入"));
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.history_state.current, node);
    assert!(app.history.is_empty() && app.redo.is_empty());
    assert_eq!(app.search_state.undo.len(), 1);
    assert!(app.search_state.redo.is_empty());
    assert!(app.localization_ui.has_unsubmitted_work());
    assert_buffers(&app, &linked);
    assert_eq!(disk_files(&root), disk_before);
    fs::remove_dir_all(root).unwrap();
}
