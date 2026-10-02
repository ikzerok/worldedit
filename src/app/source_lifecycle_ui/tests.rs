use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use worldline_core::project::Project;

mod performance;

fn app() -> (egui::Context, WorldeditApp, PathBuf) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let ctx = egui::Context::default();
    let cc = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&cc, None);
    let root = std::env::temp_dir().join(format!(
        "source-move-ui-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    app.project = Project::new(&root);
    let entry = app.project.entry.clone();
    app.project.documents.retain(|path, _| path == &entry);
    app.project
        .set_text(&entry, "event start\n  -> chapter\n".into())
        .unwrap();
    let source = app.project.add_file(Path::new("chapters/旧章.wl")).unwrap();
    app.project
        .set_text(&source, "event chapter\n  正文。\n  -> END\n".into())
        .unwrap();
    app.project.save().unwrap();
    app.active_file = source.clone();
    app.reset_views();
    app.recompile();
    (ctx, app, source)
}

#[test]
fn source_move_plan_is_readonly_and_apply_has_one_undo_then_redo() {
    let (_, mut app, source) = app();
    let root = app.project.root.clone();
    app.begin_source_move(source.clone());
    let mut form = app.source_move_form.take().unwrap();
    form.destination = "中文深层/新章.wl".into();
    let baseline = app.project.content_baseline();
    app.preview_source_move(&mut form);
    assert!(form.error.is_none(), "{:?}", form.error);
    assert!(form.plan.is_some());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.apply_source_move(&mut form), "{:?}", form.error);
    assert_eq!(app.active_file, root.join("中文深层/新章.wl"));
    assert_eq!(app.history.len(), 1);
    assert!(source.exists(), "应用不自动保存");
    app.undo(false);
    assert!(app.project.document(&source).is_ok());
    app.undo(true);
    assert!(app.project.document(&root.join("中文深层/新章.wl")).is_ok());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_move_unapplied_inputs_stale_plan_and_entry_refuse_without_loss() {
    let (_, mut app, source) = app();
    let root = app.project.root.clone();
    app.new_file = Some("another.wl".into());
    app.begin_source_move(source.clone());
    assert!(app.source_move_form.is_none());
    assert_eq!(app.new_file.as_deref(), Some("another.wl"));
    app.new_file = None;
    app.begin_source_move(app.project.entry.clone());
    assert!(app.source_move_form.is_none());
    app.begin_source_move(source.clone());
    let mut form = app.source_move_form.take().unwrap();
    form.destination = "moved.wl".into();
    app.preview_source_move(&mut form);
    let current = app.project.document(&source).unwrap().to_owned() + "// 外部于计划的当前修改\n";
    app.project.set_text(&source, current.clone()).unwrap();
    let baseline = app.project.content_baseline();
    assert!(!app.apply_source_move(&mut form));
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&source), Ok(current.as_str()));
    assert_eq!(form.destination, "moved.wl");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_move_path_input_participates_in_exit_and_export_guards() {
    let (_, mut app, source) = app();
    let root = app.project.root.clone();
    app.begin_source_move(source.clone());
    assert!(!app.dirty_draft_names().contains(&"源码路径"));
    app.source_move_form.as_mut().unwrap().destination = "新的章.wl".into();
    assert!(app.dirty_draft_names().contains(&"源码路径"));
    assert!(app
        .unapplied_export_inputs()
        .iter()
        .any(|input| input.kind == "源码路径" && input.source.contains("新的章.wl")));
    let before = app.project.content_baseline();
    app.discard_authoring_drafts();
    assert!(app.source_move_form.is_none());
    assert_eq!(before, app.project.content_baseline());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn nested_native_source_path_becomes_portable_lifecycle_request() {
    let root = std::env::temp_dir().join("source-request-format-only");
    let form = SourceMoveForm {
        source: root.join("lore").join("part.wl"),
        root,
        destination: "drafts/new.wl".into(),
        plan: None,
        error: None,
    };
    assert_eq!(
        form.request(),
        SourceLifecycleRequest::Move {
            from: PathBuf::from("lore/part.wl"),
            to: PathBuf::from("drafts/new.wl"),
        }
    );
}
