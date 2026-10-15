//! 真实 App 更新和离屏按钮回归；原生窗口/系统选择器仍须桌面旅程验证。
mod support;
use super::*;
use crate::app::Pending;
use egui::{Event, Key, Modifiers};
use std::collections::BTreeMap;
use support::*;

#[test]
fn no_path_shows_welcome_and_exit_never_creates_a_workspace() {
    let root = crate::app::init::draft_root();
    let existed = root.exists();
    let before = inventory(&root);
    let (ctx, mut app) = app(None);
    for _ in 0..3 {
        let output = frame(&ctx, &mut app, vec![]);
        assert!(!has_close(&output));
    }
    let output = frame(&ctx, &mut app, vec![key(Key::S, Modifiers::COMMAND)]);
    let text = rendered(&output);
    for label in [
        "欢迎使用 worldedit",
        "新建作品",
        "打开工程",
        "外观设置",
        "退出",
    ] {
        assert!(text.contains(label), "{text}");
    }
    assert!(!text.contains("保存全部"));
    assert!(app.native_startup.welcome);
    assert!(!app.saved_location && !app.allow_close);
    assert!(app.directory.is_none() && app.pending.is_none());
    let mut storage = Storage::default();
    eframe::App::save(&mut app, &mut storage);
    assert!(recent_project(Some(&storage)).is_none());
    assert!(!storage
        .0
        .values()
        .any(|text| text.contains("worldedit-draft-")));
    click(&ctx, &mut app, "退出");
    assert!(app.allow_close);
    assert!(app.pending.is_none() && app.draft_action.is_none());
    assert_eq!(root.exists(), existed);
    assert_eq!(inventory(&root), before);
}

#[test]
fn explicit_missing_empty_or_unreadable_source_stays_open_without_writes() {
    let workspace = Workspace::new();
    let empty = workspace.empty("空工作区");
    let invalid = workspace.0.join("invalid.wl");
    std::fs::write(&invalid, [0xff, 0xfe, 0x00]).unwrap();
    let before = inventory(&workspace.0);
    for path in [workspace.0.join("missing"), empty, invalid] {
        let (ctx, mut app) = app(Some(path));
        assert!(app.native_startup.welcome && !app.saved_location);
        assert!(app.io_error.is_some());
        for _ in 0..3 {
            let output = frame(&ctx, &mut app, vec![]);
            assert!(!has_close(&output));
        }
        let output = frame(&ctx, &mut app, vec![]);
        assert!(rendered(&output).contains("无法完成操作"));
        click(&ctx, &mut app, "退出");
        assert!(app.allow_close);
    }
    assert_eq!(inventory(&workspace.0), before);
}

#[test]
fn welcome_cancel_repeated_new_and_open_keep_the_app_and_disk_unchanged() {
    let workspace = Workspace::new();
    let (ctx, mut app) = app(None);
    for label in ["新建作品", "打开工程", "新建作品", "打开工程"] {
        click(&ctx, &mut app, label);
        enter_path(&ctx, &mut app, &workspace.0.join("尚未创建"));
        click(&ctx, &mut app, "取消");
        assert!(app.native_startup.welcome);
        assert!(app.native_startup.dialog.is_none());
        assert!(!app.allow_close && !app.saved_location);
        assert!(app.pending.is_none());
    }
    click(&ctx, &mut app, "新建作品");
    frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.native_startup.dialog.is_none());
    assert!(inventory(&workspace.0).is_empty());
}

#[test]
fn create_button_writes_only_blank_work_then_save_and_recent_reopen_keep_it() {
    let workspace = Workspace::new();
    let root = workspace.0.join("第一部作品");
    let (ctx, mut app) = app(None);
    click(&ctx, &mut app, "新建作品");
    enter_path(&ctx, &mut app, &root);
    assert!(!root.exists());
    click(&ctx, &mut app, "创建作品");
    assert!(!app.native_startup.welcome && app.saved_location);
    assert!(app.native_startup.dialog.is_none());
    assert_eq!(app.project.root, root);
    assert_eq!(
        inventory(&root),
        BTreeMap::from([(
            PathBuf::from("world.wl"),
            b"event start\n  -> END\n".to_vec()
        )])
    );
    let entry = app.active_file.clone();
    let source = "event start\n  第一句正文。\n  -> END\n";
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    assert!(app.save());
    app.capture_personal_view(&ctx);
    let mut storage = Storage::default();
    eframe::App::save(&mut app, &mut storage);
    let mut creation = eframe::CreationContext::_new_kittest(ctx.clone());
    creation.storage = Some(&storage);
    let reopened = WorldeditApp::start_native(&creation, None);
    assert!(reopened.saved_location && !reopened.native_startup.welcome);
    assert_eq!(reopened.project.root, root);
    assert_eq!(reopened.project.document(&entry).unwrap(), source);
    assert_eq!(std::fs::read_to_string(entry).unwrap(), source);
}

#[test]
fn invalid_open_can_be_corrected_in_place_and_cli_path_wins_over_recent() {
    let workspace = Workspace::new();
    let first = workspace.project("first");
    let second = workspace.project("second");
    let before = inventory(&workspace.0);
    let (ctx, mut app) = app(None);
    click(&ctx, &mut app, "打开工程");
    enter_path(&ctx, &mut app, &workspace.0.join("missing"));
    click(&ctx, &mut app, "打开工程");
    assert!(app.native_startup.welcome && app.native_startup.dialog.is_some());
    assert!(app.io_error.is_some() && !app.allow_close);
    enter_path(&ctx, &mut app, &first);
    click(&ctx, &mut app, "打开工程");
    assert!(!app.native_startup.welcome && app.saved_location);
    assert_eq!(app.project.root, first);
    app.capture_personal_view(&ctx);
    let mut storage = Storage::default();
    app.personal.save(&mut storage);
    let mut creation = eframe::CreationContext::_new_kittest(ctx.clone());
    creation.storage = Some(&storage);
    let explicit = WorldeditApp::start_native(&creation, Some(second.clone()));
    assert_eq!(explicit.project.root, second);
    let missing = WorldeditApp::start_native(&creation, Some(workspace.0.join("missing")));
    assert!(missing.native_startup.welcome && !missing.saved_location);
    assert!(missing.io_error.is_some() && !missing.allow_close);
    assert_eq!(inventory(&workspace.0), before);
}

#[test]
fn failed_new_keeps_path_and_retries_without_touching_nonempty_folder() {
    let workspace = Workspace::new();
    let occupied = workspace.project("occupied");
    let before = inventory(&occupied);
    let blocked = workspace.0.join("file-instead-of-folder");
    std::fs::write(&blocked, b"preserve").unwrap();
    let (ctx, mut app) = app(None);
    click(&ctx, &mut app, "新建作品");
    for path in [&occupied, &blocked] {
        enter_path(&ctx, &mut app, path);
        click(&ctx, &mut app, "创建作品");
        assert!(app.native_startup.welcome && app.native_startup.dialog.is_some());
        assert!(app.io_error.is_some() && !app.allow_close);
        assert_eq!(inventory(&occupied), before);
        assert_eq!(std::fs::read(&blocked).unwrap(), b"preserve");
    }
    let empty = workspace.empty("retry");
    enter_path(&ctx, &mut app, &empty);
    click(&ctx, &mut app, "创建作品");
    assert!(app.saved_location && !app.native_startup.welcome);
    assert_eq!(app.project.root, empty);
    assert!(app.io_error.is_none());
}

#[test]
fn existing_unapplied_drafts_still_guard_new_open_and_close() {
    let workspace = Workspace::new();
    let root = workspace.project("current");
    let other = workspace.project("other");
    let (ctx, mut app) = app(Some(root.clone()));
    app.new_event(None);
    app.event_editor.as_mut().unwrap().draft.body = "未提交草稿。\n-> END".into();
    for action in [Pending::New, Pending::Open(other), Pending::Close] {
        app.request_action(action, &ctx);
        assert!(app.draft_action.is_some());
        assert!(app.native_startup.dialog.is_none());
        assert!(!app.allow_close);
        assert_eq!(app.project.root, root);
        assert_eq!(
            app.event_editor.as_ref().unwrap().draft.body,
            "未提交草稿。\n-> END"
        );
        app.draft_action = None;
    }
}

#[test]
fn workspace_escape_waits_for_ime_and_does_not_create_a_project() {
    let (ctx, mut app) = app(None);
    click(&ctx, &mut app, "新建作品");
    for ime in [
        egui::ImeEvent::Preedit("中".into()),
        egui::ImeEvent::Commit("中".into()),
    ] {
        frame(
            &ctx,
            &mut app,
            vec![Event::Ime(ime), key(Key::Escape, Modifiers::NONE)],
        );
        assert!(app.native_startup.dialog.is_some());
        assert!(!app.saved_location);
    }
    frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(app.native_startup.dialog.is_none());
    assert!(app.native_startup.welcome && !app.allow_close);
}

#[test]
fn permission_denied_creation_result_preserves_workspace_and_dialog_until_retry() {
    // 注入 OS PermissionDenied 的保存结果，避免管理员测试进程绕过目录 chmod。
    // 原生验收另用真实不可写位置检查系统权限；这里核对同一错误处理/重试路径。
    let workspace = Workspace::new();
    let current = workspace.project("current");
    let target = workspace.0.join("new-work");
    let (ctx, mut app) = app(Some(current.clone()));
    let before = inventory(&workspace.0);
    let baseline = app.project.content_baseline();
    app.begin_workspace_dialog(WorkspaceOperation::New);
    app.native_startup.dialog.as_mut().unwrap().path = target.to_string_lossy().into_owned();
    let error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "测试目标拒绝写入");
    assert!(!app.accept_created_project(Err(error.to_string())));
    assert_eq!(app.project.root, current);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.native_startup.dialog.is_some());
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    assert!(rendered(&frame(&ctx, &mut app, vec![])).contains("测试目标拒绝写入"));
    assert_eq!(inventory(&workspace.0), before);
    assert!(!app.allow_close);
    click(&ctx, &mut app, "创建作品");
    assert_eq!(app.project.root, target);
    assert!(app.io_error.is_none() && app.native_startup.dialog.is_none());
}

#[test]
fn dirty_project_close_while_path_dialog_is_open_still_uses_unsaved_guard() {
    let workspace = Workspace::new();
    let root = workspace.project("current");
    let (ctx, mut app) = app(Some(root.clone()));
    let entry = app.active_file.clone();
    app.project
        .set_text(&entry, "event start\n  未保存。\n  -> END\n".into())
        .unwrap();
    app.begin_workspace_dialog(WorkspaceOperation::Open);
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let output = ctx.run(input, |ctx| {
        eframe::App::update(&mut app, ctx, &mut eframe::Frame::_new_kittest());
    });
    assert!(!has_close(&output));
    assert!(!app.allow_close);
    assert!(app.native_startup.dialog.is_none());
    assert!(matches!(app.pending, Some(Pending::Close)));
    assert_eq!(app.project.root, root);
    assert!(app.project.document(&entry).unwrap().contains("未保存"));
    assert!(!std::fs::read_to_string(entry).unwrap().contains("未保存"));
}

#[test]
fn welcome_native_close_request_does_not_prompt_to_save_placeholder() {
    let (ctx, mut app) = app(None);
    let baseline = app.project.content_baseline();
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    let output = ctx.run(input, |ctx| {
        eframe::App::update(&mut app, ctx, &mut eframe::Frame::_new_kittest());
    });
    assert!(has_close(&output));
    assert!(app.allow_close);
    assert!(app.pending.is_none() && app.draft_action.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn welcome_preferences_and_small_viewport_keep_project_actions_visible() {
    let (ctx, mut app) = app(None);
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, "外观设置");
    assert!(app.personal.preferences_open);
    frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(!app.personal.preferences_open);
    assert_eq!(app.project.content_baseline(), baseline);
    for _ in 0..3 {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(680.0, 480.0),
                )),
                ..Default::default()
            },
            |ctx| eframe::App::update(&mut app, ctx, &mut eframe::Frame::_new_kittest()),
        );
        for label in ["新建作品", "打开工程", "退出"] {
            assert!(
                output.shapes.iter().any(|shape| {
                    position(&shape.shape, label)
                        .is_some_and(|point| shape.clip_rect.contains(point))
                }),
                "小视口未显示 {label}: {}",
                rendered(&output)
            );
        }
    }
    assert!(app.native_startup.welcome);
}

#[test]
fn focused_path_enter_confirms_but_ime_enter_cannot_create_or_open() {
    let workspace = Workspace::new();
    let root = workspace.0.join("enter-created");
    let (ctx, mut app) = app(None);
    frame(&ctx, &mut app, vec![key(Key::N, Modifiers::COMMAND)]);
    assert!(app.native_startup.dialog.is_some());
    enter_path(&ctx, &mut app, &root);
    for ime in [
        egui::ImeEvent::Preedit(String::new()),
        egui::ImeEvent::Disabled,
    ] {
        frame(
            &ctx,
            &mut app,
            vec![Event::Ime(ime), key(Key::Enter, Modifiers::NONE)],
        );
        assert!(app.native_startup.welcome && !root.exists());
    }
    // 输入法处理回车可能已释放单行框焦点，明确恢复路径焦点后再提交。
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("workspace-startup-path")));
    frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    assert!(app.saved_location && !app.native_startup.welcome);
    assert_eq!(app.project.root, root);
    let (other_ctx, mut other) = self::app(None);
    frame(
        &other_ctx,
        &mut other,
        vec![key(Key::O, Modifiers::COMMAND)],
    );
    enter_path(&other_ctx, &mut other, &root);
    frame(
        &other_ctx,
        &mut other,
        vec![key(Key::Enter, Modifiers::NONE)],
    );
    assert!(other.saved_location && !other.native_startup.welcome);
    assert_eq!(other.project.root, root);
}
