//! 原始按键与完整应用帧：变量来源可达，IME 拦截，AltLeft 恢复同一写入。
use super::*;

const LABEL: &str = "打开实际写入来源";
const SIZE: egui::Vec2 = egui::vec2(1188.0, 848.0);

fn full_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let modifiers = events
        .iter()
        .find_map(|event| match event {
            egui::Event::Key { modifiers, .. } => Some(*modifiers),
            _ => None,
        })
        .unwrap_or_default();
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            modifiers,
            events,
            ..Default::default()
        },
        |ctx| eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest()),
    )
}
fn key(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    key: egui::Key,
    modifiers: egui::Modifiers,
) -> Vec<egui::WidgetInfo> {
    let mut focus = Vec::new();
    for pressed in [true, false] {
        let output = full_frame(
            ctx,
            app,
            size,
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
                focus.push(info);
            }
        }
    }
    focus
}
fn tab_to(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2, label: &str) -> egui::Id {
    for _ in 0..200 {
        if key(ctx, app, size, egui::Key::Tab, egui::Modifiers::NONE)
            .iter()
            .any(|info| info.label.as_deref() == Some(label))
        {
            return ctx.memory(|memory| memory.focused()).unwrap();
        }
    }
    panic!("键盘未到达 {label}");
}
fn assert_visible(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2, id: egui::Id) {
    for _ in 0..16 {
        full_frame(ctx, app, size, vec![]);
    }
    let output = full_frame(ctx, app, size, vec![]);
    let response = ctx.read_response(id).unwrap();
    assert!(response.has_focus());
    assert!(
        response.rect.left() >= 0.0 && response.rect.right() <= size.x,
        "source extends beyond viewport: {:?}",
        response.rect
    );
    assert!(
        output.shapes.iter().any(|shape| {
            let mut text = Vec::new();
            labels(&shape.shape, &mut text);
            text.iter().any(|(text, rect)| {
                text == LABEL
                    && response.rect.contains(rect.center())
                    && shape.clip_rect.contains(rect.center())
                    && shape.clip_rect.contains_rect(response.rect)
            })
        }),
        "键盘焦点来源仍被裁切：size={size:?}, id={id:?}, rect={:?}, scroll={}, label_rects={:?}",
        response.rect,
        app.comparison.scroll,
        output
            .shapes
            .iter()
            .flat_map(|shape| {
                let mut text = Vec::new();
                labels(&shape.shape, &mut text);
                text.into_iter()
                    .filter(|(label, _)| label == LABEL)
                    .map(|(_, rect)| (rect, shape.clip_rect))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    );
}

#[test]
fn variable_source_keyboard_return_keeps_real_side_selection_scroll_and_focus() {
    for (size, reversed) in [(SIZE, false), (egui::vec2(760.0, 720.0), true)] {
        let (ctx, mut app) = ready();
        app.personal.pending_restore = false;
        if reversed {
            app.comparison.swap();
        }
        full_frame(&ctx, &mut app, size, vec![]);
        let before = invariant(&app);
        let id = tab_to(&ctx, &mut app, size, LABEL);
        assert_visible(&ctx, &mut app, size, id);
        let scroll = app.comparison.scroll;
        key(
            &ctx,
            &mut app,
            size,
            egui::Key::Enter,
            egui::Modifiers::NONE,
        );
        assert_eq!(app.tab, Tab::Edit);
        assert_eq!(app.comparison.selected_variable.as_deref(), Some("coins"));
        assert_eq!(app.comparison.selected_write, Some((reversed, 1)));
        for _ in 0..4 {
            full_frame(&ctx, &mut app, size, vec![]);
        }
        key(
            &ctx,
            &mut app,
            size,
            egui::Key::ArrowLeft,
            egui::Modifiers::ALT,
        );
        assert_eq!(app.tab, Tab::Play);
        for _ in 0..12 {
            full_frame(&ctx, &mut app, size, vec![]);
            assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
            assert_eq!(app.comparison.scroll, scroll);
            assert_eq!(app.comparison.selected_write, Some((reversed, 1)));
        }
        key(
            &ctx,
            &mut app,
            size,
            egui::Key::Enter,
            egui::Modifiers::NONE,
        );
        assert_eq!(app.tab, Tab::Edit);
        assert_eq!(invariant(&app), before);
    }
}

#[test]
fn variable_terminal_card_selects_same_value_variable_and_clears_other_write() {
    let (ctx, mut app) = ready();
    app.personal.pending_restore = false;
    app.comparison.selected_variable = Some("message".into());
    app.comparison.selected_write = Some((true, 3));
    full_frame(&ctx, &mut app, SIZE, vec![]);
    let before = invariant(&app);
    tab_to(&ctx, &mut app, SIZE, "变量 · coins");
    key(
        &ctx,
        &mut app,
        SIZE,
        egui::Key::Enter,
        egui::Modifiers::NONE,
    );
    assert_eq!(app.comparison.selected_variable.as_deref(), Some("coins"));
    assert_eq!(app.comparison.selected_write, None);
    let id = tab_to(&ctx, &mut app, SIZE, LABEL);
    assert_visible(&ctx, &mut app, SIZE, id);
    assert_eq!(invariant(&app), before);
}

#[test]
fn variable_source_ime_disables_jump_then_restores_same_semantic_focus() {
    let (ctx, mut app) = ready();
    app.personal.pending_restore = false;
    full_frame(&ctx, &mut app, SIZE, vec![]);
    let id = tab_to(&ctx, &mut app, SIZE, LABEL);
    let before = invariant(&app);
    full_frame(
        &ctx,
        &mut app,
        SIZE,
        vec![egui::Event::Ime(egui::ImeEvent::Enabled)],
    );
    full_frame(
        &ctx,
        &mut app,
        SIZE,
        vec![egui::Event::Ime(egui::ImeEvent::Preedit("未提交".into()))],
    );
    key(
        &ctx,
        &mut app,
        SIZE,
        egui::Key::Enter,
        egui::Modifiers::NONE,
    );
    assert_eq!(app.tab, Tab::Play);
    assert!(!ctx.read_response(id).unwrap().enabled());
    full_frame(
        &ctx,
        &mut app,
        SIZE,
        vec![egui::Event::Ime(egui::ImeEvent::Disabled)],
    );
    full_frame(&ctx, &mut app, SIZE, vec![]);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    key(
        &ctx,
        &mut app,
        SIZE,
        egui::Key::Enter,
        egui::Modifiers::NONE,
    );
    assert_eq!(app.tab, Tab::Edit);
    assert_eq!(invariant(&app), before);
}
