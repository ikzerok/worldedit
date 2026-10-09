//! 完整 chrome、Studio 200% 与真实 egui 点击/滚轮；不是原生截图的替代品。
use super::*;

const SIZE: Vec2 = vec2(400.0, 300.0);

fn update(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
    let mut raw = RawInput {
        screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        time: Some(ctx.cumulative_frame_nr() as f64 / 60.0),
        events,
        ..Default::default()
    };
    raw.viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .native_pixels_per_point = Some(1.0);
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    let output = ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest());
    });
    assert_eq!(ctx.screen_rect().size(), SIZE);
    assert_eq!(output.pixels_per_point, 2.0);
    output
}

fn settle(ctx: &egui::Context, app: &mut WorldeditApp) -> egui::FullOutput {
    update(ctx, app, vec![]);
    update(ctx, app, vec![]);
    update(ctx, app, vec![])
}

fn point(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
    let mut rects = Vec::new();
    for shape in &output.shapes {
        text_rects(&shape.shape, label, shape.clip_rect, &mut rects);
    }
    let screen = Rect::from_min_size(egui::Pos2::ZERO, SIZE);
    rects
        .iter()
        .find(|(rect, clip)| screen.contains_rect(*rect) && clip.contains_rect(*rect))
        .map(|(rect, _)| rect.center())
}

fn press(ctx: &egui::Context, app: &mut WorldeditApp, position: egui::Pos2) {
    for pressed in [true, false] {
        update(
            ctx,
            app,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

fn click_label(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let output = settle(ctx, app);
    press(ctx, app, visible(&output, label, SIZE));
}

fn body_clip(output: &egui::FullOutput) -> Rect {
    let mut rects = Vec::new();
    for shape in &output.shapes {
        text_rects(&shape.shape, "seed ", shape.clip_rect, &mut rects);
    }
    let clip = rects.first().expect("当前快照元数据须位于正文滚动区").1;
    assert!(
        clip.height() >= 30.0,
        "完整 chrome 后至少可完整阅读/点击一行控件：{clip:?}"
    );
    assert!(Rect::from_min_size(egui::Pos2::ZERO, SIZE).contains_rect(clip));
    clip
}

fn scroll_find(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    label: &str,
    pointer: egui::Pos2,
    direction: f32,
) -> egui::Pos2 {
    for _ in 0..80 {
        let output = settle(ctx, app);
        if let Some(position) = point(&output, label) {
            return position;
        }
        update(
            ctx,
            app,
            vec![
                Event::PointerMoved(pointer),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, 20.0 * direction),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    let output = settle(ctx, app);
    visible(&output, label, SIZE)
}

fn choose_pane(ctx: &egui::Context, app: &mut WorldeditApp, current: &str, next: &str) {
    click_label(ctx, app, current);
    click_label(ctx, app, next);
}

fn start_full_app() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app();
    app.personal.pending_restore = false;
    app.personal.settings.style = crate::theme::StylePreset::Studio;
    app.personal.settings.ui_scale = 2.0;
    app.personal.settings.reduce_motion = true;
    ctx.set_zoom_factor(2.0);
    // 原普通会话先真实走到选择，之后仍须完全保留。
    app.start_play_inner(app.applied_play_scope().unwrap());
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .continue_story()
        .unwrap();
    settle(&ctx, &mut app);
    app.request_draft_rehearsal(&ctx);
    wait_prepared(&ctx, &mut app);
    click_label(&ctx, &mut app, "明确开始这份草稿试演");
    assert!(app.draft_rehearsal.active && !app.draft_rehearsal.has_pending());
    wait_idle(&ctx, &mut app);
    (ctx, app)
}

#[test]
fn full_app_studio_200_percent_keeps_real_draft_text_choice_and_all_panes_reachable() {
    let (ctx, mut app) = start_full_app();
    let baseline = app.project.content_baseline();
    let buffer = app.manuscript.writing_buffers()[0].clone();
    let regular = app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .save()
        .unwrap();
    let output = settle(&ctx, &mut app);
    // 必须包含真实外层 chrome，不能退回直接绘制 play_tab 的大视口。
    for label in [
        "worldedit",
        "保存全部",
        "重做",
        "草稿隔离试演",
        "返回原草稿位置",
        "试演操作",
        "正文与选择",
    ] {
        visible(&output, label, SIZE);
    }
    let pointer = body_clip(&output).center();
    scroll_find(&ctx, &mut app, "未应用真实正文🌦️", pointer, -1.0);
    let choice = scroll_find(&ctx, &mut app, "选择：继续", pointer, -1.0);
    assert_eq!(app.draft_rehearsal.running.as_ref().unwrap().view.turns, 0);
    press(&ctx, &mut app, choice);
    wait_idle(&ctx, &mut app);
    let running = app.draft_rehearsal.running.as_ref().unwrap();
    assert!(running.view.ended, "窄视口里的真实选择必须确实执行");
    assert_eq!(running.view.turns, 1);
    assert_eq!(
        running.view.inspection.as_ref().unwrap().items[0]
            .current
            .display,
        "4"
    );
    choose_pane(&ctx, &mut app, "正文与选择", "状态与变化");
    assert!(app.draft_rehearsal.pane == Pane::State);
    scroll_find(&ctx, &mut app, "返回此状态声明", pointer, -1.0);
    choose_pane(&ctx, &mut app, "状态与变化", "实际条件证据");
    assert!(app.draft_rehearsal.pane == Pane::Conditions);
    scroll_find(&ctx, &mut app, "条件证据 · 当前隔离试演快照", pointer, 1.0);
    choose_pane(&ctx, &mut app, "实际条件证据", "输入范围");
    assert!(app.draft_rehearsal.pane == Pane::Scope);
    scroll_find(&ctx, &mut app, "纳入未应用正文", pointer, -1.0);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(
        app.manuscript.writing_buffers()[0].source(),
        buffer.source()
    );
    assert_eq!(
        app.manuscript.writing_buffers()[0].generation(),
        buffer.generation()
    );
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .save()
            .unwrap(),
        regular
    );
    click_label(&ctx, &mut app, "返回原草稿位置");
    assert_eq!(app.tab, Tab::Manuscript, "{:?}", app.draft_rehearsal.notice);
    assert_eq!(
        app.manuscript.writing_buffers()[0].source(),
        buffer.source()
    );
}

#[test]
fn full_app_studio_200_percent_discloses_and_executes_every_rehearsal_menu_action() {
    for action in ["重新试演当前稿…", "保留试演，查看已应用稿", "关闭试演"]
    {
        let (ctx, mut app) = start_full_app();
        let baseline = app.project.content_baseline();
        let buffer = app.manuscript.writing_buffers()[0].clone();
        let regular = app
            .play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .save()
            .unwrap();
        let stamp = app
            .draft_rehearsal
            .running
            .as_ref()
            .unwrap()
            .view
            .inspection
            .as_ref()
            .unwrap()
            .stamp;
        click_label(&ctx, &mut app, "试演操作");
        let output = settle(&ctx, &mut app);
        for label in ["重新试演当前稿…", "保留试演，查看已应用稿", "关闭试演"]
        {
            visible(&output, label, SIZE);
        }
        press(&ctx, &mut app, visible(&output, action, SIZE));
        match action {
            "重新试演当前稿…" => {
                assert!(app.draft_rehearsal.has_pending());
                wait_prepared(&ctx, &mut app);
                click_label(&ctx, &mut app, "取消准备");
                assert!(!app.draft_rehearsal.has_pending());
                assert!(app.draft_rehearsal.active);
                assert_eq!(
                    app.draft_rehearsal
                        .running
                        .as_ref()
                        .unwrap()
                        .view
                        .inspection
                        .as_ref()
                        .unwrap()
                        .stamp,
                    stamp
                );
            }
            "保留试演，查看已应用稿" => {
                assert!(!app.draft_rehearsal.active);
                assert_eq!(
                    app.draft_rehearsal
                        .running
                        .as_ref()
                        .unwrap()
                        .view
                        .inspection
                        .as_ref()
                        .unwrap()
                        .stamp,
                    stamp
                );
            }
            _ => {
                assert!(!app.draft_rehearsal.active);
                assert!(app.draft_rehearsal.running.is_none());
            }
        }
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(
            app.manuscript.writing_buffers()[0].source(),
            buffer.source()
        );
        assert_eq!(
            app.play
                .as_ref()
                .unwrap()
                .story
                .as_ref()
                .unwrap()
                .save()
                .unwrap(),
            regular
        );
    }
}
