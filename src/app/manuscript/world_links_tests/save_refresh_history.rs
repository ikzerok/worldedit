//! Save and native polling must preserve A1/C history and complete author input.
use super::history_cases::{
    apply_link, apply_locale_to, assert_buffers, dirty_buffers, locale_fixture,
};
use super::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use worldline_core::manuscript::WritingBuffer;

struct SavedDeletion {
    ctx: egui::Context,
    app: WorldeditApp,
    originals: Vec<WritingBuffer>,
    initial_files: BTreeMap<PathBuf, Vec<u8>>,
    applied_sources: BTreeMap<PathBuf, String>,
    translation: Vec<u8>,
}

impl Drop for SavedDeletion {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.app.project.root);
    }
}

fn disk(app: &WorldeditApp) -> BTreeMap<PathBuf, Vec<u8>> {
    worldline_core::file_access::workspace_files(&app.project.root)
        .unwrap()
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}

fn native_poll(ctx: &egui::Context, app: &mut WorldeditApp) {
    app.last_refresh = Instant::now() - Duration::from_secs(2);
    app.tab = Tab::Edit;
    let mut native_frame = eframe::Frame::_new_kittest();
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1700.0, 1400.0))),
            ..Default::default()
        },
        |ctx| eframe::App::update(app, ctx, &mut native_frame),
    );
}

fn counts(app: &WorldeditApp) -> (usize, usize, usize, usize) {
    (
        app.history.len(),
        app.redo.len(),
        app.search_state.undo.len(),
        app.search_state.redo.len(),
    )
}

fn saved_deleted_locale() -> SavedDeletion {
    let (ctx, mut app) = locale_fixture();
    app.saved_location = true;
    native_poll(&ctx, &mut app);
    let initial_files = disk(&app);
    assert_eq!(initial_files.len(), 5);
    let initial_sources = app.project.sources();
    apply_locale_to(&ctx, &mut app, "fr");
    let originals = dirty_buffers(&ctx, &mut app);
    assert_eq!(
        originals
            .iter()
            .filter(|buffer| buffer.is_changed())
            .count(),
        2
    );
    apply_link(&ctx, &mut app, true);
    assert_eq!(counts(&app), (2, 0, 0, 0));
    let applied_sources = app.project.sources();
    let sidecar = app.project.root.join(".world/localization/fr.json");
    let translation = app
        .project
        .authoring_document(&sidecar)
        .unwrap()
        .bytes()
        .to_vec();
    assert!(app.save(), "{:?}", app.io_error);
    assert_eq!(disk(&app).len(), 6);
    native_poll(&ctx, &mut app);
    assert_eq!(counts(&app), (2, 0, 0, 0));
    for _ in 0..2 {
        app.edit_undo(false);
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        assert_buffers(&app, &originals);
    }
    assert_eq!(app.project.sources(), initial_sources);
    assert_eq!(counts(&app), (0, 2, 0, 0));
    assert!(app
        .project
        .authoring_document(&sidecar)
        .unwrap()
        .is_deleted());
    // Exercise the actual native Save host, not just a direct Project save.
    assert!(app.save(), "{:?}", app.io_error);
    assert_eq!(disk(&app), initial_files);
    assert!(!sidecar.exists());
    assert_buffers(&app, &originals);
    SavedDeletion {
        ctx,
        app,
        originals,
        initial_files,
        applied_sources,
        translation,
    }
}

#[test]
fn a1_c_undo_save_native_poll_keeps_full_drafts_and_redo_history() {
    let mut work = saved_deleted_locale();
    let sidecar = work.app.project.root.join(".world/localization/fr.json");
    let baseline = work.app.project.content_baseline();
    let node = work.app.history_state.current;
    let version = work.app.version;
    native_poll(&work.ctx, &mut work.app);
    assert_eq!(
        (
            counts(&work.app),
            work.app.history_state.current,
            work.app.version
        ),
        ((0, 2, 0, 0), node, version),
        "observing the just-saved deletion must not invalidate either history stack"
    );
    assert_eq!(work.app.project.content_baseline(), baseline);
    assert_eq!(disk(&work.app), work.initial_files);
    assert_buffers(&work.app, &work.originals);
    work.app.edit_undo(true);
    assert!(work.app.io_error.is_none(), "{:?}", work.app.io_error);
    assert_buffers(&work.app, &work.originals);
    assert!(work
        .app
        .project
        .authoring_document(&sidecar)
        .unwrap()
        .is_dirty());
    work.app.edit_undo(true);
    assert!(work.app.io_error.is_none(), "{:?}", work.app.io_error);
    assert_eq!(counts(&work.app), (2, 0, 0, 0));
    assert_eq!(work.app.project.sources(), work.applied_sources);
    let applied_baseline = work.app.project.content_baseline();
    for _ in 0..2 {
        work.app.edit_undo(true);
        assert_eq!(counts(&work.app), (2, 0, 0, 0));
        assert_eq!(work.app.project.content_baseline(), applied_baseline);
    }
    assert!(work.app.save(), "{:?}", work.app.io_error);
    assert_eq!(fs::read(&sidecar).unwrap(), work.translation);
    assert_eq!(
        Project::open(&work.app.project.root).unwrap().sources(),
        work.applied_sources
    );
}

#[test]
fn real_external_edit_after_saved_locale_deletion_still_invalidates_history_without_losing_input() {
    let mut work = saved_deleted_locale();
    let before = work.app.project.clone();
    let source = work.app.project.root.join("unrelated.wl");
    let external = "// 真正外部改稿，必须阻断旧快照\n";
    fs::write(&source, external).unwrap();
    let mut core_probe = work.app.project.clone();
    assert!(core_probe.refresh().unwrap().is_empty());
    let refreshed = core_probe.content_baseline();
    assert!(!core_probe.restore(before));
    assert_eq!(core_probe.content_baseline(), refreshed);
    let keep = work.app.manuscript.writing_buffers();
    native_poll(&work.ctx, &mut work.app);
    assert_eq!(counts(&work.app), (0, 0, 0, 0));
    assert_eq!(work.app.project.document(&source).unwrap(), external);
    assert_eq!(fs::read_to_string(&source).unwrap(), external);
    for expected in &keep {
        let actual = &work.app.manuscript.writing_buffers[expected.path()];
        assert_eq!(actual.source(), expected.source());
        assert_eq!(actual.generation(), expected.generation());
        assert_eq!(actual.baseline(), expected.baseline());
        assert_eq!(actual.is_changed(), expected.is_changed());
    }
}
