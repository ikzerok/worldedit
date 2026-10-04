use super::*;

#[test]
fn worker_rehydration_at_same_root_preserves_request_preview_and_digest() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    load(&ctx, &mut app, CSV);
    map(&mut app);
    preview(&ctx, &mut app);
    let request = app.catalog_import.request.as_ref().unwrap();
    let state = app.project.snapshot_state().unwrap();
    let files = app
        .project
        .snapshot_files_limited(10_000, 128 * 1024 * 1024)
        .unwrap();
    let entry = app.project.entry.strip_prefix(&app.project.root).unwrap();
    let restored =
        Project::from_snapshot_with_state(&app.project.root, entry, &files, &state).unwrap();
    assert_eq!(restored.content_baseline(), app.project.content_baseline());
    let plan = restored.preview_catalog_import(request).unwrap();
    assert_eq!(plan, *app.catalog_import.plan.as_ref().unwrap());
}

#[test]
fn empty_or_invalid_utf8_selection_does_not_silently_succeed() {
    let (ctx, mut app) = app();
    app.catalog_import
        .offer_file("empty.csv".into(), Vec::new(), &ctx)
        .unwrap();
    wait(&ctx, &mut app);
    assert!(app.catalog_import.error.is_some());
    assert!(app.catalog_import.table.is_none());
    assert!(app.catalog_import.has_unsubmitted_work());
    assert!(app
        .catalog_import
        .offer_file("bad.csv".into(), vec![0xff], &ctx)
        .is_err());
    assert_eq!(app.catalog_import.source_name, "empty.csv");
    assert!(app.history.is_empty());
}
