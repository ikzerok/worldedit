//! Explicit body Apply and discard must cooperate with C and A1 history.
use super::history_cases::{
    apply_link, apply_locale, assert_buffers, dirty_buffers, locale_fixture,
};
use super::*;
use std::{collections::BTreeMap, path::Path};

// Capture every real file in this isolated unit-test workspace. Undo/redo and
// explicit discard must not change or create any on-disk file, including locales.
pub(super) fn disk_files(root: &Path) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn visit(root: &Path, dir: &Path, files: &mut BTreeMap<std::path::PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

fn exercise_body_apply(source_mode: bool) {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    let path = app.project.entry.clone();
    let sidecar = root.join(".world/localization/zh-Hant.json");
    let disk_before = disk_files(&root);
    let before_locale = app.project.sources();
    let applied_original = app.project.document(&path).unwrap().to_owned();

    apply_locale(&ctx, &mut app);
    let translation = app
        .project
        .authoring_document(&sidecar)
        .unwrap()
        .bytes()
        .to_vec();
    let original_drafts = dirty_buffers(&ctx, &mut app); // D0; includes a second file.
    apply_link(&ctx, &mut app, false); // Real C preview/apply; D1.
    let linked_drafts = app.manuscript.writing_buffers();
    let linked_source = linked_drafts
        .iter()
        .find(|buffer| buffer.path() == path.as_path())
        .unwrap()
        .source()
        .to_owned();
    assert_ne!(linked_source, applied_original);
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.search_state.undo.len(), 1);
    assert_buffers(&app, &linked_drafts);

    // Exercise the actual visible button and its host, not a forged Project edge.
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
    assert!(!app
        .manuscript
        .writing_buffers
        .get(&path)
        .is_some_and(|buffer| buffer.is_changed()));
    assert_eq!(disk_files(&root), disk_before);

    app.edit_undo(false); // Undo ordinary body/source Apply, recovering full D1.
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.project.document(&path).unwrap(), applied_original);
    // assert_buffers verifies source, generation, changed flag, current baseline,
    // and public core rebase_unchanged_source, proving the preserved original.
    assert_buffers(&app, &linked_drafts);

    app.edit_undo(false); // Undo C reference, recovering full D0.
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.search_state.undo.len(), 0);
    assert_eq!(app.search_state.redo.len(), 1);
    assert_buffers(&app, &original_drafts);

    app.edit_undo(false); // Earlier A1 must remain reachable.
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert!(app.history.is_empty());
    assert_eq!(app.project.sources(), before_locale);
    assert!(app
        .project
        .authoring_document(&sidecar)
        .unwrap()
        .is_deleted());
    assert_buffers(&app, &original_drafts);

    app.edit_undo(true); // Redo A1.
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_buffers(&app, &original_drafts);
    assert_eq!(
        app.project.authoring_document(&sidecar).unwrap().bytes(),
        translation
    );
    assert!(!app
        .project
        .authoring_document(&sidecar)
        .unwrap()
        .is_deleted());

    app.edit_undo(true); // Redo C reference.
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_buffers(&app, &linked_drafts);

    app.edit_undo(true); // Redo ordinary body/source Apply.
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), 2);
    assert!(app.redo.is_empty());
    assert!(app.search_state.redo.is_empty());
    assert_eq!(app.project.document(&path).unwrap(), linked_source);
    assert!(!app
        .manuscript
        .writing_buffers
        .get(&path)
        .is_some_and(|buffer| buffer.is_changed()));
    let other: Vec<_> = linked_drafts
        .into_iter()
        .filter(|buffer| buffer.path() != path.as_path())
        .collect();
    assert_buffers(&app, &other);
    assert_eq!(disk_files(&root), disk_before);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn body_apply_after_reference_restores_full_drafts_and_reaches_earlier_locale_history() {
    exercise_body_apply(false);
}

#[test]
fn source_apply_after_reference_restores_full_drafts_and_reaches_earlier_locale_history() {
    exercise_body_apply(true);
}

#[test]
fn confirmed_body_discard_ends_only_its_draft_history_and_unblocks_earlier_locale() {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    let path = app.project.entry.clone();
    let sidecar = root.join(".world/localization/zh-Hant.json");
    let disk_before = disk_files(&root);
    let before_locale = app.project.sources();
    apply_locale(&ctx, &mut app);
    dirty_buffers(&ctx, &mut app);
    apply_link(&ctx, &mut app, false);
    let linked = app.manuscript.writing_buffers();
    let other: Vec<_> = linked
        .iter()
        .filter(|buffer| buffer.path() != path.as_path())
        .cloned()
        .collect();
    let project_baseline = app.project.content_baseline();
    let project_counts = (app.history.len(), app.redo.len());
    let draft_counts = (app.search_state.undo.len(), app.search_state.redo.len());
    app.manuscript.reader_open = false;

    // First request and real cancellation must preserve bytes, generations,
    // original guards, both histories, and the applied project.
    click(&ctx, &mut app, "丢弃此文件草稿");
    assert_buffers(&app, &linked);
    click(&ctx, &mut app, "取消丢弃");
    assert_buffers(&app, &linked);
    assert_eq!((app.history.len(), app.redo.len()), project_counts);
    assert_eq!(
        (app.search_state.undo.len(), app.search_state.redo.len()),
        draft_counts
    );
    assert_eq!(app.project.content_baseline(), project_baseline);

    // This is the real explicit confirmation button. No direct buffer removal.
    click(&ctx, &mut app, "丢弃此文件草稿");
    click(&ctx, &mut app, "确认丢弃正文草稿");
    assert!(!app
        .manuscript
        .writing_buffers
        .get(&path)
        .is_some_and(|buffer| buffer.is_changed()));
    assert_buffers(&app, &other);
    assert_eq!((app.history.len(), app.redo.len()), project_counts);
    assert_eq!(app.project.content_baseline(), project_baseline);
    app.edit_undo(false); // No stale C entry may intercept this A1 Undo.
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert!(
        app.search_state.undo.is_empty(),
        "discarded file's draft operation must end explicitly"
    );
    assert!(app.search_state.redo.is_empty());
    assert!(app.history.is_empty());
    assert_eq!(app.project.sources(), before_locale);
    assert!(app
        .project
        .authoring_document(&sidecar)
        .unwrap()
        .is_deleted());
    assert_buffers(&app, &other);
    app.edit_undo(true);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), 1);
    assert!(!app
        .project
        .authoring_document(&sidecar)
        .unwrap()
        .is_deleted());
    assert_buffers(&app, &other);
    assert_eq!(disk_files(&root), disk_before);
    fs::remove_dir_all(root).unwrap();
}
