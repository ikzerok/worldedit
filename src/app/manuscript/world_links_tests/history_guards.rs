//! Branches, bounded history, and failed travel never borrow another operation's input.
use super::history_cases::{
    apply_link, assert_buffers, dirty_buffers, locale_fixture, ordinary_edit,
};
use super::*;

fn counts(app: &WorldeditApp) -> (usize, usize, usize, usize) {
    (
        app.history.len(),
        app.redo.len(),
        app.search_state.undo.len(),
        app.search_state.redo.len(),
    )
}

#[test]
fn full_history_limit_does_not_make_draft_undo_intercept_a_new_project_operation() {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    for number in 0..40 {
        ordinary_edit(&mut app, number);
    }
    assert_eq!(app.history.len(), 40);
    let originals = dirty_buffers(&ctx, &mut app);
    apply_link(&ctx, &mut app, false);
    let linked = app.manuscript.writing_buffers();
    let node = app.history_state.current;
    ordinary_edit(&mut app, 40);
    assert_eq!(app.history.len(), 40);
    assert_ne!(app.history_state.current, node);
    app.edit_undo(false);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), 39);
    assert_eq!(app.history_state.current, node);
    assert_eq!(app.search_state.undo.len(), 1);
    assert_eq!(
        app.project.document(&root.join("unrelated.wl")).unwrap(),
        "// unrelated 39\n"
    );
    assert_buffers(&app, &linked);
    app.edit_undo(false);
    assert_eq!(app.history.len(), 39);
    assert_buffers(&app, &originals);
    app.edit_undo(true);
    assert_buffers(&app, &linked);
    app.edit_undo(true);
    assert_eq!(app.history.len(), 40);
    assert_eq!(
        app.project.document(&root.join("unrelated.wl")).unwrap(),
        "// unrelated 40\n"
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn trimming_drops_only_unreachable_draft_witnesses_and_compound_payloads() {
    for creates in [false, true] {
        let (ctx, mut app) = locale_fixture();
        let root = app.project.root.clone();
        dirty_buffers(&ctx, &mut app);
        apply_link(&ctx, &mut app, creates);
        let linked_sources = app.project.sources();
        let linked_buffers = app.manuscript.writing_buffers();
        let witness = app
            .search_state
            .undo
            .last()
            .map(|entry| std::sync::Arc::downgrade(&entry.guard));
        let count = if creates { 40 } else { 41 };
        for number in 0..count {
            ordinary_edit(&mut app, number);
        }
        assert_eq!(app.history.len(), 40);
        assert!(app.search_state.undo.is_empty());
        if let Some(witness) = witness {
            assert!(witness.upgrade().is_none());
        }
        for _ in 0..40 {
            app.edit_undo(false);
            assert!(app.io_error.is_none(), "{:?}", app.io_error);
        }
        assert!(app.history.is_empty());
        if creates {
            assert_eq!(app.project.sources(), linked_sources);
        } else {
            assert_eq!(
                app.project.document(&root.join("unrelated.wl")).unwrap(),
                "// unrelated 0\n"
            );
        }
        assert_buffers(&app, &linked_buffers);
        let before = app.project.content_baseline();
        app.edit_undo(false);
        assert_eq!(app.project.content_baseline(), before);
        assert_buffers(&app, &linked_buffers);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn later_input_even_with_identical_bytes_blocks_repeated_undo_or_redo_without_consuming() {
    for creates in [false, true] {
        for forward in [false, true] {
            let (ctx, mut app) = locale_fixture();
            let root = app.project.root.clone();
            let path = app.project.entry.clone();
            dirty_buffers(&ctx, &mut app);
            apply_link(&ctx, &mut app, creates);
            if forward {
                app.edit_undo(false);
                assert!(app.io_error.is_none());
            }
            let before = app.project.content_baseline();
            let history = counts(&app);
            let buffer = app.manuscript.writing_buffers.get_mut(&path).unwrap();
            let original = buffer.source().to_owned();
            buffer.replace_source(format!("{original}\n// 新输入不许覆盖\n"));
            // Redo and draft-only Undo must notice later editing even if bytes return.
            if forward || !creates {
                buffer.replace_source(original);
            }
            let keep = app.manuscript.writing_buffers();
            for _ in 0..2 {
                app.edit_undo(forward);
                assert!(app.io_error.is_some());
                assert_eq!(app.project.content_baseline(), before);
                assert_eq!(counts(&app), history);
                assert_buffers(&app, &keep);
            }
            fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn external_refresh_rejects_each_history_kind_without_consuming_or_replacing_input() {
    for creates in [false, true] {
        for forward in [false, true] {
            let (ctx, mut app) = locale_fixture();
            let root = app.project.root.clone();
            dirty_buffers(&ctx, &mut app);
            apply_link(&ctx, &mut app, creates);
            if forward {
                app.edit_undo(false);
                assert!(app.io_error.is_none());
            }
            let keep = app.manuscript.writing_buffers();
            let history = counts(&app);
            let node = app.history_state.current;
            // Core refresh arrives before the host clears stale history. Its restore guard
            // must independently reject this race; no forged history entries are involved.
            fs::write(root.join("unrelated.wl"), "// 外部真实磁盘改稿\n").unwrap();
            assert!(app.project.refresh().unwrap().is_empty());
            let refreshed = app.project.content_baseline();
            for _ in 0..2 {
                app.edit_undo(forward);
                assert!(
                    app.io_error
                        .as_ref()
                        .is_some_and(|error| error.contains("外部刷新")),
                    "{:?}",
                    app.io_error
                );
                assert_eq!(app.project.content_baseline(), refreshed);
                assert_eq!(counts(&app), history);
                assert_eq!(app.history_state.current, node);
                for original in &keep {
                    let current = &app.manuscript.writing_buffers[original.path()];
                    assert_eq!(current.source(), original.source());
                    assert_eq!(current.generation(), original.generation());
                    assert_eq!(current.baseline(), original.baseline());
                }
            }
            // The actual refresh host invalidates identities but preserves author input.
            app.clear_edit_history();
            assert_ne!(app.history_state.current, node);
            assert_eq!(counts(&app), (0, 0, 0, 0));
            for original in &keep {
                let current = &app.manuscript.writing_buffers[original.path()];
                assert_eq!(current.source(), original.source());
                assert_eq!(current.generation(), original.generation());
            }
            fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn draft_branch_clears_project_and_draft_redo_while_reusing_same_node_guard() {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    dirty_buffers(&ctx, &mut app);
    apply_link(&ctx, &mut app, false);
    let guard = std::sync::Arc::clone(&app.search_state.undo.last().unwrap().guard);
    ordinary_edit(&mut app, 0);
    app.edit_undo(false);
    assert_eq!(app.redo.len(), 1);
    app.edit_undo(false);
    assert_eq!(app.search_state.redo.len(), 1);
    apply_link(&ctx, &mut app, false);
    assert!(app.redo.is_empty());
    assert!(app.search_state.redo.is_empty());
    assert!(std::sync::Arc::ptr_eq(
        &guard,
        &app.search_state.undo.last().unwrap().guard
    ));
    let keep = app.manuscript.writing_buffers();
    app.edit_undo(true);
    assert_buffers(&app, &keep);
    fs::remove_dir_all(root).unwrap();
}
