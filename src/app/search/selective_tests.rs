//! Event-driven regressions; native layout and physical IME are separate checks.
use super::*;

fn three() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app();
    app.project.set_text(&app.active_file.clone(),
        "event start\n  needle needle\n  保留证词 needle\n  -> END\n".into()).unwrap();
    app.recompile();
    app.open_search(&ctx, false, true);
    app.project_query = "needle".into();
    app.search_state.replacement = "needle revised".into();
    (ctx, app)
}

#[test]
fn same_line_choices_replace_exactly_two_and_one_undo_restores_all() {
    let (_ctx, mut app) = three();
    let hits = app.current_search_hits().unwrap();
    assert_eq!(hits.len(), 3);
    let before = app.project.document(&app.active_file).unwrap().to_owned();
    app.choose_search_hit(&hits[0], true);
    app.choose_search_hit(&hits[1], true);
    app.search_state.selected = 2;
    assert_eq!(app.search_state.chosen.len(), 2, "navigation is not replacement selection");
    app.preview_selected_search_replacement();
    let plan = app.search_state.plan.as_ref().unwrap();
    assert_eq!(plan.hits, hits[..2]);
    assert_eq!(plan.occurrences.len(), 2);
    app.apply_search_replacement();
    let after = app.project.document(&app.active_file).unwrap().to_owned();
    assert!(after.contains("needle revised needle revised"));
    assert!(after.contains("保留证词 needle\n"));
    assert_eq!(after.matches("revised").count(), 2);
    assert_eq!(app.search_state.applied_count, Some(2));
    app.edit_undo(false);
    assert_eq!(app.project.document(&app.active_file).unwrap(), before);
    app.edit_undo(true);
    assert_eq!(app.project.document(&app.active_file).unwrap(), after);
}

#[test]
fn empty_all_clear_and_cancel_are_explicit_and_never_modify_source() {
    let (_ctx, mut app) = three();
    let baseline = app.project.content_baseline();
    app.preview_selected_search_replacement();
    assert!(app.search_state.plan.is_none());
    assert!(app.search_state.error.is_some());
    app.choose_all_search_hits();
    assert_eq!(app.search_state.chosen.len(), 3);
    app.preview_selected_search_replacement();
    assert!(app.search_state.plan.is_some());
    app.cancel_search_preview();
    assert!(app.search_state.plan.is_none());
    assert_eq!(app.search_state.chosen.len(), 3);
    app.clear_search_choices();
    app.apply_search_replacement();
    assert!(app.search_state.chosen.is_empty());
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn query_options_scope_and_source_changes_discard_old_choices_without_index_reuse() {
    for change in 0..6 {
        let (_ctx, mut app) = three();
        let hits = app.current_search_hits().unwrap();
        app.choose_search_hit(&hits[1], true);
        app.preview_selected_search_replacement();
        assert!(app.search_state.plan.is_some());
        match change {
            0 => app.project_query = "revised".into(),
            1 => app.search_state.options.case_sensitive = false,
            2 => app.search_state.options.whole_word = true,
            3 => app.search_state.source = true,
            4 => app.search_state.scope = Scope::Project,
            _ => {
                let text = app.project.document(&app.active_file).unwrap().replace("  needle", "  前文 needle");
                app.project.set_text(&app.active_file.clone(), text).unwrap();
            }
        }
        let hits = app.current_search_hits().unwrap();
        app.reconcile_search_review(&hits);
        assert!(app.search_state.chosen.is_empty(), "change {change}");
        assert!(app.search_state.plan.is_none());
        assert!(app.search_state.review_notice.as_deref().unwrap().contains("清空"));
    }
}

#[test]
fn replacement_and_chosen_set_invalidate_preview_but_preserve_matching_choices() {
    let (_ctx, mut app) = three();
    let hits = app.current_search_hits().unwrap();
    app.choose_search_hit(&hits[0], true);
    app.preview_selected_search_replacement();
    app.search_state.replacement = "different".into();
    app.reconcile_search_review(&app.current_search_hits().unwrap());
    assert_eq!(app.search_state.chosen, vec![hits[0].clone()]);
    assert!(app.search_state.plan.is_none());
    app.preview_selected_search_replacement();
    app.choose_search_hit(&hits[1], true);
    assert_eq!(app.search_state.chosen.len(), 2);
    assert!(app.search_state.plan.is_none());
    app.preview_selected_search_replacement();
    app.choose_search_hit(&hits[0], false);
    assert_eq!(app.search_state.chosen, vec![hits[1].clone()]);
    assert!(app.search_state.plan.is_none());
}

#[test]
fn stale_snapshot_with_same_offsets_is_not_reselected() {
    let (_ctx, mut app) = three();
    let hit = app.current_search_hits().unwrap()[0].clone();
    app.choose_search_hit(&hit, true);
    let mut buffer = app.project.open_source_writing_buffer(&app.active_file).unwrap();
    let original = buffer.source().to_owned();
    buffer.replace_source(original.replace("保留", "临时"));
    buffer.replace_source(original);
    app.manuscript.restore_writing_buffers(&[buffer]);
    let hits = app.current_search_hits().unwrap();
    app.reconcile_search_review(&hits);
    assert!(app.search_state.chosen.is_empty());
    app.choose_search_hit(&hit, true);
    assert!(app.search_state.chosen.is_empty());
    assert!(app.search_state.error.is_some());
}

#[test]
fn protected_hits_are_visible_but_not_implicitly_selected_or_silently_skipped() {
    let (_ctx, mut app) = three();
    app.project.set_text(&app.active_file.clone(),
        "event needle\n  needle\n  -> END\n".into()).unwrap();
    app.search_state.source = true;
    let hits = app.current_search_hits().unwrap();
    assert_eq!(hits.len(), 2);
    assert!(!hits[0].replaceable);
    app.choose_search_hit(&hits[0], true);
    assert!(app.search_state.chosen.is_empty());
    app.choose_all_search_hits();
    assert_eq!(app.search_state.chosen, vec![hits[1].clone()]);
    app.preview_selected_search_replacement();
    assert_eq!(app.search_state.plan.as_ref().unwrap().hits.len(), 1);
    app.preview_search_replacement();
    assert!(app.search_state.plan.is_none(), "legacy all preserves protected-token refusal");
    assert!(app.search_state.error.is_some());
}

#[test]
fn source_view_and_return_preserve_choices_preview_and_current_result() {
    let (ctx, mut app) = three();
    frame(&ctx, &mut app, vec![]);
    let hits = app.current_search_hits().unwrap();
    app.choose_search_hit(&hits[1], true);
    app.preview_selected_search_replacement();
    let plan = app.search_state.plan.clone();
    app.show_search_source(&ctx, &hits[1]);
    assert!(app.search_state.source_view);
    frame(&ctx, &mut app, vec![]);
    app.return_to_search_review(&ctx);
    frame(&ctx, &mut app, vec![]);
    assert!(!app.search_state.source_view);
    assert_eq!(app.search_state.selected, 1);
    assert_eq!(app.search_state.chosen, vec![hits[1].clone()]);
    assert_eq!(app.search_state.plan, plan);
    assert_eq!(ctx.memory(|m| m.focused()), Some(review::query_id()));
}

#[test]
fn current_draft_selected_replace_preserves_project_then_undo_redo_is_one_step() {
    let (ctx, mut app) = three();
    let path = app.active_file.clone();
    let mut buffer = app.project.open_source_writing_buffer(&path).unwrap();
    buffer.replace_source("event start\n  needle needle fresh\n  -> END\n".into());
    app.manuscript.restore_writing_buffers(&[buffer.clone()]);
    app.open_search(&ctx, false, true);
    app.project_query = "needle".into();
    let baseline = app.project.content_baseline();
    let hits = app.current_search_hits().unwrap();
    app.choose_search_hit(&hits[1], true);
    app.preview_selected_search_replacement();
    app.apply_search_replacement();
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.manuscript.writing_buffers()[0].source().contains("needle needle revised fresh"));
    app.edit_undo(false);
    assert_eq!(app.manuscript.writing_buffers()[0].source(), buffer.source());
    app.edit_undo(true);
    assert!(app.manuscript.writing_buffers()[0].source().contains("needle revised fresh"));
}

#[test]
fn cross_file_partial_transaction_and_undo_restore_each_unique_draft() {
    let (ctx, mut app) = three();
    let a = app.active_file.clone();
    let b = app.project.add_file(std::path::Path::new("second.wl")).unwrap();
    app.project.set_text(&b, "event second\n  needle needle\n".into()).unwrap();
    let mut draft = app.project.open_source_writing_buffer(&b).unwrap();
    draft.replace_source("event second\n  needle needle 草稿原字节\n".into());
    app.manuscript.restore_writing_buffers(&[draft.clone()]);
    app.open_search(&ctx, true, true);
    app.project_query = "needle".into();
    let hits = app.current_search_hits().unwrap();
    let selected: Vec<_> = [&a, &b].into_iter()
        .map(|path| hits.iter().find(|hit| hit.path == *path).unwrap().clone()).collect();
    for hit in &selected { app.choose_search_hit(hit, true); }
    app.preview_selected_search_replacement();
    assert_eq!(app.search_state.plan.as_ref().unwrap().changes.len(), 2);
    let baseline = app.project.content_baseline();
    app.apply_search_replacement();
    assert_eq!(app.search_state.applied_count, Some(2));
    assert!(app.manuscript.writing_buffers().is_empty());
    assert!(app.project.document(&b).unwrap().contains("needle revised needle 草稿原字节"));
    app.edit_undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.manuscript.writing_buffers().len(), 1);
    assert_eq!(app.manuscript.writing_buffers()[0].source(), draft.source());
    app.edit_undo(true);
    assert!(app.manuscript.writing_buffers().is_empty());
    assert!(app.project.document(&a).unwrap().contains("保留证词 needle\n"));
}

#[test]
fn every_hit_beyond_one_thousand_is_reachable_by_page_and_navigation() {
    let (ctx, mut app) = three();
    let source = format!("event start\n{}", "  needle\n".repeat(1001));
    app.project.set_text(&app.active_file.clone(), source).unwrap();
    let hits = app.current_search_hits().unwrap();
    app.reconcile_search_review(&hits);
    app.set_search_result_page(25, hits.len());
    assert_eq!(app.search_state.selected, 1000);
    assert_eq!(app.search_result_page(), 25);
    app.navigate_search(&ctx, false);
    assert_eq!(app.search_state.located.as_ref(), Some(&hits[1000]));
    assert_eq!(app.search_result_page(), 25);
    app.choose_search_hit(&hits[1000], true);
    app.preview_selected_search_replacement();
    assert_eq!(app.search_state.plan.as_ref().unwrap().hits, vec![hits[1000].clone()]);
    app.navigate_search(&ctx, false);
    assert_eq!(app.search_state.selected, 0);
    assert_eq!(app.search_result_page(), 0);
}

#[test]
fn captured_selection_uses_only_its_exact_matches_and_invalidates_after_source_edit() {
    let (ctx, mut app) = three();
    let source = app.project.document(&app.active_file).unwrap().to_owned();
    let start = source.find("needle").unwrap();
    app.search_state.current.as_mut().unwrap().range = start..start + "needle needle".len();
    app.search_state.scope = Scope::Selection;
    let hits = app.current_search_hits().unwrap();
    assert_eq!(hits.len(), 2);
    app.choose_search_hit(&hits[1], true);
    app.preview_selected_search_replacement();
    app.apply_search_replacement();
    assert!(app.project.document(&app.active_file).unwrap().contains("  needle needle revised\n"));
    assert!(app.current_search_hits().is_err());
    frame(&ctx, &mut app, vec![]);
    assert!(app.search_state.plan.is_none());
}
