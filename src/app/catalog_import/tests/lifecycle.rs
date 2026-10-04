use super::*;

#[test]
fn preview_apply_undo_redo_save_reopen_and_reimport_are_one_batch() {
    let (ctx, mut app) = app();
    let original = app.project.sources();
    load(&ctx, &mut app, CSV);
    map(&mut app);
    preview(&ctx, &mut app);
    let plan = app.catalog_import.plan.as_ref().unwrap();
    assert!(plan.can_apply, "{plan:?}");
    assert_eq!(plan.rows.len(), 2);
    assert_eq!(plan.ignored_columns.len(), 1);
    assert_ne!(
        plan.runtime_fingerprint_after,
        Some(plan.runtime_fingerprint_before)
    );
    assert_eq!(app.project.sources(), original);
    apply(&mut app);
    assert!(
        app.catalog_import.submitted,
        "{:?}",
        app.catalog_import.error
    );
    assert_eq!(app.history.len(), 1);
    let imported = app.project.sources();
    assert!(imported
        .values()
        .any(|s| s.contains("// 不在导入范围的备注") && s.contains("property voice = \"轻声\"")));
    app.undo(false);
    assert_eq!(app.project.sources(), original);
    app.undo(true);
    assert_eq!(app.project.sources(), imported);
    app.project.save().unwrap();
    let mut reopened = Project::open(&app.project.root).unwrap();
    assert!(!reopened.compile().has_errors());
    assert_eq!(reopened.sources(), imported);
    preview(&ctx, &mut app);
    let plan = app.catalog_import.plan.as_ref().unwrap();
    assert!(
        plan.rows.iter().all(|r| r.operation == "unchanged"),
        "{plan:?}"
    );
    assert!(plan.changed_files.is_empty());
    assert_eq!(app.history.len(), 1);
    assert!(!app.project.is_dirty());
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn mapping_target_and_current_baseline_invalidate_acknowledgement() {
    let (ctx, mut app) = app();
    load(&ctx, &mut app, CSV);
    map(&mut app);
    preview(&ctx, &mut app);
    app.catalog_import.acknowledged = true;
    app.catalog_import.columns[2].as_mut().unwrap().blank = Blank::Keep;
    app.catalog_import.invalidate();
    assert!(app.catalog_import.stale);
    assert!(!app.catalog_import.acknowledged);
    let before = app.project.sources();
    apply(&mut app);
    assert_eq!(app.project.sources(), before);
    preview(&ctx, &mut app);
    app.catalog_import.acknowledged = true;
    app.catalog_import.destination = PathBuf::from("not-active.wl");
    app.catalog_import.invalidate();
    assert!(!app.catalog_import.acknowledged);
    app.catalog_import.destination = app
        .project
        .entry
        .strip_prefix(&app.project.root)
        .unwrap()
        .to_path_buf();
    preview(&ctx, &mut app);
    app.project
        .set_text(
            &app.active_file.clone(),
            format!(
                "{}\n# 外部于预览的内存编辑\n",
                app.project.document(&app.active_file).unwrap()
            ),
        )
        .unwrap();
    let before = app.project.sources();
    apply(&mut app);
    assert!(app.catalog_import.stale);
    assert_eq!(app.project.sources(), before);
}

#[test]
fn invalid_batch_and_unmapped_column_have_no_partial_changes() {
    let (ctx, mut app) = app();
    let before = app.project.sources();
    load(&ctx, &mut app, &CSV.replace(",35,", ",NaN,"));
    map(&mut app);
    preview(&ctx, &mut app);
    assert!(app.catalog_import.plan.as_ref().unwrap().error_count > 0);
    apply(&mut app);
    assert_eq!(app.project.sources(), before);
    assert!(app.history.is_empty());
    app.catalog_import.columns[4] = None;
    app.catalog_import.invalidate();
    preview(&ctx, &mut app);
    assert!(!app.catalog_import.plan.as_ref().unwrap().can_apply);
    assert_eq!(app.project.sources(), before);
}

#[test]
fn input_is_preserved_for_exit_export_play_and_file_replacement_cancel() {
    let (ctx, mut app) = app();
    load(&ctx, &mut app, CSV);
    map(&mut app);
    let signature = app.catalog_import.input_signature();
    // Native/browser picker cancellation produces no file event: the state is unchanged.
    frame(&ctx, &mut app, egui::vec2(760.0, 620.0), vec![]);
    assert_eq!(app.catalog_import.input_signature(), signature);
    app.catalog_import
        .offer_file(
            "replacement.csv".into(),
            CSV.replace("向导", "新向导").into_bytes(),
            &ctx,
        )
        .unwrap();
    click(&ctx, &mut app, "保留当前快照");
    assert_eq!(app.catalog_import.input_signature(), signature);
    assert!(app.dirty_draft_names().contains(&"世界资料导入"));
    assert!(app
        .unapplied_export_inputs()
        .iter()
        .any(|i| i.kind == "世界资料导入"));
    assert!(app
        .unapplied_play_inputs()
        .iter()
        .any(|i| i.kind == "世界资料导入"));
    app.request_action(super::super::super::Pending::Close, &ctx);
    assert!(app.draft_action.is_some());
    assert!(!app.allow_close);
    assert_eq!(app.catalog_import.input_signature(), signature);
    click(&ctx, &mut app, "丢弃导入输入…");
    click(&ctx, &mut app, "取消丢弃");
    assert_eq!(app.catalog_import.input_signature(), signature);
}

#[test]
fn unsaved_author_input_blocks_import_without_clearing_it() {
    let (ctx, mut app) = app();
    app.select_character("traveler");
    app.character_editor.as_mut().unwrap().draft.display = "未应用的手工输入".into();
    load(&ctx, &mut app, CSV);
    map(&mut app);
    let before = app.project.sources();
    preview(&ctx, &mut app);
    assert!(app.catalog_import.plan.is_none());
    assert!(app
        .catalog_import
        .error
        .as_ref()
        .unwrap()
        .contains("人物资料"));
    assert_eq!(
        app.character_editor.as_ref().unwrap().draft.display,
        "未应用的手工输入"
    );
    assert_eq!(app.project.sources(), before);
}

#[test]
fn cancelled_and_superseded_background_results_do_not_replace_current_input() {
    let (ctx, mut app) = app();
    load(&ctx, &mut app, CSV);
    map(&mut app);
    let mut state = std::mem::take(&mut app.catalog_import);
    state.preview(&app, &ctx);
    state.invalidate();
    app.catalog_import = state;
    wait(&ctx, &mut app);
    assert!(app.catalog_import.plan.is_none());
    assert!(app.catalog_import.status.as_ref().unwrap().contains("丢弃"));
    assert!(app.history.is_empty());
}
