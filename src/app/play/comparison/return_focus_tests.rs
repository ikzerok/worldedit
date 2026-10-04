//! 完整 update + egui end_pass 回归，不能把 AltLeft 与返回页绘制拆开。
use super::*;

const SIZE: egui::Vec2 = egui::vec2(1188.0, 848.0);
const SOURCE_BUTTON: &str = "打开实际动作来源";

fn full_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let modifiers = events.iter().find_map(|event| match event {
        egui::Event::Key { modifiers, .. } => Some(*modifiers),
        _ => None,
    });
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
            modifiers: modifiers.unwrap_or_default(),
            events,
            ..Default::default()
        },
        |ctx| eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest()),
    )
}

fn key(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    key: egui::Key,
    modifiers: egui::Modifiers,
) -> Vec<egui::WidgetInfo> {
    let mut focused = Vec::new();
    for pressed in [true, false] {
        let output = full_frame(
            ctx,
            app,
            vec![egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            }],
        );
        for event in output.platform_output.events {
            if let egui::output::OutputEvent::FocusGained(info) = event {
                focused.push(info);
            }
        }
    }
    focused
}

fn source_focus(ctx: &egui::Context, app: &mut WorldeditApp) -> egui::Id {
    for _ in 0..180 {
        let events = key(ctx, app, egui::Key::Tab, egui::Modifiers::NONE);
        if events.iter().any(|info| {
            info.typ == egui::WidgetType::Button && info.label.as_deref() == Some(SOURCE_BUTTON)
        }) {
            return ctx.memory(|memory| memory.focused()).unwrap();
        }
    }
    panic!("完整应用 Tab 未到达实际来源按钮");
}

fn return_roundtrip(move_source_focus: bool) {
    let (ctx, mut app) = setup();
    app.personal.pending_restore = false;
    compare(&ctx, &mut app);
    full_frame(&ctx, &mut app, vec![]);
    let before = invariant(&app);
    let focused = source_focus(&ctx, &mut app);
    for _ in 0..24 {
        full_frame(&ctx, &mut app, vec![]);
    }
    assert!(app.comparison.scroll > 0.0);
    let scroll = app.comparison.scroll;
    let identity = (
        app.comparison.a,
        app.comparison.b,
        app.comparison.generation,
    );
    key(&ctx, &mut app, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(app.tab, Tab::Edit);
    let source_id = egui::Id::new(("source", &app.active_file));
    for _ in 0..8 {
        full_frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(source_id));
    if move_source_focus {
        // 真实点击源码页标题空白区域，让编辑器失焦；不调用 request/surrender_focus。
        let output = full_frame(&ctx, &mut app, vec![]);
        let relative = app
            .active_file
            .strip_prefix(&app.project.root)
            .unwrap()
            .display()
            .to_string();
        let mut text = Vec::new();
        for shape in &output.shapes {
            labels(&shape.shape, &mut text);
        }
        let point = text
            .iter()
            .find(|(label, _)| label == &relative)
            .unwrap()
            .1
            .center();
        for pressed in [true, false] {
            full_frame(
                &ctx,
                &mut app,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert_ne!(ctx.memory(|memory| memory.focused()), Some(source_id));
    }
    key(&ctx, &mut app, egui::Key::ArrowLeft, egui::Modifiers::ALT);
    assert_eq!(app.tab, Tab::Play);
    for _ in 0..8 {
        full_frame(&ctx, &mut app, vec![]);
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(focused),
            "同帧返回后焦点被覆盖，source_focus_moved={move_source_focus}"
        );
        assert!(ctx.read_response(focused).unwrap().has_focus());
        assert_eq!(app.comparison.scroll, scroll);
    }
    assert_eq!(
        (
            app.comparison.a,
            app.comparison.b,
            app.comparison.generation
        ),
        identity
    );
    assert_eq!(app.comparison.selected_action, Some((false, 1)));
    key(&ctx, &mut app, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(app.tab, Tab::Edit, "返回后无需再次 Tab 即可重开同一来源");
    assert_eq!(invariant(&app), before);
}

#[test]
fn full_update_return_from_focused_source_keeps_source_button_focus_and_enter() {
    return_roundtrip(false);
}

#[test]
fn full_update_return_from_unfocused_source_keeps_source_button_focus_and_enter() {
    return_roundtrip(true);
}
