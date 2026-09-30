use super::*;
use std::time::{Duration, Instant, SystemTime};

fn poll_workspace(ctx: &egui::Context, app: &mut WorldeditApp) {
    app.last_refresh = Instant::now() - Duration::from_secs(2);
    let mut native_frame = eframe::Frame::_new_kittest();
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1700.0, 1400.0))),
            ..Default::default()
        },
        |ctx| eframe::App::update(app, ctx, &mut native_frame),
    );
}

fn saved_workspace() -> (egui::Context, WorldeditApp, String) {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    app.saved_location = true;
    app.tab = super::Tab::Edit;
    poll_workspace(&ctx, &mut app);
    let original = app.project.document(&app.active_file).unwrap().to_owned();
    (ctx, app, original)
}

fn edit_with_undo_and_redo(app: &mut WorldeditApp, original: &str) -> String {
    let path = app.active_file.clone();
    let first = format!("{original}\n// first local change\n");
    assert!(app.commit("first", |project| project.set_text(&path, first.clone())));
    let second = format!("{first}// second local change\n");
    assert!(app.commit("second", |project| project.set_text(&path, second.clone())));
    app.undo(false);
    assert_eq!((app.history.len(), app.redo.len()), (1, 1));
    first
}

fn form_is_current(app: &WorldeditApp) -> bool {
    app.entity_editor
        .as_ref()
        .unwrap()
        .guard
        .is_current(&app.project, app.version)
}

#[test]
fn self_save_poll_preserves_new_form_and_undo_redo() {
    let (ctx, mut app, original) = saved_workspace();
    let saved = edit_with_undo_and_redo(&mut app, &original);
    assert!(app.save());
    app.edit_entity(Some("a"));
    assert!(form_is_current(&app));
    let version = app.version;
    let baseline = app.project.content_baseline();
    poll_workspace(&ctx, &mut app);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(
        (
            app.version,
            app.history.len(),
            app.redo.len(),
            form_is_current(&app)
        ),
        (version, 1, 1, true),
        "本地保存的磁盘变化不是外部编辑，不能清历史或使保存后新开的表单陈旧"
    );
    app.entity_editor = None;
    app.undo(false);
    assert_eq!(app.project.document(&app.active_file).unwrap(), original);
    app.undo(true);
    assert_eq!(app.project.document(&app.active_file).unwrap(), saved);
}

#[test]
fn metadata_only_poll_preserves_dirty_draft_form_and_history() {
    let (ctx, mut app, original) = saved_workspace();
    let draft = edit_with_undo_and_redo(&mut app, &original);
    app.edit_entity(Some("a"));
    let version = app.version;
    std::fs::File::options()
        .write(true)
        .open(&app.active_file)
        .unwrap()
        .set_modified(SystemTime::now() + Duration::from_secs(5))
        .unwrap();
    poll_workspace(&ctx, &mut app);
    assert_eq!(app.project.document(&app.active_file).unwrap(), draft);
    assert_eq!(std::fs::read_to_string(&app.active_file).unwrap(), original);
    assert_eq!(
        (
            app.version,
            app.history.len(),
            app.redo.len(),
            form_is_current(&app)
        ),
        (version, 1, 1, true)
    );
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
}

#[test]
fn external_content_poll_refreshes_clean_buffer_and_invalidates_form_history() {
    let (ctx, mut app, original) = saved_workspace();
    edit_with_undo_and_redo(&mut app, &original);
    assert!(app.save());
    app.edit_entity(Some("a"));
    let external = format!("{original}\n// genuine external edit\n");
    std::fs::write(&app.active_file, &external).unwrap();
    poll_workspace(&ctx, &mut app);
    assert_eq!(app.project.document(&app.active_file).unwrap(), external);
    assert!(!app.project.is_dirty());
    assert!(!form_is_current(&app));
    assert_eq!((app.history.len(), app.redo.len()), (0, 0));
}

#[test]
fn external_content_poll_keeps_conflicting_dirty_buffer_and_rejects_old_form() {
    let (ctx, mut app, original) = saved_workspace();
    let draft = edit_with_undo_and_redo(&mut app, &original);
    app.edit_entity(Some("a"));
    let external = format!("{original}\n// conflicting external edit\n");
    std::fs::write(&app.active_file, &external).unwrap();
    poll_workspace(&ctx, &mut app);
    assert_eq!(app.project.document(&app.active_file).unwrap(), draft);
    assert_eq!(std::fs::read_to_string(&app.active_file).unwrap(), external);
    assert!(app.project.is_dirty());
    assert!(!form_is_current(&app));
    assert_eq!((app.history.len(), app.redo.len()), (0, 0));
    assert!(app
        .io_error
        .as_deref()
        .is_some_and(|error| error.contains("外部修改与未保存内容冲突")));
}

#[test]
fn recovery_conflict_poll_invalidates_form_even_when_buffer_baseline_is_unchanged() {
    let (ctx, mut app, original) = saved_workspace();
    edit_with_undo_and_redo(&mut app, &original);
    app.edit_entity(Some("a"));
    let baseline = app.project.content_baseline();
    let transaction = app
        .project
        .root
        .join(".world/.transactions/recovery-conflict-test");
    std::fs::create_dir_all(&transaction).unwrap();
    // 日志期待文件不存在，但磁盘上的第三方内容仍在，core 必须保留并报告恢复冲突。
    std::fs::write(transaction.join("journal.json"),
        br#"{"version":1,"status":"applying","files":[{"path":"world.wl","before":null,"after":null,"payload":null}]}"#,
    ).unwrap();
    // 事务目录不属于可见作品文件；用纯 mtime 变化触发现有的磁盘轮询。
    std::fs::File::options()
        .write(true)
        .open(&app.active_file)
        .unwrap()
        .set_modified(SystemTime::now() + Duration::from_secs(5))
        .unwrap();
    assert!(app.project.recovery_conflicts().is_empty());
    poll_workspace(&ctx, &mut app);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.recovery_conflicts(), &[app.active_file.clone()]);
    assert!(
        !form_is_current(&app),
        "恢复冲突改变写入安全边界，旧表单必须失效"
    );
    assert_eq!((app.history.len(), app.redo.len()), (0, 0));
    assert!(!app.save());
    assert!(app
        .io_error
        .as_deref()
        .is_some_and(|error| error.contains("保存事务")));
    assert_eq!(std::fs::read_to_string(&app.active_file).unwrap(), original);
    assert!(transaction.join("journal.json").is_file());
}
