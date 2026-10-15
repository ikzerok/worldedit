//! 合成原生形状的 Tab/IME 会话停用序列；不替代桌面或物理输入法证据。
use super::*;
use egui::{Event, ImeEvent, Key, Modifiers};

fn frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
    frame_at_height(ctx, app, 848.0, events)
}

fn frame_at_height(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    height: f32,
    events: Vec<Event>,
) -> egui::FullOutput {
    let mut raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1188.0, height),
        )),
        events,
        focused: true,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest())
    })
}

fn tab(pressed: bool) -> Event {
    Event::Key {
        key: Key::Tab,
        physical_key: Some(Key::Tab),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn label_rect(output: &egui::FullOutput, label: &str, complete: bool) -> egui::Rect {
    fn find(shape: &egui::Shape, label: &str) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.galley.rect.translate(text.pos.to_vec2()))
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, label)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|shape| {
            find(&shape.shape, label).and_then(|rect| {
                if complete {
                    shape.clip_rect.contains_rect(rect).then_some(rect)
                } else {
                    let visible = rect.intersect(shape.clip_rect);
                    visible.is_positive().then_some(visible)
                }
            })
        })
        .unwrap_or_else(|| panic!("actual label must be fully painted: {label}"))
}

fn ordinary_search_to_structural_button(disabled_with_release: bool, idle_enabled: bool) {
    let (ctx, mut app) = setup();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.select_event("start");
    app.tab = crate::app::Tab::Timeline;
    app.personal.pending_restore = false;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    let output = frame(&ctx, &mut app, vec![]);
    let search_rect = label_rect(&output, "筛选候选：名称 / ID / 时段 / 根 / 来源", false);
    let search_candidates: Vec<_> = ctx.viewport(|viewport| {
        viewport
            .this_pass
            .widgets
            .layers()
            .flat_map(|(_, widgets)| widgets.iter())
            .filter(|widget| widget.enabled && widget.rect.contains(search_rect.center()))
            .map(|widget| widget.id)
            .collect()
    });
    let search = search_candidates
        .into_iter()
        .find(|id| egui::TextEdit::load_state(&ctx, *id).is_some())
        .expect("the real predecessor search TextEdit exists");
    let baseline = app.project.content_baseline();
    let original = app.event_editor.as_ref().unwrap().draft.clone();
    for _ in 0..150 {
        if ctx.memory(|memory| memory.focused()) == Some(search) {
            break;
        }
        frame(&ctx, &mut app, vec![tab(true)]);
        frame(&ctx, &mut app, vec![tab(false)]);
        frame(&ctx, &mut app, vec![]);
        assert!(app.event_editor.as_ref().unwrap().draft == original);
    }
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(search));
    if idle_enabled {
        frame(&ctx, &mut app, vec![Event::Ime(ImeEvent::Enabled)]);
        frame(&ctx, &mut app, vec![]);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(search));
    }
    let output = frame(&ctx, &mut app, vec![tab(true)]);
    let button = ctx
        .memory(|memory| memory.focused())
        .expect("Tab enters the next control");
    assert_ne!(button, search);
    let response = ctx.read_response(button).unwrap();
    assert!(response.enabled());
    assert!(response
        .rect
        .contains_rect(label_rect(&output, "＋ 添加选项", true)));
    assert!(ctx.screen_rect().contains_rect(response.rect));
    // 原生按下与松开之间会有重绘，不能只覆盖单个批次事件。
    for _ in 0..2 {
        frame(&ctx, &mut app, vec![]);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(button));
    }
    if disabled_with_release {
        frame(
            &ctx,
            &mut app,
            vec![tab(false), Event::Ime(ImeEvent::Disabled)],
        );
    } else {
        frame(&ctx, &mut app, vec![tab(false)]);
        frame(&ctx, &mut app, vec![Event::Ime(ImeEvent::Disabled)]);
    }
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        Some(button),
        "stopping an idle IME session after Tab must not disable the newly focused action"
    );
    assert!(ctx.read_response(button).unwrap().enabled());
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(button));
    }
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.event_editor.as_ref().unwrap().draft == original);
}

#[test]
fn condition_ui_native_tab_release_disabled_keeps_new_structural_focus() {
    ordinary_search_to_structural_button(true, false);
}

#[test]
fn condition_ui_native_tab_then_disabled_keeps_new_structural_focus() {
    ordinary_search_to_structural_button(false, false);
}

#[test]
fn condition_ui_native_idle_enabled_allows_tab_into_structural_action() {
    ordinary_search_to_structural_button(true, true);
}

#[test]
fn condition_ui_native_coalesced_tab_disabled_reveals_entire_offscreen_candidate() {
    use crate::chrome::action_layout::trace;
    let (ctx, mut app) = setup();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let source = format!(
        "world room\ntag calm\nstate mood on world room with calm\n{}",
        app.project
            .document(&app.active_file)
            .unwrap()
            .replace("if allowed", "if has(mood, calm)")
    );
    app.project
        .set_text(&app.active_file.clone(), source)
        .unwrap();
    app.project.save().unwrap();
    app.recompile();
    app.select_event("start");
    app.tab = crate::app::Tab::Timeline;
    app.personal.pending_restore = false;
    let mut height = 760.0;
    for _ in 0..3 {
        frame_at_height(&ctx, &mut app, height, vec![]);
    }
    let baseline = app.project.content_baseline();
    let draft = app.event_editor.as_ref().unwrap().draft.clone();
    for _ in 0..150 {
        if trace(&ctx).iter().any(|response| {
            response.role == "condition/state-picker"
                && response.enabled
                && ctx.memory(|memory| memory.focused()) == Some(response.id)
        }) {
            break;
        }
        frame_at_height(&ctx, &mut app, height, vec![tab(true)]);
        frame_at_height(&ctx, &mut app, height, vec![tab(false)]);
        frame_at_height(&ctx, &mut app, height, vec![]);
        assert!(app.event_editor.as_ref().unwrap().draft == draft);
    }
    for _ in 0..24 {
        frame_at_height(&ctx, &mut app, height, vec![]);
    }
    let controls = || {
        let actions = trace(&ctx);
        let state = actions
            .iter()
            .find(|r| r.role == "condition/state-picker")
            .unwrap()
            .clone();
        let tag = actions
            .iter()
            .find(|r| r.role == "condition/tag-picker")
            .unwrap()
            .clone();
        (state, tag)
    };
    let (state, _) = controls();
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(state.id));
    // 只改变真实客户端高度，使前一控件完整可见而目标底部在滚动clip之外。
    // 每次等真实布局与滚动动画结束再读几何，不能假设resize会等量改变clip。
    for _ in 0..8 {
        let (state, tag) = controls();
        if state.clip.contains_rect(state.rect) && !tag.clip.contains_rect(tag.rect) {
            break;
        }
        let wanted_bottom = (tag.rect.bottom() - 8.0).max(state.rect.bottom() + 4.0);
        height += wanted_bottom - tag.clip.bottom();
        assert!(
            (300.0..=1100.0).contains(&height),
            "bounded real client: {height}"
        );
        for _ in 0..24 {
            frame_at_height(&ctx, &mut app, height, vec![]);
        }
    }
    let (state, tag) = controls();
    assert!(
        state.clip.contains_rect(state.rect),
        "preceding control: {state:?}"
    );
    assert!(
        !tag.clip.contains_rect(tag.rect),
        "target must genuinely start offscreen: {tag:?}"
    );
    frame_at_height(
        &ctx,
        &mut app,
        height,
        vec![tab(true), Event::Ime(ImeEvent::Disabled)],
    );
    frame_at_height(&ctx, &mut app, height, vec![tab(false)]);
    for _ in 0..3 {
        frame_at_height(&ctx, &mut app, height, vec![]);
    }
    let (_, tag) = controls();
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(tag.id));
    assert!(tag.enabled);
    assert!(
        tag.clip.expand(1.0).contains_rect(tag.rect),
        "same-intent Tab must reveal the entire action despite idle Disabled: {tag:?}"
    );
    assert!(ctx.screen_rect().expand(1.0).contains_rect(tag.rect));
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.event_editor.as_ref().unwrap().draft == draft);
}
