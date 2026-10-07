use super::*;

fn key(key: egui::Key, modifiers: egui::Modifiers) -> Vec<Event> {
    vec![Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }]
}
fn click(ctx: &egui::Context, app: &mut WorldeditApp, point: Pos2, width: f32, height: f32) {
    for pressed in [true, false] {
        render(
            ctx,
            app,
            width,
            height,
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

#[test]
fn focus_navigation_drawer_keeps_files_tabs_and_escape_return_without_mutating_layout() {
    for (width, height) in [(1280.0, 800.0), (400.0, 300.0)] {
        let (ctx, mut app) = app();
        app.personal.settings.style = StylePreset::Focus;
        app.tab = Tab::Edit;
        let settings = serde_json::to_value(&app.personal.settings).unwrap();
        let baseline = app.project.content_baseline();
        for _ in 0..3 {
            render(&ctx, &mut app, width, height, Vec::new());
        }
        let source = egui::Id::new(("source", &app.active_file));
        ctx.memory_mut(|memory| memory.request_focus(source));
        render(&ctx, &mut app, width, height, Vec::new());
        for _ in 0..2 {
            render(
                &ctx,
                &mut app,
                width,
                height,
                key(
                    egui::Key::E,
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                ),
            );
            for _ in 0..3 {
                render(&ctx, &mut app, width, height, Vec::new());
            }
            assert!(app.navigation_drawer_open(&ctx));
            let output = render(&ctx, &mut app, width, height, Vec::new());
            assert!(
                text_position(&output, "返回正文").is_some(),
                "{width}: close is visible"
            );
            render(
                &ctx,
                &mut app,
                width,
                height,
                key(egui::Key::Escape, egui::Modifiers::NONE),
            );
            assert!(!app.navigation_drawer_open(&ctx));
            assert_eq!(ctx.memory(|memory| memory.focused()), Some(source));
            assert_eq!(
                serde_json::to_value(&app.personal.settings).unwrap(),
                settings
            );
            assert_eq!(app.project.content_baseline(), baseline);
        }
        if width > 600.0 {
            app.open_navigation_drawer(&ctx);
            for _ in 0..3 {
                render(&ctx, &mut app, width, height, Vec::new());
            }
            let output = render(&ctx, &mut app, width, height, Vec::new());
            assert!(text_position(&output, "工程文件").is_some());
            assert!(text_position(&output, "引用").is_some());
            let point = text_position(&output, "正文概览").unwrap();
            click(&ctx, &mut app, point, width, height);
            assert_eq!(app.tab, Tab::Overview);
            assert!(!app.navigation_drawer_open(&ctx));
            assert_eq!(
                serde_json::to_value(&app.personal.settings).unwrap(),
                settings
            );
            assert_eq!(app.project.content_baseline(), baseline);
        }
    }
}

#[test]
fn focus_reference_drawer_preserves_original_floating_or_docked_choice() {
    for docked in [false, true] {
        let (ctx, mut app) = app();
        app.personal.settings.style = StylePreset::Focus;
        app.personal.settings.dock_references = docked;
        let id = app
            .reading_panels
            .pin(worldline_core::TargetRef::new("event", "start"))
            .unwrap();
        app.selected_reading_panel = Some(id);
        let settings = serde_json::to_value(&app.personal.settings).unwrap();
        let baseline = app.project.content_baseline();
        for _ in 0..3 {
            render(&ctx, &mut app, 1280.0, 800.0, Vec::new());
        }
        let output = render(&ctx, &mut app, 1280.0, 800.0, Vec::new());
        assert!(app.compact_reference_mode(&ctx));
        let point = text_position(&output, "参考 1").unwrap();
        click(&ctx, &mut app, point, 1280.0, 800.0);
        for _ in 0..3 {
            render(&ctx, &mut app, 1280.0, 800.0, Vec::new());
        }
        assert!(app.compact_reference_open(&ctx));
        render(
            &ctx,
            &mut app,
            1280.0,
            800.0,
            key(egui::Key::Escape, egui::Modifiers::NONE),
        );
        assert!(!app.compact_reference_open(&ctx));
        assert_eq!(app.reading_panels.ids(), vec![id]);
        assert_eq!(
            serde_json::to_value(&app.personal.settings).unwrap(),
            settings
        );
        assert_eq!(app.project.content_baseline(), baseline);
        app.personal.settings.style = StylePreset::Studio;
        render(&ctx, &mut app, 1280.0, 800.0, Vec::new());
        assert!(!app.compact_reference_mode(&ctx));
        assert_eq!(app.personal.settings.dock_references, docked);
    }
}

#[test]
fn drawer_keyboard_moves_within_visible_controls_without_typing_into_source() {
    let (ctx, mut app) = app();
    app.personal.settings.style = StylePreset::Focus;
    app.tab = Tab::Edit;
    for _ in 0..3 {
        render(&ctx, &mut app, 1280.0, 800.0, Vec::new());
    }
    let source = egui::Id::new(("source", &app.active_file));
    ctx.memory_mut(|memory| memory.request_focus(source));
    render(&ctx, &mut app, 1280.0, 800.0, Vec::new());
    let baseline = app.project.content_baseline();
    render(
        &ctx,
        &mut app,
        1280.0,
        800.0,
        key(
            egui::Key::E,
            egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        ),
    );
    for _ in 0..3 {
        render(&ctx, &mut app, 1280.0, 800.0, Vec::new());
    }
    let first = ctx
        .memory(|memory| memory.focused())
        .expect("drawer gets keyboard focus");
    assert_ne!(first, source);
    render(
        &ctx,
        &mut app,
        1280.0,
        800.0,
        key(egui::Key::Tab, egui::Modifiers::NONE),
    );
    assert_ne!(ctx.memory(|memory| memory.focused()), Some(source));
    assert_eq!(app.project.content_baseline(), baseline);
    render(
        &ctx,
        &mut app,
        1280.0,
        800.0,
        key(egui::Key::Tab, egui::Modifiers::SHIFT),
    );
    // egui Previous焦点按契约在下一帧安装；期间也不得回到幕后源码。
    assert_ne!(ctx.memory(|memory| memory.focused()), Some(source));
    render(&ctx, &mut app, 1280.0, 800.0, Vec::new());
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(first));
    render(
        &ctx,
        &mut app,
        1280.0,
        800.0,
        key(egui::Key::Enter, egui::Modifiers::NONE),
    );
    assert!(
        !app.navigation_drawer_open(&ctx),
        "Enter activates focused drawer return"
    );
    render(&ctx, &mut app, 1280.0, 800.0, Vec::new());
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(source));
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn drawer_mouse_entry_and_close_preserve_ime_ownership() {
    let (ctx, mut app) = app();
    app.personal.settings.style = StylePreset::Focus;
    app.tab = Tab::Edit;
    for _ in 0..3 {
        render(&ctx, &mut app, 1280.0, 800.0, Vec::new());
    }
    let source = egui::Id::new(("source", &app.active_file));
    ctx.memory_mut(|memory| memory.request_focus(source));
    let baseline = app.project.content_baseline();
    app.ime_composing = true;
    app.open_navigation_drawer(&ctx);
    assert!(!app.navigation_drawer_open(&ctx));
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(source));
    app.ime_composing = false;
    app.open_navigation_drawer(&ctx);
    assert!(app.navigation_drawer_open(&ctx));
    app.ime_composing = true;
    app.close_navigation_drawer(&ctx);
    assert!(app.navigation_drawer_open(&ctx));
    app.ime_composing = false;
    app.close_navigation_drawer(&ctx);
    assert!(!app.navigation_drawer_open(&ctx));
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn drawer_back_closes_top_layer_before_consuming_author_history() {
    let (ctx, mut app) = app();
    app.personal.settings.style = StylePreset::Focus;
    app.switch_tab(Tab::Edit);
    let history = app.personal.history.len();
    assert!(history > 0);
    let baseline = app.project.content_baseline();
    for _ in 0..3 {
        render(&ctx, &mut app, 1280.0, 800.0, Vec::new());
    }
    app.open_navigation_drawer(&ctx);
    for _ in 0..3 {
        render(&ctx, &mut app, 1280.0, 800.0, Vec::new());
    }
    render(
        &ctx,
        &mut app,
        1280.0,
        800.0,
        key(egui::Key::ArrowLeft, egui::Modifiers::ALT),
    );
    assert!(!app.navigation_drawer_open(&ctx));
    assert_eq!(app.tab, Tab::Edit);
    assert_eq!(app.personal.history.len(), history);
    assert_eq!(app.project.content_baseline(), baseline);
}
