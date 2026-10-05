//! 真实报告入口Tab/Enter与窗口焦点交接；不注入焦点或直接启动任务。
use super::*;
use crate::app::Tab;

const ENTRY: &str = "试玩路径报告…";
const GENERATE: &str = "生成并预览已验证报告";
const SCOPE: &str = "我确认仅验证已应用稿，以上草稿仍保留且不进入报告";
const SIZE: egui::Vec2 = egui::vec2(1188.0, 848.0);

fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                ctx.data(|data| data.get_temp::<egui::Vec2>(egui::Id::new("report-test-size")))
                    .unwrap_or(SIZE),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            app.author_shortcuts(ctx);
            app.prepare_play_keyboard(ctx);
            app.poll_playthrough_report(ctx);
            app.play_tab(ctx);
            app.playthrough_report_window(ctx);
            app.capture_edit_focus(ctx);
        },
    )
}
fn key(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    key: egui::Key,
    shift: bool,
) -> Vec<egui::WidgetInfo> {
    let mut focused = Vec::new();
    for pressed in [true, false] {
        let output = frame(
            ctx,
            app,
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
        focused.extend(
            output
                .platform_output
                .events
                .into_iter()
                .filter_map(|event| {
                    if let egui::output::OutputEvent::FocusGained(info) = event {
                        Some(info)
                    } else {
                        None
                    }
                }),
        );
    }
    focused
}
fn tab_to(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) -> egui::Id {
    for _ in 0..128 {
        if key(ctx, app, egui::Key::Tab, false)
            .iter()
            .any(|info| info.label.as_deref() == Some(label))
        {
            return ctx.memory(|memory| memory.focused()).unwrap();
        }
    }
    panic!("真实Tab没有到达{label}");
}
fn assert_visible(ctx: &egui::Context, app: &mut WorldeditApp, id: egui::Id, label: &str) {
    // ScrollArea在pass末应用滚入；read_response交互矩形允许沿用上一pass。
    // 重开含预览的窗口后先完成这一帧，再核对稳定布局，期间焦点也必须保留。
    frame(ctx, app, Vec::new());
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    let output = frame(ctx, app, Vec::new());
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    let response = ctx.read_response(id).unwrap();
    fn matches(shape: &egui::Shape, clip: egui::Rect, response: egui::Rect, label: &str) -> bool {
        match shape {
            egui::Shape::Text(text) => {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                text.galley.job.text.contains(label)
                    && clip.contains(rect.center())
                    && response.contains(rect.center())
            }
            egui::Shape::Vec(shapes) => shapes
                .iter()
                .any(|shape| matches(shape, clip, response, label)),
            _ => false,
        }
    }
    assert!(
        output.shapes.iter().any(|shape| matches(
            &shape.shape,
            shape.clip_rect,
            response.rect,
            label
        )),
        "焦点控件不可见：{label} {:?}; route={:?}; reviewed={}; matched_text={:?}; shapes={}",
        response.rect,
        app.playthrough_report.route,
        app.playthrough_report.reviewed.is_some(),
        output
            .shapes
            .iter()
            .filter_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    text.galley.job.text.contains("进港").then(|| {
                        (
                            text.galley.job.text.clone(),
                            text.galley.rect.translate(text.pos.to_vec2()),
                            shape.clip_rect,
                        )
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>(),
        output.shapes.len()
    );
}
fn ready() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = setup();
    app.close_playthrough_report();
    app.tab = Tab::Play;
    for _ in 0..3 {
        frame(&ctx, &mut app, Vec::new());
    }
    (ctx, app)
}
fn open_with_keyboard(ctx: &egui::Context, app: &mut WorldeditApp) -> (egui::Id, egui::Id) {
    let entry = tab_to(ctx, app, ENTRY);
    assert_visible(ctx, app, entry, ENTRY);
    key(ctx, app, egui::Key::Enter, false);
    assert!(app.playthrough_report.open);
    let route = ctx.memory(|memory| memory.focused()).unwrap();
    assert_ne!(route, entry, "打开报告后焦点必须离开底层入口");
    assert_visible(ctx, app, route, "已录制：进港路线");
    assert!(app.playthrough_report.job.is_none());
    assert!(app.playthrough_report.reviewed.is_none());
    (entry, route)
}

#[test]
fn report_keyboard_entry_focus_tab_generate_escape_and_reopen() {
    let (ctx, mut app) = ready();
    let before = invariant(&app);
    let (entry, route) = open_with_keyboard(&ctx, &mut app);
    let generate = tab_to(&ctx, &mut app, GENERATE);
    assert_visible(&ctx, &mut app, generate, GENERATE);
    assert_ne!(generate, route);
    // 一次交接结束后，后续普通帧不能把新Tab焦点拉回入口控件。
    for _ in 0..3 {
        frame(&ctx, &mut app, Vec::new());
    }
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(generate));
    key(&ctx, &mut app, egui::Key::Enter, false);
    assert!(app.playthrough_report.job.is_some() || app.playthrough_report.reviewed.is_some());
    let start = std::time::Instant::now();
    while app.playthrough_report.job.is_some() {
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        frame(&ctx, &mut app, Vec::new());
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        app.playthrough_report.reviewed.is_some(),
        "{:?}",
        app.playthrough_report.notice
    );
    assert!(!app.playthrough_report.privacy_confirmed);
    assert!(app.checked_report_markdown().is_err());
    key(&ctx, &mut app, egui::Key::Escape, false);
    assert!(!app.playthrough_report.open);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(entry));
    key(&ctx, &mut app, egui::Key::Enter, false);
    assert!(app.playthrough_report.open);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(route));
    assert_visible(&ctx, &mut app, route, "已录制：进港路线");
    assert!(app.playthrough_report.job.is_none());
    assert!(!app.playthrough_report.privacy_confirmed);
    assert_eq!(invariant(&app), before);
}

#[test]
fn report_keyboard_entry_cannot_skip_unapplied_scope_confirmation() {
    let (ctx, mut app) = ready();
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    buffer.replace_source(format!("{SOURCE}\n// 保留未应用稿"));
    app.manuscript.restore_writing_buffers(&[buffer]);
    let before = invariant(&app);
    open_with_keyboard(&ctx, &mut app);
    let scope = tab_to(&ctx, &mut app, SCOPE);
    assert_visible(&ctx, &mut app, scope, SCOPE);
    assert!(!app.playthrough_report.scope_confirmed);
    assert!(app.playthrough_report.job.is_none());
    key(&ctx, &mut app, egui::Key::Enter, false);
    assert!(app.playthrough_report.scope_confirmed);
    let generate = tab_to(&ctx, &mut app, GENERATE);
    assert_visible(&ctx, &mut app, generate, GENERATE);
    key(&ctx, &mut app, egui::Key::Enter, false);
    assert!(app.playthrough_report.job.is_some() || app.playthrough_report.reviewed.is_some());
    key(&ctx, &mut app, egui::Key::Escape, false);
    assert!(!app.playthrough_report.open);
    assert!(app.playthrough_report.job.is_none());
    assert!(app.manuscript.writing_buffers()[0]
        .source()
        .contains("保留未应用稿"));
    assert_eq!(invariant(&app), before);
}

#[path = "keyboard_export_tests.rs"]
mod export;
