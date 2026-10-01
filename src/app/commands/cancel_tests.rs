use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

fn fixture_root(clock_tick: u128) -> std::path::PathBuf {
    static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);
    // 时间戳只防跨进程旧目录重名；并行测试不能依赖系统时钟精度。
    std::env::temp_dir().join(format!(
        "cancel-layers-{}-{clock_tick}-{}",
        std::process::id(),
        NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
    ))
}

#[test]
fn fixture_roots_remain_unique_when_parallel_clock_ticks_match() {
    let roots = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..32).map(|_| scope.spawn(|| fixture_root(123))).collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
    });
    assert_eq!(roots.len(), 32, "同一时钟值下并行夹具必须使用不同工作区");
}

fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 1100.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest()),
    )
}
fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
fn app(dirty: bool) -> (egui::Context, WorldeditApp, egui::Id) {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = fixture_root(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    );
    app.project = worldline_core::project::Project::new(&root);
    app.active_file = app.project.entry.clone();
    app.project
        .documents
        .retain(|path, _| path == &app.active_file);
    app.project
        .set_text(
            &app.active_file.clone(),
            "entity harbor kind place as \"海港\"\n".into(),
        )
        .unwrap();
    app.project.create_authoring_document(&root.join(".world/project.json"),
        br#"{"schema_version":1,"language_version":"1.10","required_features":["content.entities.v1"],"maps":{},"graph_views":{}}"#.to_vec()).unwrap();
    app.project.save().unwrap();
    app.saved_location = true;
    app.recompile();
    app.tab = Tab::Edit;
    app.edit_entity(Some("harbor"));
    if dirty {
        app.entity_editor.as_mut().unwrap().draft.display = "未应用海港".into();
    }
    frame(&ctx, &mut app, vec![]);
    let id = egui::Id::new("entity-editor-name");
    let mut state = egui::TextEdit::load_state(&ctx, id).unwrap();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(1),
        )));
    state.store(&ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
    frame(&ctx, &mut app, vec![]);
    (ctx, app, id)
}

#[test]
fn escape_closes_only_top_transient_over_clean_or_dirty_entity_and_preserves_selection() {
    for dirty in [false, true] {
        let (ctx, mut app, id) = app(dirty);
        let before = app.project.content_baseline();
        let draft = serde_json::to_value(&app.entity_editor.as_ref().unwrap().draft).unwrap();
        let cursor = egui::TextEdit::load_state(&ctx, id)
            .unwrap()
            .cursor
            .char_range();
        app.open_search(&ctx, true, false);
        frame(&ctx, &mut app, vec![]);
        let search_focus = ctx.memory(|m| m.focused());
        app.open_commands(&ctx, false);
        app.command_palette.query = "no-such-object".into();
        frame(&ctx, &mut app, vec![]);
        let command_focus = ctx.memory(|m| m.focused());
        app.personal.preferences_open = true;
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
        assert!(!app.personal.preferences_open);
        assert!(app.command_palette.open && app.search_open);
        assert_eq!(ctx.memory(|m| m.focused()), command_focus);
        frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
        assert!(!app.command_palette.open && app.search_open);
        assert_eq!(ctx.memory(|m| m.focused()), search_focus);
        frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
        assert!(!app.search_open);
        assert_eq!(ctx.memory(|m| m.focused()), Some(id));
        assert_eq!(
            egui::TextEdit::load_state(&ctx, id)
                .unwrap()
                .cursor
                .char_range(),
            cursor
        );
        assert_eq!(
            serde_json::to_value(&app.entity_editor.as_ref().unwrap().draft).unwrap(),
            draft
        );
        assert_eq!(app.project.content_baseline(), before);
        assert!(!app.project.is_dirty());
        frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
        assert!(app.entity_editor.is_some(), "底层复杂表单仍须明确取消");
        let _ = std::fs::remove_dir_all(app.project.root);
    }
}

#[test]
fn command_enter_owns_nested_key_and_no_results_stays_open() {
    let (ctx, mut app, id) = app(true);
    app.open_search(&ctx, true, false);
    frame(&ctx, &mut app, vec![]);
    app.open_commands(&ctx, true);
    app.command_palette.query = "no-command".into();
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
    assert!(app.command_palette.open);
    app.command_palette.query = "显示 / 隐藏导航".into();
    let before = app.personal.settings.navigation;
    frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
    assert!(!app.command_palette.open && app.search_open);
    assert_eq!(app.personal.settings.navigation, !before);
    frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
    assert_eq!(ctx.memory(|m| m.focused()), Some(id));
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.display,
        "未应用海港"
    );
    let _ = std::fs::remove_dir_all(app.project.root);
}

#[test]
fn command_escape_waits_for_ime_and_survives_external_refresh() {
    let (ctx, mut app, id) = app(true);
    app.open_commands(&ctx, false);
    frame(&ctx, &mut app, vec![]);
    for event in [
        egui::ImeEvent::Preedit("中".into()),
        egui::ImeEvent::Commit("中".into()),
    ] {
        frame(
            &ctx,
            &mut app,
            vec![egui::Event::Ime(event), key(egui::Key::Escape)],
        );
        assert!(app.command_palette.open);
    }
    let source = std::fs::read_to_string(&app.active_file).unwrap();
    std::fs::write(&app.active_file, format!("{source}\n// 外部刷新\n")).unwrap();
    app.last_refresh = Instant::now() - Duration::from_secs(2);
    frame(&ctx, &mut app, vec![]);
    assert!(app.stale_form);
    let before = app.project.content_baseline();
    frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
    assert!(!app.command_palette.open);
    assert_eq!(ctx.memory(|m| m.focused()), Some(id));
    assert_eq!(app.project.content_baseline(), before);
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.display,
        "未应用海港"
    );
    assert!(!app
        .entity_editor
        .as_ref()
        .unwrap()
        .guard
        .is_current(&app.project, app.version));
    let _ = std::fs::remove_dir_all(app.project.root);
}

#[test]
fn command_window_x_restores_lower_form_focus_without_changes() {
    let (ctx, mut app, id) = app(true);
    let before = app.project.content_baseline();
    app.open_commands(&ctx, false);
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    let rect = ctx
        .memory(|m| m.area_rect(egui::Id::new("author-command-palette")))
        .unwrap();
    // 点击实际绘制的关闭叉号，不假定主题标题栏高度/边距。
    let output = frame(&ctx, &mut app, vec![]);
    fn close_cross(shape: &egui::Shape, rect: egui::Rect) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::LineSegment { points, .. } => {
                let delta = points[1] - points[0];
                let center = points[0] + delta * 0.5;
                (rect.contains(center)
                    && center.x > rect.right() - 50.0
                    && center.y < rect.top() + 45.0
                    && delta.x.abs() > 5.0
                    && (delta.x.abs() - delta.y.abs()).abs() < 0.5)
                    .then_some(center)
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| close_cross(shape, rect)),
            _ => None,
        }
    }
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| close_cross(&shape.shape, rect))
        .expect("命令窗口须实际绘制关闭叉号");
    for pressed in [true, false] {
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert!(!app.command_palette.open);
    assert_eq!(ctx.memory(|m| m.focused()), Some(id));
    assert_eq!(app.project.content_baseline(), before);
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.display,
        "未应用海港"
    );
    let _ = std::fs::remove_dir_all(app.project.root);
}

#[test]
fn settings_command_enter_then_escape_returns_to_entity_focus_and_selection() {
    let (ctx, mut app, id) = app(true);
    let before = app.project.content_baseline();
    let cursor = egui::TextEdit::load_state(&ctx, id)
        .unwrap()
        .cursor
        .char_range();
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Key {
            key: egui::Key::P,
            physical_key: Some(egui::Key::P),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        }],
    );
    assert!(app.command_palette.open && app.command_palette.commands_only);
    frame(&ctx, &mut app, vec![egui::Event::Text("设置字体".into())]);
    frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
    assert!(!app.command_palette.open && app.personal.preferences_open);
    frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
    assert!(!app.personal.preferences_open);
    assert_eq!(ctx.memory(|m| m.focused()), Some(id));
    assert_eq!(
        egui::TextEdit::load_state(&ctx, id)
            .unwrap()
            .cursor
            .char_range(),
        cursor
    );
    assert_eq!(app.project.content_baseline(), before);
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.display,
        "未应用海港"
    );
    let _ = std::fs::remove_dir_all(app.project.root);
}
