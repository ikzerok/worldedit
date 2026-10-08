//! 合成IME经完整raw_input_hook→App::update；不代表物理操作系统输入法实测。
use super::*;
use crate::app::manuscript::navigation_input;

pub(super) fn native_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    events: Vec<Event>,
) -> egui::FullOutput {
    let mut raw = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)),
        events,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest())
    })
}
pub(super) fn native_settle(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
) -> egui::FullOutput {
    for _ in 0..3 {
        native_frame(ctx, app, size, vec![]);
    }
    native_frame(ctx, app, size, vec![])
}
pub(super) fn setup() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app_with_source(SOURCE);
    let path = app.project.root.join(".world/manuscripts/book.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&path).unwrap().bytes()).unwrap();
    for entry in value["entries"].as_array_mut().unwrap() {
        entry["status"] = serde_json::json!("draft");
        entry["pov"] = serde_json::json!({"kind":"character","id":"lin"});
    }
    app.project
        .set_authoring_document(&path, serde_json::to_vec(&value).unwrap())
        .unwrap();
    app.project.save().unwrap();
    app.recompile();
    app.personal.settings.navigation = false;
    app.manuscript.reader_open = false;
    native_settle(&ctx, &mut app, vec2(1600.0, 1100.0));
    app.manuscript.books.get_mut("book").unwrap().selected_entry = Some("second".into());
    native_settle(&ctx, &mut app, vec2(1600.0, 1100.0));
    (ctx, app)
}
fn key_event(key: egui::Key, pressed: bool) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
pub(super) fn gesture(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    point: egui::Pos2,
) {
    for pressed in [true, false] {
        native_frame(
            ctx,
            app,
            size,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}
pub(super) fn click(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2, label: &str) {
    let output = native_settle(ctx, app, size);
    let point =
        visible(&output, label).unwrap_or_else(|| panic!("missing {label}: {}", texts(&output)));
    gesture(ctx, app, size, point);
}
pub(super) fn focus_field(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    field: &str,
) -> egui::Id {
    native_settle(ctx, app, size);
    let id = navigation_input::id(&app.project.root, "book", field);
    let rect = ctx
        .read_response(id)
        .unwrap_or_else(|| panic!("missing filter {field}"))
        .rect;
    gesture(ctx, app, size, rect.center());
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    id
}
fn value<'a>(app: &'a WorldeditApp, field: &str) -> &'a str {
    match field {
        "text" => &app.manuscript.navigation.session.text,
        "status" => &app.manuscript.navigation.session.status,
        _ => &app.manuscript.navigation.session.pov,
    }
}

#[test]
fn native_navigation_ime_candidate_commit_and_next_navigation_keep_each_filter_owner() {
    for (field, commit) in [("text", "First"), ("status", "draft"), ("pov", "林芜")] {
        let (ctx, mut app) = setup();
        let size = vec2(1600.0, 1100.0);
        let id = focus_field(&ctx, &mut app, size, field);
        let baseline = app.project.content_baseline();
        native_frame(
            &ctx,
            &mut app,
            size,
            vec![Event::Ime(egui::ImeEvent::Enabled)],
        );
        native_frame(
            &ctx,
            &mut app,
            size,
            vec![
                Event::Ime(egui::ImeEvent::Preedit("候选中".into())),
                key_event(egui::Key::ArrowDown, true),
            ],
        );
        assert!(app.manuscript.navigation.input.blocked());
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
        assert_eq!(
            app.manuscript.books["book"].selected_entry.as_deref(),
            Some("second")
        );
        native_frame(
            &ctx,
            &mut app,
            size,
            vec![key_event(egui::Key::ArrowDown, false)],
        );
        native_frame(
            &ctx,
            &mut app,
            size,
            vec![key_event(egui::Key::Enter, true)],
        );
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(id),
            "持续帧Enter仍是候选键"
        );
        native_frame(
            &ctx,
            &mut app,
            size,
            vec![key_event(egui::Key::Enter, false)],
        );
        native_frame(
            &ctx,
            &mut app,
            size,
            vec![
                Event::Ime(egui::ImeEvent::Commit(commit.into())),
                key_event(egui::Key::ArrowDown, true),
            ],
        );
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
        assert_eq!(value(&app, field), commit);
        assert_eq!(
            app.manuscript.books["book"].selected_entry.as_deref(),
            Some("second")
        );
        native_frame(
            &ctx,
            &mut app,
            size,
            vec![key_event(egui::Key::ArrowDown, false)],
        );
        native_frame(
            &ctx,
            &mut app,
            size,
            vec![key_event(
                if field == "text" {
                    egui::Key::Enter
                } else {
                    egui::Key::ArrowDown
                },
                true,
            )],
        );
        assert_eq!(
            app.manuscript.books["book"].selected_entry.as_deref(),
            Some("first")
        );
        assert_ne!(ctx.memory(|memory| memory.focused()), Some(id));
        assert_eq!(app.project.content_baseline(), baseline);
        assert!(app.history.is_empty());
        std::fs::remove_dir_all(&app.project.root).unwrap();
    }
}

#[test]
fn native_navigation_ime_focus_popup_resize_commit_then_returns_to_body() {
    let (ctx, mut app) = setup();
    app.personal.settings.style = crate::theme::StylePreset::Focus;
    let wide = vec2(1600.0, 1100.0);
    click(&ctx, &mut app, wide, "选择章节");
    let id = focus_field(&ctx, &mut app, wide, "status");
    native_frame(
        &ctx,
        &mut app,
        wide,
        vec![Event::Ime(egui::ImeEvent::Preedit("草稿候选".into()))],
    );
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        Some(id),
        "Focus预编辑帧仍由原状态输入框接收；response={:?}, blocked={}, popup={}",
        ctx.read_response(id),
        app.manuscript.navigation.input.blocked(),
        egui::Popup::is_any_open(&ctx)
    );
    let small = vec2(900.0, 380.0);
    ctx.set_pixels_per_point(2.0);
    native_frame(
        &ctx,
        &mut app,
        small,
        vec![key_event(egui::Key::Enter, true)],
    );
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        Some(id),
        "缩放候选帧保留原输入框；response={:?}, blocked={}, popup={}",
        ctx.read_response(id),
        app.manuscript.navigation.input.blocked(),
        egui::Popup::is_any_open(&ctx)
    );
    assert!(
        ctx.read_response(id).is_some(),
        "收起模式仍绘制持焦点筛选框"
    );
    assert_eq!(
        app.manuscript.books["book"].selected_entry.as_deref(),
        Some("second")
    );
    native_frame(
        &ctx,
        &mut app,
        small,
        vec![key_event(egui::Key::Enter, false)],
    );
    native_frame(
        &ctx,
        &mut app,
        small,
        vec![
            Event::Ime(egui::ImeEvent::Commit("draft".into())),
            key_event(egui::Key::Enter, true),
        ],
    );
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    native_frame(
        &ctx,
        &mut app,
        wide,
        vec![key_event(egui::Key::Enter, false)],
    );
    ctx.set_pixels_per_point(1.0);
    native_frame(
        &ctx,
        &mut app,
        wide,
        vec![key_event(egui::Key::ArrowDown, true)],
    );
    native_frame(
        &ctx,
        &mut app,
        wide,
        vec![key_event(egui::Key::ArrowDown, false)],
    );
    native_frame(
        &ctx,
        &mut app,
        wide,
        vec![key_event(egui::Key::Enter, true)],
    );
    native_frame(
        &ctx,
        &mut app,
        wide,
        vec![key_event(egui::Key::Enter, false)],
    );
    assert_eq!(
        app.manuscript.active_writing_target().unwrap().0,
        TargetRef::new("event", "start")
    );
    native_frame(
        &ctx,
        &mut app,
        wide,
        vec![Event::Text("返回正文继续输入".into())],
    );
    assert!(app.manuscript.writing_buffers()[0]
        .source()
        .contains("返回正文继续输入"));
    assert_eq!(app.manuscript.navigation.session.status, "draft");
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn native_navigation_ime_does_not_own_body_or_foreign_window_input() {
    let (ctx, mut app) = setup();
    let size = vec2(1600.0, 1100.0);
    focus_field(&ctx, &mut app, size, "text");
    native_frame(
        &ctx,
        &mut app,
        size,
        vec![Event::Ime(egui::ImeEvent::Preedit("查找候选".into()))],
    );
    let (target, path) = app.manuscript.active_writing_target().unwrap();
    let buffer = app.manuscript.writing_buffers.get(&path).unwrap();
    let projection = app.project.project_writing_buffer(buffer, &target).unwrap();
    let block = projection
        .blocks
        .iter()
        .find(|block| block.kind == worldline_core::manuscript::WritingBlockKind::Prose)
        .unwrap();
    let body_id = egui::Id::new((
        "writing-prose",
        &path,
        &target.kind,
        &target.id,
        block.range.start,
    ));
    ctx.memory_mut(|memory| memory.request_focus(body_id));
    native_frame(
        &ctx,
        &mut app,
        size,
        vec![
            Event::Ime(egui::ImeEvent::Preedit("正文候选".into())),
            key_event(egui::Key::ArrowDown, true),
        ],
    );
    assert!(!app.manuscript.navigation.input.blocked());
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(body_id));
    native_frame(
        &ctx,
        &mut app,
        size,
        vec![Event::Ime(egui::ImeEvent::Commit("正文提交".into()))],
    );
    assert!(app
        .manuscript
        .writing_buffers
        .get(&path)
        .unwrap()
        .source()
        .contains("正文提交"));
    let foreign = egui::Id::new("foreign-navigation-ime-editor");
    let mut foreign_text = String::new();
    let mut initial = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)),
        ..Default::default()
    };
    eframe::App::raw_input_hook(&mut app, &ctx, &mut initial);
    let _ = ctx.run(initial, |ctx| {
        eframe::App::update(&mut app, ctx, &mut eframe::Frame::_new_kittest());
        egui::Window::new("其他窗口").show(ctx, |ui| {
            ui.add(egui::TextEdit::singleline(&mut foreign_text).id(foreign));
        });
    });
    ctx.memory_mut(|memory| {
        memory.request_focus(foreign);
        memory.set_focus_lock_filter(
            foreign,
            egui::EventFilter {
                tab: true,
                escape: true,
                horizontal_arrows: true,
                vertical_arrows: true,
            },
        );
    });
    let mut raw = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)),
        events: vec![
            Event::Ime(egui::ImeEvent::Preedit("其他窗口".into())),
            key_event(egui::Key::ArrowDown, true),
        ],
        ..Default::default()
    };
    eframe::App::raw_input_hook(&mut app, &ctx, &mut raw);
    assert!(
        raw.events.iter().any(|event| matches!(
            event,
            Event::Key {
                key: egui::Key::ArrowDown,
                pressed: true,
                ..
            }
        )),
        "本层不能吞其他窗口候选键"
    );
    let _ = ctx.run(raw, |ctx| {
        eframe::App::update(&mut app, ctx, &mut eframe::Frame::_new_kittest());
        egui::Window::new("其他窗口").show(ctx, |ui| {
            ui.add(egui::TextEdit::singleline(&mut foreign_text).id(foreign));
        });
    });
    assert!(!app.manuscript.navigation.input.blocked());
    assert!(foreign_text.contains("其他窗口"));
    std::fs::remove_dir_all(&app.project.root).unwrap();
}
