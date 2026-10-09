//! All explicit manuscript discard hosts end the same draft-history lifecycle.
use super::body_apply_discard::disk_files;
use super::history_cases::{
    apply_link, apply_locale, apply_locale_to, assert_buffers, dirty_buffers, locale_fixture,
    open_body,
};
use super::*;
use crate::app::Pending;
use std::sync::Arc;

fn large_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    frame_size(ctx, app, vec2(1700.0, 1600.0), events)
}

fn label_position(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .find_map(|shape| {
            let mut texts = Vec::new();
            text_shapes(&shape.shape, &mut texts);
            texts.into_iter().find_map(|text| {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                (text.galley.text() == label && shape.clip_rect.contains_rect(rect))
                    .then_some(rect.center())
            })
        })
        .unwrap_or_else(|| panic!("missing fully visible control {label}: {}", labels(output)))
}

fn large_click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for _ in 0..3 {
        large_frame(ctx, app, vec![]);
    }
    let point = label_position(&large_frame(ctx, app, vec![]), label);
    for pressed in [true, false] {
        large_frame(
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
fn discard_at_later_locale_node_removes_older_file_entries_and_releases_witness() {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    let disk_before = disk_files(&root);
    let initial = app.project.sources();
    apply_locale(&ctx, &mut app);
    dirty_buffers(&ctx, &mut app);
    apply_link(&ctx, &mut app, false);
    let old_node = app.history_state.current;
    let witness = Arc::downgrade(&app.search_state.undo.last().unwrap().guard);
    let path = app.project.entry.clone();
    let other: Vec<_> = app
        .manuscript
        .writing_buffers()
        .into_iter()
        .filter(|buffer| buffer.path() != path.as_path())
        .collect();
    apply_locale_to(&ctx, &mut app, "zh-Hans");
    assert_ne!(app.history_state.current, old_node);
    assert_eq!(app.history.len(), 2);
    assert_eq!(app.search_state.undo.len(), 1);
    open_body(&ctx, &mut app);
    app.manuscript.reader_open = false;
    large_click(&ctx, &mut app, "丢弃此文件草稿");
    large_click(&ctx, &mut app, "确认丢弃正文草稿");
    assert_eq!(app.history.len(), 2);
    assert!(
        witness.upgrade().is_none(),
        "discard must release the obsolete entry and cache"
    );
    app.edit_undo(false);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history_state.current, old_node);
    app.edit_undo(false); // The discarded file's old-node C must not intercept A1.
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert!(app.history.is_empty());
    assert_eq!(app.project.sources(), initial);
    assert_buffers(&app, &other);
    for _ in 0..2 {
        app.edit_undo(true);
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
    }
    assert_eq!(app.history.len(), 2);
    assert!(app.search_state.undo.is_empty() && app.search_state.redo.is_empty());
    assert_eq!(disk_files(&root), disk_before);
    fs::remove_dir_all(root).unwrap();
}

fn two_file_fixture() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = locale_fixture();
    let remote = app.project.root.join("资料.wl");
    let book = app.project.root.join(".world/manuscripts/book.json");
    // Saved fixture setup, before any operation under test or operation history.
    app.project
        .set_text(
            &remote,
            "event remote\n  我和林芜😀望向远岸。\n  -> END\n".into(),
        )
        .unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&book).unwrap().bytes()).unwrap();
    value["entries"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": "remote", "kind": "chapter", "title": "Remote",
            "target_ref": {"kind": "event", "id": "remote"}
        }));
    app.project
        .set_authoring_document(&book, serde_json::to_vec(&value).unwrap())
        .unwrap();
    app.project.save().unwrap();
    app.reset_views();
    app.recompile();
    open_body(&ctx, &mut app);
    (ctx, app)
}

fn apply_remote_reference(ctx: &egui::Context, app: &mut WorldeditApp) {
    large_click(ctx, app, "Remote");
    large_frame(ctx, app, vec![]);
    let path = app.project.root.join("资料.wl");
    let buffer = &app.manuscript.writing_buffers[&path];
    let offset = buffer.source().find("我和林芜😀").unwrap();
    let id = egui::Id::new(("writing-prose", &path, "event", "remote", offset));
    let mut editor = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    editor
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(2),
            egui::text::CCursor::new(5),
        )));
    editor.store(ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
    large_frame(ctx, app, vec![]);
    let selection = crate::app::search::editor_selection(ctx).unwrap();
    assert_eq!(selection.path, path);
    assert_eq!(&selection.source[selection.range], "林芜😀");
    app.begin_manuscript_world_links(ctx);
    let mut state = app.manuscript.world_links.take().unwrap();
    state.chosen = Some(TargetRef::new("character", "lin"));
    app.preview_manuscript_world_link(&mut state);
    assert!(state.plan.is_some(), "{:?}", state.error);
    assert!(
        app.apply_manuscript_world_link(ctx, &mut state),
        "{:?}",
        state.error
    );
}

#[test]
fn discarding_one_file_preserves_other_files_real_reference_redo_and_undo() {
    let (ctx, mut app) = two_file_fixture();
    let root = app.project.root.clone();
    let disk_before = disk_files(&root);
    let initial = app.project.sources();
    apply_locale(&ctx, &mut app);
    apply_link(&ctx, &mut app, false);
    let first = app.project.entry.clone();
    let remote = root.join("资料.wl");
    let remote_before = app.project.open_source_writing_buffer(&remote).unwrap();
    apply_remote_reference(&ctx, &mut app);
    let remote_after = app.manuscript.writing_buffers[&remote].clone();
    assert_eq!(app.search_state.undo.len(), 2);
    let witness = Arc::downgrade(&app.search_state.undo.last().unwrap().guard);
    assert!(Arc::ptr_eq(
        &app.search_state.undo[0].guard,
        &app.search_state.undo[1].guard
    ));
    app.edit_undo(false); // Keep the second file's operation in draft redo.
    assert_buffers(&app, std::slice::from_ref(&remote_before));
    large_click(&ctx, &mut app, "First");
    app.manuscript.reader_open = false;
    large_click(&ctx, &mut app, "丢弃此文件草稿");
    large_click(&ctx, &mut app, "确认丢弃正文草稿");
    assert!(!app
        .manuscript
        .writing_buffers
        .get(&first)
        .is_some_and(|buffer| buffer.is_changed()));
    assert!(app.search_state.undo.is_empty());
    assert_eq!(app.search_state.redo.len(), 1);
    assert!(
        witness.upgrade().is_some(),
        "the other file still owns its shared guard"
    );
    app.edit_undo(true);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_buffers(&app, std::slice::from_ref(&remote_after));
    app.edit_undo(false);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_buffers(&app, std::slice::from_ref(&remote_before));
    app.edit_undo(false); // Earlier A1 is still reachable.
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert!(app.history.is_empty());
    assert_eq!(app.project.sources(), initial);
    assert_eq!(disk_files(&root), disk_before);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn orphaned_file_discard_uses_real_keep_and_confirm_buttons_without_clearing_other_input() {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    let disk_before = disk_files(&root);
    apply_locale(&ctx, &mut app);
    dirty_buffers(&ctx, &mut app);
    apply_link(&ctx, &mut app, false);
    let linked = app.manuscript.writing_buffers();
    let path = app.project.entry.clone();
    let other: Vec<_> = linked
        .iter()
        .filter(|buffer| buffer.path() != path.as_path())
        .cloned()
        .collect();
    let baseline = app.project.content_baseline();
    let witness = Arc::downgrade(&app.search_state.undo.last().unwrap().guard);
    app.manuscript.reader_open = false;
    large_click(&ctx, &mut app, "编排与来源");
    large_click(&ctx, &mut app, "删除编排项");
    large_click(&ctx, &mut app, "确认只删除编排");
    assert!(app.manuscript.books["book"].selected_entry.is_none());
    assert!(app.manuscript.books["book"].changed);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(labels(&large_frame(&ctx, &mut app, vec![])).contains("保留的正文草稿"));
    large_click(&ctx, &mut app, "world.wl");
    large_click(&ctx, &mut app, "丢弃这份保留草稿");
    large_click(&ctx, &mut app, "继续保留草稿");
    assert_buffers(&app, &linked);
    assert_eq!(app.search_state.undo.len(), 1);
    large_click(&ctx, &mut app, "丢弃这份保留草稿");
    large_click(&ctx, &mut app, "确认丢弃保留草稿");
    assert!(!app.manuscript.writing_buffers.contains_key(&path));
    assert_buffers(&app, &other);
    assert!(
        app.manuscript.books["book"].changed,
        "discarding text must preserve the separate arrangement draft"
    );
    assert_eq!(app.history.len(), 1);
    assert!(witness.upgrade().is_none());
    app.edit_undo(false);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert!(app.history.is_empty());
    assert!(app.search_state.undo.is_empty() && app.search_state.redo.is_empty());
    assert_buffers(&app, &other);
    assert_eq!(disk_files(&root), disk_before);
    fs::remove_dir_all(root).unwrap();
}

fn exit_frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 1000.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            app.draft_exit_dialog(ctx);
            app.dialogs(ctx);
        },
    )
}

fn exit_click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for _ in 0..3 {
        exit_frame(ctx, app, vec![]);
    }
    let point = label_position(&exit_frame(ctx, app, vec![]), label);
    for pressed in [true, false] {
        exit_frame(
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
fn global_discard_dialog_cancels_then_ends_all_draft_history_without_consuming_project_edges() {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    let disk_before = disk_files(&root);
    apply_locale(&ctx, &mut app);
    dirty_buffers(&ctx, &mut app);
    apply_link(&ctx, &mut app, false);
    let linked = app.manuscript.writing_buffers();
    let witness = Arc::downgrade(&app.search_state.undo.last().unwrap().guard);
    let baseline = app.project.content_baseline();
    assert!(
        app.project.is_dirty(),
        "the headless close request must stop at unsaved-project confirmation"
    );
    app.request_action(Pending::Close, &ctx);
    assert!(app.draft_action.is_some());
    exit_click(&ctx, &mut app, "取消离开");
    assert!(app.draft_action.is_none());
    assert_buffers(&app, &linked);
    assert_eq!(app.search_state.undo.len(), 1);
    assert!(witness.upgrade().is_some());
    app.request_action(Pending::Close, &ctx);
    exit_click(&ctx, &mut app, "丢弃未应用输入并继续");
    assert!(app.draft_action.is_none());
    assert!(app.pending.is_some());
    assert!(!app.allow_close, "no native window close is executed");
    assert!(app.manuscript.writing_buffers().is_empty());
    assert!(app.search_state.undo.is_empty() && app.search_state.redo.is_empty());
    assert!(witness.upgrade().is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.history.len(), 1);
    exit_click(&ctx, &mut app, "取消"); // Real unsaved-project modal; retain the project.
    assert!(app.pending.is_none());
    assert!(!app.allow_close);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.history.len(), 1);
    app.edit_undo(false);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert!(app.history.is_empty());
    assert_eq!(disk_files(&root), disk_before);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn reset_views_drops_project_and_draft_histories_and_cached_witness() {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    let disk_before = disk_files(&root);
    apply_locale(&ctx, &mut app);
    dirty_buffers(&ctx, &mut app);
    apply_link(&ctx, &mut app, false);
    let witness = Arc::downgrade(&app.search_state.undo.last().unwrap().guard);
    let baseline = app.project.content_baseline();
    app.reset_views();
    assert!(app.history.is_empty() && app.redo.is_empty());
    assert!(app.search_state.undo.is_empty() && app.search_state.redo.is_empty());
    assert!(app.manuscript.writing_buffers().is_empty());
    assert!(witness.upgrade().is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(disk_files(&root), disk_before);
    fs::remove_dir_all(root).unwrap();
}
