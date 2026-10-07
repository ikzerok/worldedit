//! 真实 egui 控件/按键回归；不等同物理 IME 或桌面视觉验收。
use super::*;
use crate::theme::{self, PaletteId};
use egui::{Context, Event, Key, Modifiers};
fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}
fn full_frame(
    ctx: &Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
    size: egui::Vec2,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| {
            let _theme = theme::configure_appearance(ctx, app.personal.appearance());
            app.author_shortcuts(ctx);
            app.source_tab(ctx);
            app.command_window(ctx);
            app.preferences_window(ctx);
            app.capture_edit_focus(ctx);
        },
    )
}
fn frame(
    ctx: &Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> Vec<egui::output::OutputEvent> {
    full_frame(ctx, app, events, egui::vec2(1280.0, 850.0))
        .platform_output
        .events
}
fn app() -> (Context, WorldeditApp, egui::Id, String) {
    let ctx = Context::default();
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    app.personal.settings.reduce_motion = true;
    let text = "event start\n  背景源码不能接收设置窗口的Tab\n".to_owned();
    app.project
        .set_text(&app.active_file.clone(), text.clone())
        .unwrap();
    app.tab = Tab::Edit;
    let id = egui::Id::new(("source", &app.active_file));
    frame(&ctx, &mut app, vec![]);
    ctx.memory_mut(|memory| memory.request_focus(id));
    frame(&ctx, &mut app, vec![]);
    (ctx, app, id, text)
}
fn text_rect(output: &egui::FullOutput, text: &str) -> Option<egui::Rect> {
    fn find(shape: &egui::Shape, text: &str) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Text(shape) if shape.galley.job.text == text => {
                Some(shape.galley.rect.translate(shape.pos.to_vec2()))
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, text)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|shape| find(&shape.shape, text))
}
fn click(ctx: &Context, app: &mut WorldeditApp, point: egui::Pos2, size: egui::Vec2) {
    for pressed in [true, false] {
        full_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
            size,
        );
    }
}
#[test]
fn preferences_command_moves_focus_inside_and_keyboard_edits_draft_without_touching_source() {
    let (ctx, mut app, source_id, text) = app();
    let baseline = app.project.content_baseline();
    let committed = app.personal.settings.appearance;
    frame(
        &ctx,
        &mut app,
        vec![key(Key::P, Modifiers::COMMAND | Modifiers::SHIFT)],
    );
    frame(&ctx, &mut app, vec![Event::Text("设置字体".into())]);
    frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    assert!(app.personal.preferences_open);
    assert!(!app.command_palette.open);
    assert_ne!(ctx.memory(|memory| memory.focused()), Some(source_id));
    let mut tested_font_arrow = false;
    let mut tested_source_arrow = false;
    for _ in 0..48 {
        let events = frame(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)]);
        for event in events {
            if let egui::output::OutputEvent::FocusGained(info) = event {
                if info.label.as_deref() == Some("正文字号") {
                    let before = app.personal.appearance().body_size;
                    frame(&ctx, &mut app, vec![key(Key::ArrowRight, Modifiers::NONE)]);
                    assert!(app.personal.appearance().body_size > before);
                    tested_font_arrow = true;
                }
                if info.label.as_deref() == Some("源码字号") {
                    let before = app.personal.appearance().source_size;
                    frame(&ctx, &mut app, vec![key(Key::ArrowRight, Modifiers::NONE)]);
                    assert!(app.personal.appearance().source_size > before);
                    tested_source_arrow = true;
                }
            }
        }
        assert_eq!(app.project.document(&app.active_file).unwrap(), text);
        assert_eq!(
            app.personal.settings.appearance, committed,
            "预览不能提前提交"
        );
        if tested_font_arrow && tested_source_arrow {
            break;
        }
    }
    assert!(
        tested_font_arrow && tested_source_arrow,
        "两种独立字号必须可由键盘到达"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(!app.personal.preferences_open);
    assert_eq!(app.personal.settings.appearance, committed);
    assert_eq!(*app.personal.appearance(), committed);
}
#[test]
fn preferences_only_takes_initial_focus_and_retakes_it_after_close_and_reopen() {
    let (ctx, mut app, source_id, text) = app();
    app.personal.preferences_open = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    let first = ctx.memory(|memory| memory.focused());
    assert!(first.is_some());
    assert_ne!(first, Some(source_id));
    frame(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)]);
    let second = ctx.memory(|memory| memory.focused());
    assert_ne!(first, second);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(ctx.memory(|memory| memory.focused()), second);
    app.personal.preferences_open = false;
    frame(&ctx, &mut app, vec![]);
    ctx.memory_mut(|memory| memory.request_focus(source_id));
    app.personal.preferences_open = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(ctx.memory(|memory| memory.focused()), first);
    assert_eq!(app.project.document(&app.active_file).unwrap(), text);
}
#[test]
fn real_apply_then_cancel_keeps_commit_and_narrow_two_hundred_percent_keeps_actions_visible() {
    for size in [
        egui::vec2(800.0, 600.0),
        egui::vec2(400.0, 300.0),
        egui::vec2(320.0, 260.0),
    ] {
        let (ctx, mut app, _, _) = app();
        let baseline = app.project.content_baseline();
        app.personal.settings.ui_scale = if size.x < 500.0 { 2.0 } else { 1.0 };
        if size.x < 350.0 {
            app.personal.settings.style = theme::StylePreset::Manuscript;
            app.personal.settings.density = theme::Density::Spacious;
            app.personal.storage_notice =
                Some("设备设置由较新版本保存；原始数据受保护，本次更改不会写回。".into());
        }
        app.personal.preferences_open = true;
        for _ in 0..3 {
            full_frame(&ctx, &mut app, vec![], size);
        }
        app.personal.appearance_draft.as_mut().unwrap().palette = PaletteId::Plum;
        let output = full_frame(&ctx, &mut app, vec![], size);
        for label in ["取消", "应用", "确定"] {
            let rect = text_rect(&output, label).expect("固定操作栏必须绘制");
            assert!(
                egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(rect),
                "{label}: {rect:?} / {size:?}"
            );
        }
        let apply = text_rect(&output, "应用").unwrap().center();
        click(&ctx, &mut app, apply, size);
        assert_eq!(app.personal.settings.palette, PaletteId::Plum);
        assert!(app.personal.preferences_open);
        app.personal.appearance_draft.as_mut().unwrap().palette = PaletteId::Copper;
        let output = full_frame(&ctx, &mut app, vec![], size);
        click(
            &ctx,
            &mut app,
            text_rect(&output, "取消").unwrap().center(),
            size,
        );
        assert!(!app.personal.preferences_open);
        assert_eq!(app.personal.settings.palette, PaletteId::Plum);
        assert_eq!(app.project.content_baseline(), baseline);
        assert!(app.personal.appearance_draft.is_none());
    }
}

#[test]
fn actual_window_x_discards_preview_and_reopening_has_no_stale_draft() {
    let (ctx, mut app, _, _) = app();
    let committed = app.personal.settings.appearance;
    let size = egui::vec2(1280.0, 850.0);
    app.personal.preferences_open = true;
    for _ in 0..3 {
        full_frame(&ctx, &mut app, vec![], size);
    }
    app.personal.appearance_draft.as_mut().unwrap().palette = PaletteId::Copper;
    let output = full_frame(&ctx, &mut app, vec![], size);
    let title = text_rect(&output, "阅读与外观 · 仅此设备").unwrap();
    let close = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::LineSegment { points, .. } => {
                let delta = points[1] - points[0];
                let center = points[0] + delta * 0.5;
                (delta.x.abs() > 8.0
                    && (delta.x.abs() - delta.y.abs()).abs() < 0.1
                    && center.x > title.right()
                    && (center.y - title.center().y).abs() < 4.0)
                    .then_some(center)
            }
            _ => None,
        })
        .max_by(|a, b| a.x.total_cmp(&b.x))
        .expect("actual X close strokes");
    click(&ctx, &mut app, close, size);
    assert!(!app.personal.preferences_open);
    assert_eq!(app.personal.settings.appearance, committed);
    assert!(app.personal.appearance_draft.is_none());
    app.personal.preferences_open = true;
    full_frame(&ctx, &mut app, vec![], size);
    assert_eq!(*app.personal.appearance(), committed);
}

#[test]
fn new_palette_buttons_show_mode_support_and_real_apply_cancel_keep_original_intent() {
    for (palette, label, requested) in [
        (
            PaletteId::Terminal,
            "终端绿 · 仅深色",
            theme::ThemeMode::Light,
        ),
        (
            PaletteId::Neon,
            "霓虹夜景 · 仅深色",
            theme::ThemeMode::Light,
        ),
        (PaletteId::Vellum, "羊皮墨 · 仅浅色", theme::ThemeMode::Dark),
        (PaletteId::Sakura, "樱花", theme::ThemeMode::System),
        (PaletteId::Monochrome, "印刷黑白", theme::ThemeMode::System),
    ] {
        let (ctx, mut app, _, text) = app();
        let size = egui::vec2(1280.0, 850.0);
        app.personal.settings.theme = requested;
        let original = app.personal.settings.appearance;
        let baseline = app.project.content_baseline();
        app.personal.preferences_open = true;
        let mut output = full_frame(&ctx, &mut app, vec![], size);
        for _ in 0..3 {
            output = full_frame(&ctx, &mut app, vec![], size);
        }
        let palette_rect = visible_text_rect(&output, label).expect("新配色入口必须实际可见");
        click(&ctx, &mut app, palette_rect.center(), size);
        output = full_frame(&ctx, &mut app, vec![], size);
        assert_eq!(app.personal.appearance().palette, palette);
        assert_eq!(app.personal.appearance().theme, requested);
        assert_eq!(app.personal.settings.appearance, original);
        assert_eq!(
            theme::resolved(&ctx).effective_mode,
            palette.effective_mode(requested, None)
        );
        if let Some(notice) = palette.mode_support().notice() {
            assert!(
                visible_text_rect(&output, notice).is_some(),
                "固定模式说明必须可见"
            );
        }
        click(
            &ctx,
            &mut app,
            visible_text_rect(&output, "应用")
                .unwrap_or_else(|| {
                    panic!(
                        "Apply hidden for {palette:?}: {:?}, screen {:?}",
                        text_clip(&output, "应用"),
                        ctx.screen_rect()
                    )
                })
                .center(),
            size,
        );
        assert_eq!(app.personal.settings.palette, palette);
        assert_eq!(app.personal.settings.theme, requested);
        output = full_frame(&ctx, &mut app, vec![], size);
        click(
            &ctx,
            &mut app,
            visible_text_rect(&output, "雾松").unwrap().center(),
            size,
        );
        assert_eq!(app.personal.appearance().palette, PaletteId::Mist);
        output = full_frame(&ctx, &mut app, vec![], size);
        click(
            &ctx,
            &mut app,
            visible_text_rect(&output, "取消").unwrap().center(),
            size,
        );
        output = full_frame(&ctx, &mut app, vec![], size);
        assert!(!app.personal.preferences_open);
        assert!(text_rect(&output, "阅读与外观 · 仅此设备").is_none());
        assert_eq!(app.personal.settings.palette, palette);
        assert_eq!(app.personal.settings.theme, requested);
        assert_eq!(app.project.document(&app.active_file).unwrap(), text);
        assert_eq!(app.project.content_baseline(), baseline);
    }
}

fn visible_text_rect(output: &egui::FullOutput, label: &str) -> Option<egui::Rect> {
    fn find(shape: &egui::Shape, clip: egui::Rect, label: &str) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.job.text == label => {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                clip.contains_rect(rect).then_some(rect)
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, clip, label)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|shape| find(&shape.shape, shape.clip_rect, label))
}

#[test]
fn narrow_settings_keyboard_reaches_new_palette_labels_and_cancel() {
    let (ctx, mut app, _, text) = app();
    let size = egui::vec2(320.0, 260.0);
    app.personal.settings.ui_scale = 2.0;
    app.personal.settings.theme = theme::ThemeMode::Light;
    let committed = app.personal.settings.appearance;
    app.personal.preferences_open = true;
    for _ in 0..3 {
        full_frame(&ctx, &mut app, vec![], size);
    }
    let labels = [
        "终端绿 · 仅深色",
        "霓虹夜景 · 仅深色",
        "羊皮墨 · 仅浅色",
        "樱花",
        "印刷黑白",
    ];
    let mut reached = std::collections::BTreeSet::new();
    for _ in 0..48 {
        let output = full_frame(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)], size);
        for event in output.platform_output.events {
            if let egui::output::OutputEvent::FocusGained(info) = event {
                if let Some(label) = info.label.filter(|label| labels.contains(&label.as_str())) {
                    let output = full_frame(&ctx, &mut app, vec![], size);
                    assert!(
                        visible_text_rect(&output, &label).is_some(),
                        "focused palette must scroll into view: {label}: {:?}; screen {:?}",
                        text_clip(&output, &label),
                        ctx.screen_rect()
                    );
                    full_frame(&ctx, &mut app, vec![key(Key::Space, Modifiers::NONE)], size);
                    let selected = PaletteId::ALL
                        .into_iter()
                        .find(|palette| label.starts_with(palette.label()))
                        .unwrap();
                    assert_eq!(
                        app.personal.appearance().palette,
                        selected,
                        "Space must activate the actually focused palette"
                    );
                    assert_eq!(app.personal.appearance().theme, theme::ThemeMode::Light);
                    reached.insert(label);
                }
            }
        }
        if reached.len() == labels.len() {
            break;
        }
    }
    assert_eq!(
        reached.len(),
        labels.len(),
        "all new choices must be keyboard reachable"
    );
    let output = full_frame(&ctx, &mut app, vec![], size);
    click(
        &ctx,
        &mut app,
        visible_text_rect(&output, "取消").unwrap().center(),
        size,
    );
    assert_eq!(*app.personal.appearance(), committed);
    assert!(!app.personal.preferences_open);
    assert_eq!(app.project.document(&app.active_file).unwrap(), text);
}

fn text_clip(output: &egui::FullOutput, label: &str) -> Vec<(egui::Rect, egui::Rect)> {
    output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text == label => Some((
                text.galley.rect.translate(text.pos.to_vec2()),
                shape.clip_rect,
            )),
            _ => None,
        })
        .collect()
}
