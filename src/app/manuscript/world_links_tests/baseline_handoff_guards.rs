//! Handoff cannot certify changed sources, older refresh epochs, or later input.
use super::body_apply_discard::disk_files;
use super::cross_feature_undo::{click_localization, localization_frame};
use super::history_cases::{
    apply_link, apply_locale_to, assert_buffers, dirty_buffers, locale_fixture, open_body,
};
use super::*;
use worldline_core::manuscript::WritingBuffer;

fn counts(app: &WorldeditApp) -> (usize, usize, usize, usize) {
    (
        app.history.len(),
        app.redo.len(),
        app.search_state.undo.len(),
        app.search_state.redo.len(),
    )
}

fn unchanged_buffers(app: &WorldeditApp, expected: &[WritingBuffer]) {
    for previous in expected {
        let current = &app.manuscript.writing_buffers[previous.path()];
        assert_eq!(current.source(), previous.source());
        assert_eq!(current.generation(), previous.generation());
        assert_eq!(current.is_changed(), previous.is_changed());
        assert_eq!(current.baseline(), previous.baseline());
    }
}

fn open_locale(ctx: &egui::Context, app: &mut WorldeditApp) {
    app.tab = Tab::Localization;
    app.localization_ui.source_locale = "en".into();
    app.localization_ui.open_translation("fr", Some("body"));
    localization_frame(ctx, app, Vec::new());
    app.localization_ui
        .settle_pending_for_test(&app.project, app.version);
    localization_frame(ctx, app, Vec::new());
}

#[test]
fn source_changing_id_apply_cannot_rebase_the_old_body_but_keeps_other_file_live() {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    let path = app.project.entry.clone();
    let disk_before = disk_files(&root);
    let originals = dirty_buffers(&ctx, &mut app);
    apply_link(&ctx, &mut app, false);
    let linked = app.manuscript.writing_buffers();
    let old_body = app.manuscript.writing_buffers[&path].clone();
    open_locale(&ctx, &mut app);
    assert!(!click_localization(&ctx, &mut app, "稳定身份与来源"));
    assert!(!click_localization(
        &ctx,
        &mut app,
        "例如 chapter01_welcome"
    ));
    localization_frame(&ctx, &mut app, vec![Event::Text("body_renamed".into())]);
    assert!(!click_localization(&ctx, &mut app, "预览此 ID 修改"));
    assert!(click_localization(&ctx, &mut app, "应用到工程（可撤销）"));
    assert!(app
        .project
        .document(&path)
        .unwrap()
        .contains("#wl-localization:body_renamed"));
    unchanged_buffers(&app, &[old_body]);
    let other: Vec<_> = linked
        .iter()
        .filter(|buffer| buffer.path() != path.as_path())
        .cloned()
        .collect();
    assert_buffers(&app, &other);
    let baseline = app.project.content_baseline();
    let history = counts(&app);
    open_body(&ctx, &mut app);
    app.apply_manuscript_body(&path, false);
    assert!(app
        .io_error
        .as_ref()
        .is_some_and(|error| error.contains("基线已过期")));
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(counts(&app), history);
    app.edit_undo(false);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_buffers(&app, &linked);
    app.edit_undo(false);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_buffers(&app, &originals);
    assert_eq!(disk_files(&root), disk_before);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn external_disk_write_after_typed_preview_does_not_commit_or_rebase_any_draft() {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    dirty_buffers(&ctx, &mut app);
    apply_link(&ctx, &mut app, false);
    let linked = app.manuscript.writing_buffers();
    let baseline = app.project.content_baseline();
    let node = app.history_state.current;
    let history = counts(&app);
    open_locale(&ctx, &mut app);
    assert!(!click_localization(&ctx, &mut app, "保留此译文并待复核"));
    assert!(!click_localization(&ctx, &mut app, "预览 1 项译文"));
    fs::write(root.join("unrelated.wl"), "// 预览后的真实外部修改\n").unwrap();
    let disk_after_external = disk_files(&root);
    assert!(!click_localization(&ctx, &mut app, "应用到工程（可撤销）"));
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.history_state.current, node);
    assert_eq!(counts(&app), history);
    unchanged_buffers(&app, &linked);
    assert!(app.localization_ui.has_unsubmitted_work());
    assert_eq!(disk_files(&root), disk_after_external);
    assert!(!root.join(".world/localization/fr.json").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_locale_handoff_preserves_external_baseline_and_separate_history_epoch_guards() {
    for restore_same_content in [false, true] {
        let (ctx, mut app) = locale_fixture();
        let root = app.project.root.clone();
        let path = app.project.entry.clone();
        let unrelated = root.join("unrelated.wl");
        let original_disk = fs::read(&unrelated).unwrap();
        dirty_buffers(&ctx, &mut app);
        apply_link(&ctx, &mut app, false);
        let linked = app.manuscript.writing_buffers();
        let node = app.history_state.current;
        let baseline = app.project.content_baseline();
        fs::write(&unrelated, "// 真实外部代次变化\n").unwrap();
        assert!(app.project.refresh().unwrap().is_empty());
        if restore_same_content {
            fs::write(&unrelated, original_disk).unwrap();
            assert!(app.project.refresh().unwrap().is_empty());
            assert_eq!(app.project.content_baseline(), baseline);
        } else {
            assert_ne!(app.project.content_baseline(), baseline);
        }
        let witness = &app.search_state.undo.last().unwrap().guard;
        assert!(!app.project.clone().restore((**witness).clone()));
        apply_locale_to(&ctx, &mut app, "fr");
        assert_ne!(app.history_state.current, node);
        if restore_same_content {
            // WritingBuffer has no epoch identity: its unchanged original and
            // current baseline remain valid. The separate C history witness
            // must still reject the older refresh epoch after A1 is undone.
            assert_buffers(&app, &linked);
        } else {
            unchanged_buffers(&app, &linked);
            let before_apply = app.project.content_baseline();
            let history = counts(&app);
            app.apply_manuscript_body(&path, true);
            assert!(app.io_error.is_some());
            assert_eq!(app.project.content_baseline(), before_apply);
            assert_eq!(counts(&app), history);
            unchanged_buffers(&app, &linked);
        }
        app.edit_undo(false);
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        let preserved = app.manuscript.writing_buffers();
        let baseline = app.project.content_baseline();
        let history = counts(&app);
        for _ in 0..2 {
            app.edit_undo(false);
            assert!(app
                .io_error
                .as_ref()
                .is_some_and(|error| error.contains("外部刷新")));
            assert_eq!(app.project.content_baseline(), baseline);
            assert_eq!(counts(&app), history);
            unchanged_buffers(&app, &preserved);
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn input_after_successful_locale_handoff_still_blocks_the_original_reference_undo() {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    let path = app.project.entry.clone();
    let disk_before = disk_files(&root);
    dirty_buffers(&ctx, &mut app);
    apply_link(&ctx, &mut app, false);
    apply_locale_to(&ctx, &mut app, "fr");
    let source = app.manuscript.writing_buffers[&path].source().to_owned();
    app.manuscript
        .writing_buffers
        .get_mut(&path)
        .unwrap()
        .replace_source(format!("{source}\n// 后改必须完整保留😀\n"));
    let later = app.manuscript.writing_buffers();
    app.edit_undo(false); // The sidecar itself can travel without touching source input.
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_buffers(&app, &later);
    let baseline = app.project.content_baseline();
    let history = counts(&app);
    for _ in 0..2 {
        app.edit_undo(false);
        assert!(app
            .io_error
            .as_ref()
            .is_some_and(|error| error.contains("后续输入")));
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(counts(&app), history);
        assert_buffers(&app, &later);
    }
    assert_eq!(disk_files(&root), disk_before);
    fs::remove_dir_all(root).unwrap();
}

fn capability_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1800.0, 1400.0))),
            events,
            ..Default::default()
        },
        |ctx| app.capability_window(ctx),
    )
}

fn capability_click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for _ in 0..3 {
        capability_frame(ctx, app, Vec::new());
    }
    let output = capability_frame(ctx, app, Vec::new());
    let point = output
        .shapes
        .iter()
        .find_map(|shape| {
            let mut texts = Vec::new();
            text_shapes(&shape.shape, &mut texts);
            texts
                .into_iter()
                .filter(|text| text.galley.text() == label)
                .map(|text| text.pos + text.galley.rect.center().to_vec2())
                .find(|point| shape.clip_rect.contains(*point))
        })
        .unwrap_or_else(|| panic!("missing {label}: {}", labels(&output)));
    for pressed in [true, false] {
        capability_frame(
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
}

#[test]
fn capability_entry_still_explicitly_blocks_unapplied_body_and_cancel_keeps_histories() {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    let disk_before = disk_files(&root);
    dirty_buffers(&ctx, &mut app);
    apply_link(&ctx, &mut app, false);
    let linked = app.manuscript.writing_buffers();
    let baseline = app.project.content_baseline();
    let history = counts(&app);
    app.open_capabilities();
    for _ in 0..3 {
        capability_frame(&ctx, &mut app, Vec::new());
    }
    let rendered = labels(&capability_frame(&ctx, &mut app, Vec::new()));
    assert!(rendered.contains("以下输入尚待处理"), "{rendered}");
    assert!(rendered.contains("正文"), "{rendered}");
    capability_click(&ctx, &mut app, "预览全稿兼容影响");
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(counts(&app), history);
    assert_buffers(&app, &linked);
    capability_click(&ctx, &mut app, "取消 / 保留当前设置");
    assert!(app.capability_ui.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(counts(&app), history);
    assert_buffers(&app, &linked);
    assert_eq!(disk_files(&root), disk_before);
    fs::remove_dir_all(root).unwrap();
}
