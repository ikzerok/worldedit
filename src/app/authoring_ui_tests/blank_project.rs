//! 空目录启动、实际保存与重开后的试玩；离屏 egui 不替代原生交互验收。
use super::*;
use std::path::PathBuf;

fn blank_editor() -> (egui::Context, WorldeditApp, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "worldedit-blank-{}-{}",
        std::process::id(),
        NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let app = WorldeditApp::start_native(&creation, Some(root.clone()));
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert!(app.saved_location);
    (ctx, app, root)
}

fn assert_only_entry(root: &std::path::Path, expected: &str) {
    let files: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(files, [std::ffi::OsString::from("world.wl")]);
    assert_eq!(
        std::fs::read_to_string(root.join("world.wl")).unwrap(),
        expected
    );
}

#[test]
fn empty_directory_start_save_reopen_and_play_keeps_a_blank_work() {
    let (ctx, mut app, root) = blank_editor();
    assert_eq!(app.project.documents.len(), 1);
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities
        .is_empty());
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .symbols
        .characters
        .is_empty());
    assert_only_entry(&root, "event start\n  -> END\n");
    assert!(app.save(), "{:?}", app.io_error);
    drop(app);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut reopened = WorldeditApp::new(&creation, Some(root.clone()));
    reopened.tab = Tab::Play;
    reopened.start_play();
    let _ = frame(&ctx, &mut reopened, Vec::new(), 20);
    let play = reopened.play.as_ref().unwrap();
    assert!(play.ended);
    assert!(play.error.is_none());
    assert!(play.transcript.is_empty());
    assert_only_entry(&root, "event start\n  -> END\n");
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn authored_text_survives_save_reopen_and_normal_play_from_blank() {
    let (ctx, mut app, root) = blank_editor();
    let source = "event start\n  author text.\n  -> END\n";
    app.project
        .set_text(&app.active_file.clone(), source.into())
        .unwrap();
    app.recompile();
    assert!(app.save(), "{:?}", app.io_error);
    assert_only_entry(&root, source);
    drop(app);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut reopened = WorldeditApp::new(&creation, Some(root.clone()));
    reopened.tab = Tab::Play;
    reopened.start_play();
    let _ = frame(&ctx, &mut reopened, Vec::new(), 20);
    let play = reopened.play.as_ref().unwrap();
    assert!(play.ended);
    assert!(play.error.is_none());
    assert_eq!(play.transcript, "author text.");
    assert_only_entry(&root, source);
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn new_event_form_keeps_the_existing_creation_path_on_blank_work() {
    let (_, mut app, root) = blank_editor();
    app.new_event(None);
    let editor = app.event_editor.take().unwrap();
    assert!(editor.original.is_none());
    app.project
        .write_event(&editor.path, None, &editor.draft)
        .unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .symbols
        .events
        .contains_key(&editor.draft.id));
    assert!(app.save(), "{:?}", app.io_error);
    let mut reopened = Project::open(&root).unwrap();
    assert!(reopened
        .compile()
        .analysis
        .symbols
        .events
        .contains_key(&editor.draft.id));
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
