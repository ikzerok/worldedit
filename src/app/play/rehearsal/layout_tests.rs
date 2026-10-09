//! 首帧Area测量和真实可点击几何分别验证；不是物理窗口/200%系统缩放证明。
use super::*;
use egui::{pos2, vec2, Event, PointerButton, RawInput, Rect, Shape, Vec2};
#[path = "layout_app_tests.rs"]
mod full_app;

fn draw(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: Vec2,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| {
            app.author_shortcuts(ctx);
            if app.tab == Tab::Manuscript {
                app.manuscript_tab(ctx);
            } else {
                app.play_tab(ctx);
            }
            app.draft_rehearsal_dialog(ctx);
        },
    )
}
fn settled(ctx: &egui::Context, app: &mut WorldeditApp, size: Vec2) -> egui::FullOutput {
    // egui 0.32 Area::begin/content_ui明确把新area的首帧sizing pass设为invisible，
    // end时请求重绘；测量帧不能被当作最终可见窗口，也不能跳过后续几何断言。
    draw(ctx, app, size, vec![]);
    draw(ctx, app, size, vec![]);
    draw(ctx, app, size, vec![])
}
fn text_rects(shape: &Shape, label: &str, clip: Rect, out: &mut Vec<(Rect, Rect)>) {
    match shape {
        Shape::Text(text) if text.galley.job.text.contains(label) => {
            out.push((text.galley.rect.translate(text.pos.to_vec2()), clip));
        }
        Shape::Vec(shapes) => {
            for shape in shapes {
                text_rects(shape, label, clip, out);
            }
        }
        _ => {}
    }
}
fn visible(output: &egui::FullOutput, label: &str, size: Vec2) -> egui::Pos2 {
    let mut rects = Vec::new();
    for shape in &output.shapes {
        text_rects(&shape.shape, label, shape.clip_rect, &mut rects);
    }
    // 保留原测试三条文字存在断言，并额外要求完整文字落在屏幕与真实clip中。
    assert!(!rects.is_empty(), "缺少原要求的可见文字：{label}");
    let screen = Rect::from_min_size(egui::Pos2::ZERO, size);
    rects
        .iter()
        .find(|(rect, clip)| screen.contains_rect(*rect) && clip.contains_rect(*rect))
        .map(|(rect, _)| rect.center())
        .unwrap_or_else(|| panic!("{label} 未完整可见：{rects:?}，屏幕{screen:?}"))
}
fn click(ctx: &egui::Context, app: &mut WorldeditApp, size: Vec2, point: egui::Pos2) {
    for pressed in [true, false] {
        draw(
            ctx,
            app,
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

pub(super) fn check_dialog_and_rehearsal(size: Vec2, pixels_per_point: f32) {
    let (ctx, mut app) = app();
    ctx.set_pixels_per_point(pixels_per_point);
    app.new_file = Some("未提交的长文件名与源路径".repeat(25));
    let source = app.manuscript.writing_buffers()[0].source().to_owned();
    let baseline = app.project.content_baseline();
    app.request_draft_rehearsal(&ctx);
    wait_prepared(&ctx, &mut app);
    let first = draw(&ctx, &mut app, size, vec![]);
    let _ = first;
    assert!(app.draft_rehearsal.pending.is_some());
    assert!(
        app.draft_rehearsal.running.is_none(),
        "测量帧不能偷偷开始故事"
    );
    let output = settled(&ctx, &mut app, size);
    visible(&output, "明确开始这份草稿试演", size);
    let cancel = visible(&output, "取消准备", size);
    click(&ctx, &mut app, size, cancel);
    assert!(
        app.draft_rehearsal.pending.is_none(),
        "窄窗取消必须真正可点击"
    );
    assert_eq!(app.manuscript.writing_buffers()[0].source(), source);
    assert_eq!(app.project.content_baseline(), baseline);
    app.request_draft_rehearsal(&ctx);
    wait_prepared(&ctx, &mut app);
    let output = settled(&ctx, &mut app, size);
    let start = visible(&output, "明确开始这份草稿试演", size);
    click(&ctx, &mut app, size, start);
    assert!(app.draft_rehearsal.pending.is_none());
    assert!(app.draft_rehearsal.active, "必须实际启动同一准备快照");
    wait_idle(&ctx, &mut app);
    let mut output = settled(&ctx, &mut app, size);
    visible(&output, "返回原草稿位置", size);
    if size.x < 720.0 || size.y < 360.0 {
        let menu = visible(&output, "试演操作", size);
        click(&ctx, &mut app, size, menu);
        output = settled(&ctx, &mut app, size);
        for label in ["重新试演当前稿…", "保留试演，查看已应用稿", "关闭试演"]
        {
            visible(&output, label, size);
        }
        click(&ctx, &mut app, size, menu);
        output = settled(&ctx, &mut app, size);
    } else {
        visible(&output, "关闭试演", size);
    }
    if size.y < 400.0 {
        // 在很短的正文滚动区用真实滚轮抵达正文/选择，不把屏幕外shape当可读。
        for _ in 0..3 {
            draw(
                &ctx,
                &mut app,
                size,
                vec![
                    Event::PointerMoved(pos2(size.x / 2.0, size.y - 24.0)),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, -260.0),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            output = settled(&ctx, &mut app, size);
        }
    }
    visible(&output, "未应用真实正文", size);
    visible(&output, "选择：继续", size);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.manuscript.writing_buffers()[0].source(), source);
}

#[test]
fn compact_400_by_300_logical_dialog_keeps_start_cancel_and_rehearsal_reachable() {
    check_dialog_and_rehearsal(vec2(400.0, 300.0), 1.0);
}
#[test]
fn two_times_pixel_ratio_with_400_by_300_logical_size_keeps_actual_controls_reachable() {
    check_dialog_and_rehearsal(vec2(400.0, 300.0), 2.0);
}
