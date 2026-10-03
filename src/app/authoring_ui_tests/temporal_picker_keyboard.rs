//! 逐帧输入回归：真实native发现popup未聚焦搜索、未知ID前缀改变高度丢焦点。
use super::focused_workbench::{focus_app, work_frame};
use super::*;
fn key(key: egui::Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
fn frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
    work_frame(ctx, app, vec2(1188.0, 848.0), events)
}
fn opened() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = focus_app();
    app.open_temporal_issues(&ctx);
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    (ctx, app)
}
#[test]
fn picker_enter_focuses_search_types_each_character_and_escape_returns_to_combo() {
    let (ctx, mut app) = opened();
    let combo = app.temporal_issues.entry_focus.unwrap();
    assert_eq!(ctx.memory(|m| m.focused()), Some(combo));
    frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
    assert!(egui::Popup::is_any_open(&ctx));
    let query = ctx.memory(|m| m.focused()).expect("popup query focus");
    assert_ne!(query, combo);
    assert!(egui::TextEdit::load_state(&ctx, query).is_some());
    for character in "witness".chars() {
        frame(&ctx, &mut app, vec![Event::Text(character.to_string())]);
        assert_eq!(ctx.memory(|m| m.focused()), Some(query));
    }
    frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
    assert_eq!(app.temporal_issues.left, "witness");
    assert!(!egui::Popup::is_any_open(&ctx));
    assert_eq!(ctx.memory(|m| m.focused()), Some(combo));
    frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
    assert!(egui::Popup::is_any_open(&ctx));
    frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
    assert!(!egui::Popup::is_any_open(&ctx));
    assert!(app.temporal_issues.open);
    assert_eq!(ctx.memory(|m| m.focused()), Some(combo));
}
#[test]
fn optional_id_typing_keeps_focus_through_unknown_prefix_layout_changes() {
    let (ctx, mut app) = opened();
    let output = frame(&ctx, &mut app, vec![]);
    let point = visible_text_position(&output, "按稳定 ID 直接输入").unwrap();
    for pressed in [true, false] {
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    for (value, id, left) in [
        ("witness", "temporal-compare-left-input", true),
        ("lights_out", "temporal-compare-right-input", false),
    ] {
        ctx.memory_mut(|m| m.request_focus(egui::Id::new(id)));
        let mut prefix = String::new();
        for character in value.chars() {
            prefix.push(character);
            frame(&ctx, &mut app, vec![Event::Text(character.to_string())]);
            assert_eq!(
                if left {
                    &app.temporal_issues.left
                } else {
                    &app.temporal_issues.right
                },
                &prefix
            );
            assert_eq!(ctx.memory(|m| m.focused()), Some(egui::Id::new(id)));
        }
    }
    let out = frame(&ctx, &mut app, vec![]);
    let mut text = String::new();
    for shape in &out.shapes {
        collect_text(&shape.shape, &mut text);
    }
    assert!(text.contains("左侧事件先于右侧事件"));
}

#[test]
fn expanded_raw_ids_and_cycle_evidence_keep_footer_inside_660_and_900_viewports() {
    for size in [vec2(1040.0, 660.0), vec2(1363.0, 900.0)] {
        let (ctx, mut app) = focus_app();
        app.project.set_text(&app.active_file.clone(),"period night\nevent witness during night follows lights_out\n  -> END\nevent lights_out during night follows witness\n  -> END\nevent lighthouse during night follows lights_out\n  -> END\nevent cave during night follows lighthouse\n  -> END\n".into()).unwrap();
        app.recompile();
        app.open_temporal_issues(&ctx);
        app.temporal_issues.left = "witness".into();
        app.temporal_issues.right = "lights_out".into();
        for _ in 0..3 {
            work_frame(&ctx, &mut app, size, vec![]);
        }
        let out = work_frame(&ctx, &mut app, size, vec![]);
        let point = visible_text_position(&out, "按稳定 ID 直接输入").unwrap();
        for pressed in [true, false] {
            work_frame(
                &ctx,
                &mut app,
                size,
                vec![
                    Event::PointerMoved(point),
                    Event::PointerButton {
                        pos: point,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        for _ in 0..5 {
            work_frame(&ctx, &mut app, size, vec![]);
        }
        let out = work_frame(&ctx, &mut app, size, vec![]);
        assert!(
            visible_text_position(&out, "关闭并返回原焦点").is_some(),
            "footer clipped at {size:?}"
        );
        assert!(
            egui::TextEdit::load_state(&ctx, egui::Id::new("temporal-compare-left-input"))
                .is_some()
        );
        let window = ctx
            .memory(|m| m.area_rect(egui::Id::new("temporal-issues")))
            .unwrap();
        assert!(
            Rect::from_min_size(pos2(0.0, 0.0), size).contains_rect(window),
            "{window:?} outside {size:?}"
        );
    }
}

fn cycle_window(size: egui::Vec2, raw_ids: bool) -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = focus_app();
    app.project.set_text(&app.active_file.clone(),"period night\nevent witness during night follows lights_out\n  -> END\nevent lights_out during night follows witness\n  -> END\nevent lighthouse during night follows lights_out\n  -> END\nevent cave during night follows lighthouse\n  -> END\n".into()).unwrap();
    app.recompile();
    app.open_temporal_issues(&ctx);
    app.temporal_issues.left = "witness".into();
    app.temporal_issues.right = "lights_out".into();
    for _ in 0..4 {
        work_frame(&ctx, &mut app, size, vec![]);
    }
    if raw_ids {
        let output = work_frame(&ctx, &mut app, size, vec![]);
        let point = visible_text_position(&output, "按稳定 ID 直接输入").unwrap();
        for pressed in [true, false] {
            work_frame(
                &ctx,
                &mut app,
                size,
                vec![
                    Event::PointerMoved(point),
                    Event::PointerButton {
                        pos: point,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }
    for _ in 0..6 {
        work_frame(&ctx, &mut app, size, vec![]);
    }
    (ctx, app)
}
fn press_tab(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    shift: bool,
) -> Vec<egui::WidgetInfo> {
    let mut focused = Vec::new();
    for pressed in [true, false] {
        let output = work_frame(
            ctx,
            app,
            size,
            vec![Event::Key {
                key: egui::Key::Tab,
                physical_key: Some(egui::Key::Tab),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers {
                    shift,
                    ..Default::default()
                },
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
fn assert_focused_visible(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    label: &str,
) -> egui::Id {
    for _ in 0..16 {
        work_frame(ctx, app, size, vec![]);
    }
    let output = work_frame(ctx, app, size, vec![]);
    let id = ctx.memory(|m| m.focused()).expect("focused source/header");
    let response = ctx.read_response(id).unwrap();
    assert!(
        output.shapes.iter().any(|shape| {
            text_position(&shape.shape, label).is_some_and(|point| {
                response.rect.contains(point) && shape.clip_rect.contains_rect(response.rect)
            })
        }),
        "focus {label} is clipped: {:?}",
        response.rect
    );
    assert!(visible_text_position(&output, "关闭并返回原焦点").is_some());
    id
}
#[test]
fn tab_scrolls_every_cycle_control_into_view_and_enter_locates_real_follows() {
    for raw_ids in [false, true] {
        let size = vec2(1040.0, 660.0);
        let (ctx, mut app) = cycle_window(size, raw_ids);
        let mut seen =
            std::collections::HashMap::<String, std::collections::HashSet<egui::Id>>::new();
        let labels = [
            "定位事件",
            "定位受阻事件",
            "定位 follows",
            "闭环证据 · 每一条都来自真实 follows",
            "技术详情",
        ];
        for _ in 0..80 {
            let focused = press_tab(&ctx, &mut app, size, false);
            for info in focused {
                if let Some(label) = info.label.filter(|label| labels.contains(&label.as_str())) {
                    let id = assert_focused_visible(&ctx, &mut app, size, &label);
                    seen.entry(label).or_default().insert(id);
                }
            }
            if labels
                .iter()
                .zip([2, 2, 2, 1, 1])
                .all(|(label, count)| seen.get(*label).is_some_and(|ids| ids.len() >= count))
            {
                break;
            }
        }
        for (label, count) in labels.iter().zip([2, 2, 2, 1, 1]) {
            assert_eq!(
                seen.get(*label).map(|ids| ids.len()),
                Some(count),
                "raw_ids={raw_ids}, missing {label}: {seen:?}"
            );
        }
        let mut located = false;
        for _ in 0..30 {
            let focused = press_tab(&ctx, &mut app, size, true);
            if focused
                .iter()
                .any(|info| info.label.as_deref() == Some("定位 follows"))
            {
                assert_focused_visible(&ctx, &mut app, size, "定位 follows");
                work_frame(&ctx, &mut app, size, vec![key(egui::Key::Enter)]);
                assert!(!app.temporal_issues.open);
                assert_eq!(app.tab, Tab::Edit);
                assert!(matches!(app.jump, Some((2 | 4, 1))), "{:?}", app.jump);
                located = true;
                break;
            }
        }
        assert!(located, "Shift+Tab must return to an actual follows source");
    }
}
#[test]
fn time_window_recovers_available_height_after_narrow_viewport_expands() {
    let small = vec2(1040.0, 660.0);
    let large = vec2(1363.0, 900.0);
    let (ctx, mut app) = cycle_window(small, true);
    let before = app.temporal_issues.window_height;
    for _ in 0..6 {
        work_frame(&ctx, &mut app, large, vec![]);
    }
    assert!(
        app.temporal_issues.window_height > before + 180.0,
        "small={before}, large={}",
        app.temporal_issues.window_height
    );
    let output = work_frame(&ctx, &mut app, large, vec![]);
    assert!(visible_text_position(&output, "关闭并返回原焦点").is_some());
    let window = ctx
        .memory(|m| m.area_rect(egui::Id::new("temporal-issues")))
        .unwrap();
    assert!(Rect::from_min_size(pos2(0.0, 0.0), large).contains_rect(window));
}
