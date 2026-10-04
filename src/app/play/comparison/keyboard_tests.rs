//! 实际键盘到达主阅读区控件后必须看得见；不注入焦点或滚轮帮测试过关。
use super::*;

const SIZE: egui::Vec2 = egui::vec2(1188.0, 848.0);
const SOURCE_BUTTON: &str = "打开实际动作来源";

fn navigation_key(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    key: egui::Key,
    shift: bool,
) -> (Vec<egui::WidgetInfo>, egui::FullOutput) {
    let mut focused = Vec::new();
    let mut last = None;
    for pressed in [true, false] {
        let output = frame_events(
            ctx,
            app,
            SIZE / ctx.zoom_factor(),
            vec![egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers {
                    shift,
                    ..Default::default()
                },
            }],
        );
        for event in &output.platform_output.events {
            if let egui::output::OutputEvent::FocusGained(info) = event {
                focused.push(info.clone());
            }
        }
        last = Some(output);
    }
    (focused, last.unwrap())
}

fn tab_to_source(ctx: &egui::Context, app: &mut WorldeditApp) -> egui::Id {
    for _ in 0..128 {
        let (focused, _) = navigation_key(ctx, app, egui::Key::Tab, false);
        if focused.iter().any(|info| {
            info.typ == egui::WidgetType::Button && info.label.as_deref() == Some(SOURCE_BUTTON)
        }) {
            return ctx
                .memory(|memory| memory.focused())
                .expect("具体来源按钮应持有焦点");
        }
    }
    panic!("Tab 未到达实际动作来源按钮");
}

fn assert_source_visible(ctx: &egui::Context, app: &mut WorldeditApp, expected: egui::Id) {
    for _ in 0..16 {
        frame(ctx, app, SIZE / ctx.zoom_factor());
    }
    let output = frame(ctx, app, SIZE / ctx.zoom_factor());
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(expected));
    let response = ctx.read_response(expected).expect("焦点来源应是实际控件");
    let visible = output.shapes.iter().any(|clipped| {
        let mut texts = Vec::new();
        labels(&clipped.shape, &mut texts);
        texts.iter().any(|(text, rect)| {
            text == SOURCE_BUTTON
                && response.rect.contains(rect.center())
                && clipped.clip_rect.contains(rect.center())
        })
    });
    assert!(
        visible,
        "来源获得真实键盘焦点但仍在主阅读区屏外：rect={:?}, scroll={}",
        response.rect, app.comparison.scroll
    );
}

#[test]
fn keyboard_tab_reveals_actual_state_source_without_advancing_live_story() {
    let (ctx, mut app) = setup();
    compare(&ctx, &mut app);
    frame(&ctx, &mut app, SIZE);
    let before = invariant(&app);
    let focused = tab_to_source(&ctx, &mut app);
    assert_source_visible(&ctx, &mut app, focused);
    assert!(app.comparison.scroll > 0.0);
    assert_eq!(invariant(&app), before);
}

#[test]
fn keyboard_shift_tab_reveals_source_after_zoom_reset_on_the_following_frame() {
    let (ctx, mut app) = setup();
    compare(&ctx, &mut app);
    ctx.set_zoom_factor(0.6);
    frame(&ctx, &mut app, SIZE / 0.6);
    let focused = tab_to_source(&ctx, &mut app);
    assert_source_visible(&ctx, &mut app, focused);
    let before = invariant(&app);
    ctx.set_zoom_factor(1.0);
    frame(&ctx, &mut app, SIZE);
    navigation_key(&ctx, &mut app, egui::Key::Tab, false);
    assert_ne!(ctx.memory(|memory| memory.focused()), Some(focused));
    let (events, _) = navigation_key(&ctx, &mut app, egui::Key::Tab, true);
    assert!(events.iter().any(|info| {
        info.typ == egui::WidgetType::Button && info.label.as_deref() == Some(SOURCE_BUTTON)
    }));
    assert_source_visible(&ctx, &mut app, focused);
    assert_eq!(invariant(&app), before);
}

fn scroll_away(ctx: &egui::Context, app: &mut WorldeditApp, focused: egui::Id) {
    let point = ctx.read_response(focused).unwrap().rect.center();
    frame_events(
        ctx,
        app,
        SIZE,
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 2000.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..64 {
        frame(ctx, app, SIZE);
    }
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(focused));
    assert_eq!(app.comparison.scroll, 0.0, "手动阅读不应被旧焦点拉回");
    assert!(ctx.read_response(focused).unwrap().rect.top() > SIZE.y);
}

#[test]
fn focused_source_does_not_pull_back_manual_reading_on_later_frames() {
    let (ctx, mut app) = setup();
    compare(&ctx, &mut app);
    frame(&ctx, &mut app, SIZE);
    let before = invariant(&app);
    let generation = app.comparison.generation;
    let focused = tab_to_source(&ctx, &mut app);
    assert_source_visible(&ctx, &mut app, focused);
    scroll_away(&ctx, &mut app, focused);
    assert_eq!(app.comparison.generation, generation);
    assert!(app.comparison.job.is_none());
    assert_eq!(invariant(&app), before);
}

#[test]
fn keyboard_source_enter_and_alt_left_restore_saved_position_once_before_new_tab() {
    for manual_reading in [false, true] {
        let (ctx, mut app) = setup();
        compare(&ctx, &mut app);
        frame(&ctx, &mut app, SIZE);
        let before = invariant(&app);
        let generation = app.comparison.generation;
        let focused = tab_to_source(&ctx, &mut app);
        assert_source_visible(&ctx, &mut app, focused);
        // 保留真实焦点、用手动滚动选择另一个阅读位置，返回必须优先尊重该位置。
        if manual_reading {
            scroll_away(&ctx, &mut app, focused);
        } else {
            assert!(app.comparison.scroll > 0.0);
        }
        let mut saved = app.comparison_location(Some(&ctx)).unwrap();
        frame_events(
            &ctx,
            &mut app,
            SIZE,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: Some(egui::Key::Enter),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(app.tab, Tab::Edit, "真实 Enter 应执行来源安全桥");
        assert_eq!(app.comparison.selected_action, Some((false, 1)));
        saved.selected_action = app.comparison.selected_action;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
                events: vec![egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: Some(egui::Key::Enter),
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| app.source_tab(ctx),
        );
        for pressed in [true, false] {
            let _ = ctx.run(
                egui::RawInput {
                    modifiers: egui::Modifiers::ALT,
                    events: vec![egui::Event::Key {
                        key: egui::Key::ArrowLeft,
                        physical_key: Some(egui::Key::ArrowLeft),
                        pressed,
                        repeat: false,
                        modifiers: egui::Modifiers::ALT,
                    }],
                    ..Default::default()
                },
                |ctx| app.author_shortcuts(ctx),
            );
        }
        assert_eq!(app.tab, Tab::Play);
        for _ in 0..32 {
            frame(&ctx, &mut app, SIZE);
            assert_eq!(app.comparison_location(Some(&ctx)), Some(saved.clone()));
        }
        assert!(!app.comparison.restore_scroll);
        assert!(app.comparison.restore_focus.is_none());
        // 返回后的下一次真实焦点转移可以再次揭示屏外来源，不能永久禁用滚入。
        navigation_key(&ctx, &mut app, egui::Key::Tab, false);
        navigation_key(&ctx, &mut app, egui::Key::Tab, true);
        assert_source_visible(&ctx, &mut app, focused);
        assert_eq!(app.comparison.generation, generation);
        assert!(app.comparison.job.is_none());
        assert_eq!(invariant(&app), before);
    }
}
