use super::*;
fn app() -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    let cc = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&cc, None);
    let root = std::env::temp_dir().join(format!(
        "search-ui-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.project = worldline_core::project::Project::new(&root);
    app.active_file = app.project.entry.clone();
    app.project
        .documents
        .retain(|path, _| path == &app.active_file);
    app.project
        .set_text(
            &app.active_file.clone(),
            "event start\n  needle needle\n  -> END\n".into(),
        )
        .unwrap();
    let result = app.project.compile();
    assert!(!result.has_errors(), "{:?}", result.diagnostics);
    app.recompile();
    app.tab = crate::app::Tab::Edit;
    (ctx, app)
}
fn frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<egui::Event>) {
    let raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1040.0, 660.0),
        )),
        events,
        ..Default::default()
    };
    let _ = ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest())
    });
}
fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}
#[test]
fn search_keyboard_opens_current_and_escape_restores_text_focus() {
    let (ctx, mut app) = app();
    frame(&ctx, &mut app, vec![]);
    let id = egui::Id::new(("source", &app.active_file));
    ctx.memory_mut(|m| m.request_focus(id));
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::F, egui::Modifiers::COMMAND)],
    );
    assert!(app.search_open);
    assert_eq!(app.search_state.scope as u8, Scope::Current as u8);
    app.project_query = "needle".into();
    assert_eq!(app.current_search_hits().unwrap().len(), 2);
    let baseline = app.project.content_baseline();
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(!app.search_open);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(ctx.memory(|m| m.focused()), Some(id));
}
#[test]
fn search_ime_does_not_open_or_close_and_project_undo_does_not_save() {
    let (ctx, mut app) = app();
    frame(
        &ctx,
        &mut app,
        vec![
            egui::Event::Ime(egui::ImeEvent::Preedit("中".into())),
            key(egui::Key::F, egui::Modifiers::COMMAND),
        ],
    );
    assert!(!app.search_open);
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Ime(egui::ImeEvent::Disabled)],
    );
    app.open_search(&ctx, true, true);
    app.project_query = "needle".into();
    app.search_state.replacement = "newword".into();
    app.preview_search_replacement();
    assert!(app.search_state.plan.is_some());
    let before = app.project.content_baseline();
    app.apply_search_replacement();
    assert_ne!(app.project.content_baseline(), before);
    app.close_search(&ctx);
    if let Some(id) = ctx.memory(|m| m.focused()) {
        ctx.memory_mut(|m| m.surrender_focus(id));
    }
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Z, egui::Modifiers::COMMAND)],
    );
    assert_eq!(app.project.content_baseline(), before);
    assert!(app.message.is_none());
    frame(
        &ctx,
        &mut app,
        vec![key(
            egui::Key::Z,
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        )],
    );
    assert!(app
        .project
        .document(&app.active_file)
        .unwrap()
        .contains("newword"));
}
#[test]
fn search_preview_cancel_and_changed_query_cannot_apply_wrong_plan() {
    let (ctx, mut app) = app();
    app.open_search(&ctx, true, true);
    app.project_query = "needle".into();
    app.search_state.replacement = "changed".into();
    app.preview_search_replacement();
    let baseline = app.project.content_baseline();
    app.close_search(&ctx);
    assert!(app.search_state.plan.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    app.open_search(&ctx, true, true);
    app.project_query = "needle".into();
    app.search_state.replacement = "changed".into();
    app.preview_search_replacement();
    let path = app.active_file.clone();
    app.project
        .set_text(&path, "event start\n  external\n".into())
        .unwrap();
    let baseline = app.project.content_baseline();
    app.apply_search_replacement();
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.search_state.error.is_some());
}
#[test]
fn search_cross_file_undo_restores_unapplied_draft_and_redo_checks_later_input() {
    let (ctx, mut app) = app();
    let path = app.active_file.clone();
    let mut buffer = app.project.open_source_writing_buffer(&path).unwrap();
    buffer.replace_source("event start\n  needle freshdraft\n  -> END\n".into());
    app.manuscript.restore_writing_buffers(&[buffer.clone()]);
    app.open_search(&ctx, true, true);
    app.project_query = "needle".into();
    app.search_state.replacement = "newword".into();
    app.preview_search_replacement();
    app.apply_search_replacement();
    assert!(app.project.document(&path).unwrap().contains("freshdraft"));
    assert!(app.manuscript.writing_buffers().is_empty());
    app.edit_undo(false);
    assert!(!app.project.document(&path).unwrap().contains("freshdraft"));
    assert_eq!(
        app.manuscript.writing_buffers()[0].source(),
        buffer.source()
    );
    app.edit_undo(true);
    assert!(app
        .project
        .document(&path)
        .unwrap()
        .contains("newword freshdraft"));
    app.edit_undo(false);
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source("event start\n  later input\n".into());
    let before = app.project.content_baseline();
    app.edit_undo(true);
    assert_eq!(app.project.content_baseline(), before);
    assert!(app.io_error.is_some());
}
#[test]
fn search_local_draft_replace_undo_redo_and_subsequent_input_are_safe() {
    let (ctx, mut app) = app();
    let path = app.active_file.clone();
    let mut buffer = app.project.open_source_writing_buffer(&path).unwrap();
    buffer.replace_source("event start\n  needle draft\n".into());
    app.manuscript.restore_writing_buffers(&[buffer.clone()]);
    app.open_search(&ctx, false, true);
    app.project_query = "needle".into();
    app.search_state.replacement = "newword".into();
    let baseline = app.project.content_baseline();
    app.preview_search_replacement();
    app.apply_search_replacement();
    assert!(app.manuscript.writing_buffers()[0]
        .source()
        .contains("newword"));
    assert_eq!(app.project.content_baseline(), baseline);
    app.edit_undo(false);
    assert_eq!(
        app.manuscript.writing_buffers()[0].source(),
        buffer.source()
    );
    app.edit_undo(true);
    assert!(app.manuscript.writing_buffers()[0]
        .source()
        .contains("newword"));
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source("event start\n  later\n".into());
    app.edit_undo(false);
    assert!(app.manuscript.writing_buffers()[0]
        .source()
        .contains("later"));
}
#[test]
fn search_escape_closes_popup_without_closing_underlying_search_or_draft_form() {
    let (ctx, mut app) = app();
    app.open_search(&ctx, true, false);
    let popup = egui::Id::new("search-test-object-picker");
    let draw = |ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<egui::Event>| {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1040.0, 660.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest());
                egui::Window::new("对象选择测试").show(ctx, |ui| {
                    let response = ui.button("选择对象");
                    egui::Popup::from_response(&response).id(popup).show(|ui| {
                        ui.label("对象候选");
                    });
                });
            },
        );
    };
    draw(&ctx, &mut app, vec![]);
    egui::Popup::open_id(&ctx, popup);
    draw(&ctx, &mut app, vec![]);
    assert!(egui::Popup::is_id_open(&ctx, popup));
    draw(
        &ctx,
        &mut app,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(app.search_open);
    assert!(!egui::Popup::is_id_open(&ctx, popup));
    app.edit_entity(None);
    let before = app.project.content_baseline();
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(app.entity_editor.is_some());
    assert!(app.search_open);
    assert_eq!(app.project.content_baseline(), before);
}
#[test]
fn search_nested_preferences_escape_returns_to_search_then_original_editor() {
    let (ctx, mut app) = app();
    frame(&ctx, &mut app, vec![]);
    let source_id = egui::Id::new(("source", &app.active_file));
    ctx.memory_mut(|m| m.request_focus(source_id));
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::F, egui::Modifiers::COMMAND)],
    );
    let search_id = ctx.memory(|m| m.focused()).unwrap();
    assert_ne!(search_id, source_id);
    app.personal.preferences_open = true;
    frame(&ctx, &mut app, vec![]);
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(!app.personal.preferences_open);
    assert!(app.search_open);
    assert_eq!(ctx.memory(|m| m.focused()), Some(search_id));
    frame(&ctx, &mut app, vec![]);
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(!app.search_open);
    assert_eq!(ctx.memory(|m| m.focused()), Some(source_id));
}
#[test]
fn search_object_names_follow_valid_draft_and_invalid_draft_labels_applied_catalog() {
    let (ctx, mut app) = app();
    let path = app.active_file.clone();
    app.project
        .set_text(
            &path,
            "event start as \"原显示名\"\n  ordinary prose\n".into(),
        )
        .unwrap();
    app.recompile();
    let mut buffer = app.project.open_source_writing_buffer(&path).unwrap();
    buffer.replace_source("event start as \"未应用新显示名\"\n  freshneedle\n".into());
    app.manuscript.restore_writing_buffers(&[buffer]);
    app.open_search(&ctx, true, false);
    app.project_query = "未应用新显示名".into();
    let (objects, warning) = app.search_objects_in_current_drafts();
    assert!(warning.is_none(), "{:?}", warning);
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].display, "未应用新显示名");
    assert!(
        app.current_search_hits().unwrap().is_empty(),
        "名称目录不依赖正文命中"
    );
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source("event start as \"坏稿\"\n  freshneedle {unfinished\n".into());
    app.project_query = "原显示名".into();
    let (objects, warning) = app.search_objects_in_current_drafts();
    assert_eq!(objects.len(), 1);
    assert!(warning.unwrap().contains("已应用版本"));
    app.navigate_search_object(&ctx, &objects[0], true);
    assert_eq!(app.reading_target.as_ref(), Some(&objects[0].target));
    assert!(app.search_state.error.is_none());
    app.project_query = "freshneedle".into();
    app.search_state.source = true;
    assert_eq!(app.current_search_hits().unwrap().len(), 1);
}
#[test]
fn search_cross_tab_current_scope_and_clean_hit_use_visible_source() {
    let (ctx, mut app) = app();
    let root = app.project.root.clone();
    app.project.create_authoring_document(&root.join(".world/project.json"),br#"{"schema_version":1,"language_version":"1.10","required_features":["presentation.manuscripts.v1"],"manuscripts":{"book":".world/book.json"}}"#.to_vec()).unwrap();
    app.project.create_authoring_document(&root.join(".world/book.json"),br#"{"schema_version":1,"id":"book","title":"Book","entries":[{"id":"chapter","kind":"chapter","title":"Chapter","target_ref":{"kind":"event","id":"start"}}]}"#.to_vec()).unwrap();
    let second = app
        .project
        .add_file(std::path::Path::new("second.wl"))
        .unwrap();
    app.project
        .set_text(&second, "event second\n  currentneedle\n".into())
        .unwrap();
    app.recompile();
    app.tab = crate::app::Tab::Manuscript;
    frame(&ctx, &mut app, vec![]);
    assert!(app.manuscript.active_writing_target().is_some());
    app.tab = crate::app::Tab::Edit;
    app.active_file = second.clone();
    app.open_search(&ctx, false, false);
    app.project_query = "currentneedle".into();
    assert_eq!(app.search_request().unwrap().files[0].path, second);
    assert_eq!(app.current_search_hits().unwrap().len(), 1);
    app.tab = crate::app::Tab::Catalog;
    app.open_search(&ctx, true, false);
    app.project_query = "needle".into();
    let hit = app
        .current_search_hits()
        .unwrap()
        .into_iter()
        .find(|hit| hit.path == root.join("world.wl"))
        .unwrap();
    app.go_search_hit(&ctx, &hit);
    assert_eq!(app.tab, crate::app::Tab::Edit);
    assert_eq!(app.active_file, hit.path);
}
#[test]
fn search_prose_navigation_then_escape_keeps_actual_source_focus_and_match_selection() {
    let (ctx, mut app) = app();
    let root = app.project.root.clone();
    let path = app.active_file.clone();
    app.project.create_authoring_document(&root.join(".world/project.json"),br#"{"schema_version":1,"language_version":"1.10","required_features":["presentation.manuscripts.v1"],"manuscripts":{"book":".world/book.json"}}"#.to_vec()).unwrap();
    app.project.create_authoring_document(&root.join(".world/book.json"),br#"{"schema_version":1,"id":"book","title":"Book","entries":[{"id":"chapter","kind":"chapter","title":"Chapter","target_ref":{"kind":"event","id":"start"}}]}"#.to_vec()).unwrap();
    app.recompile();
    app.tab = crate::app::Tab::Manuscript;
    frame(&ctx, &mut app, vec![]);
    let mut buffer = app.project.open_source_writing_buffer(&path).unwrap();
    buffer.replace_source("event start\n  freshneedle 草稿\n  -> END\n".into());
    app.manuscript.restore_writing_buffers(&[buffer]);
    frame(&ctx, &mut app, vec![]);
    let target = worldline_core::TargetRef::new("event", "start");
    let buffer = app
        .manuscript
        .writing_buffers()
        .into_iter()
        .find(|b| b.path() == path)
        .unwrap();
    let offset = buffer.source().find("freshneedle").unwrap();
    let prose_id = egui::Id::new(("writing-prose", &path, &target.kind, &target.id, offset));
    assert!(egui::TextEdit::load_state(&ctx, prose_id).is_some());
    ctx.memory_mut(|memory| memory.request_focus(prose_id));
    frame(&ctx, &mut app, vec![]);
    let baseline = app.project.content_baseline();
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::F, egui::Modifiers::COMMAND)],
    );
    assert!(app.search_open);
    app.project_query = "freshneedle".into();
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
    );
    frame(&ctx, &mut app, vec![]);
    let source_id = egui::Id::new(("writing-source", &path, &target.kind, &target.id));
    assert!(egui::TextEdit::load_state(&ctx, source_id).is_some());
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(!app.search_open);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(source_id));
    let state = egui::TextEdit::load_state(&ctx, source_id).unwrap();
    let range = state.cursor.char_range().unwrap();
    let start = range.primary.index.min(range.secondary.index);
    let end = range.primary.index.max(range.secondary.index);
    let buffer = app
        .manuscript
        .writing_buffers()
        .into_iter()
        .find(|b| b.path() == path)
        .unwrap();
    assert_eq!(
        buffer
            .source()
            .chars()
            .skip(start)
            .take(end - start)
            .collect::<String>(),
        "freshneedle"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    frame(&ctx, &mut app, vec![egui::Event::Text("改写".into())]);
    let buffer = app
        .manuscript
        .writing_buffers()
        .into_iter()
        .find(|b| b.path() == path)
        .unwrap();
    assert!(buffer.source().contains("改写 草稿"));
    assert!(!buffer.source().contains("freshneedle"));
    assert_eq!(app.project.content_baseline(), baseline);
}
#[test]
fn search_linux_replacement_shortcut_opens_replacement_ui() {
    let (ctx, mut app) = app();
    frame(&ctx, &mut app, vec![]);
    if ctx.os() != egui::os::OperatingSystem::Mac {
        frame(
            &ctx,
            &mut app,
            vec![key(egui::Key::H, egui::Modifiers::COMMAND)],
        );
        assert!(app.search_open);
        assert!(app.search_state.replace);
    }
}
