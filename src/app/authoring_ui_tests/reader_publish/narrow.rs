use super::*;

#[test]
fn reader_directory_narrow_long_titles_keep_query_pager_return_and_cancel_visible() {
    let (ctx, mut app) = super::directory::directory_app();
    let source = app.project.document(&app.active_file).unwrap().replace(
        "同名公开页 ÉCLAIR",
        &format!("同名公开页 ÉCLAIR {}", "可换行的长标题".repeat(18)),
    );
    app.project
        .set_text(&app.active_file.clone(), source)
        .unwrap();
    app.recompile();
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);
    for _ in 0..4 {
        narrow_reader_frame(&ctx, &mut app, vec![]);
    }
    narrow_reader_click(&ctx, &mut app, "页面目录 / 查找");
    for _ in 0..4 {
        narrow_reader_frame(&ctx, &mut app, vec![]);
    }
    let output = narrow_reader_frame(&ctx, &mut app, vec![]);
    for label in [
        "查找公开页面",
        "下一组结果",
        "目录第 1 / 3 页 · 每页至多 12 项",
        "取消发布",
    ] {
        assert!(
            narrow_reader_point(&output, label).is_some(),
            "fixed directory control must remain visible: {label}"
        );
    }
    narrow_reader_click(&ctx, &mut app, "下一组结果");
    narrow_reader_click(&ctx, &mut app, "打开此页");
    let output = narrow_reader_frame(&ctx, &mut app, vec![]);
    assert!(narrow_reader_point(&output, "返回页面目录").is_some());
    narrow_reader_click(&ctx, &mut app, "返回页面目录");
    let output = narrow_reader_frame(&ctx, &mut app, vec![]);
    assert!(narrow_reader_point(&output, "目录第 2 / 3 页 · 每页至多 12 项").is_some());
    narrow_reader_click(&ctx, &mut app, "取消发布");
    assert!(!app.reader_publish.open);
}

#[test]
fn reader_publish_narrow_viewport_keeps_preview_and_cancel_reachable() {
    let (ctx, mut app) = reader_publish_app();
    let path = app.active_file.clone();
    let text = format!("character doctor as \"医生\"\n  property appearance = \"蓝外套\"\n  property secret = \"窄窗秘密\"\n{}", app.project.document(&path).unwrap());
    app.project.set_text(&path, text).unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    let history = app.history.len();
    let _ = narrow_reader_frame(&ctx, &mut app, Vec::new());
    app.open_reader_publish();
    // Retain a previously larger window size, then shrink the actual native viewport.
    let _ = frame(&ctx, &mut app, Vec::new(), 26);
    for _ in 0..4 {
        narrow_reader_frame(&ctx, &mut app, Vec::new());
    }
    let bounds = ctx
        .memory(|memory| memory.area_rect(egui::Id::new("reader-publish-window")))
        .unwrap();
    assert!(
        bounds.bottom() <= 660.0 && bounds.top() >= 0.0,
        "window must fit native viewport: {bounds:?}"
    );
    narrow_reader_click(&ctx, &mut app, "医生 · character (doctor)");
    narrow_reader_click(&ctx, &mut app, "生成 / 更新预览");
    let mut preview_ready = false;
    for attempt in 0..500 {
        let output = narrow_reader_frame(&ctx, &mut app, Vec::new());
        if output
            .shapes
            .iter()
            .any(|shape| text_position(&shape.shape, "作者只读预览").is_some())
        {
            preview_ready = true;
            break;
        }
        if attempt % 20 == 0 {
            let bounds = ctx
                .memory(|memory| memory.area_rect(egui::Id::new("reader-publish-window")))
                .unwrap();
            let hover = pos2(bounds.right() - 48.0, bounds.center().y);
            narrow_reader_frame(
                &ctx,
                &mut app,
                vec![
                    Event::PointerMoved(hover),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, if attempt < 250 { -120.0 } else { 120.0 }),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(preview_ready, "the actual preview worker must complete");
    narrow_reader_click(&ctx, &mut app, "4 确认生成");
    // This is intentionally disabled until confirmation; its geometry must still be reachable.
    narrow_reader_click(&ctx, &mut app, "发布 ZIP");
    narrow_reader_click(&ctx, &mut app, "取消发布");
    assert!(
        !app.reader_publish.open,
        "取消 must be an actual reachable click"
    );
    assert_eq!(baseline, app.project.content_baseline());
    assert_eq!(history, app.history.len());
}

fn narrow_reader_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    let mut native_frame = eframe::Frame::_new_kittest();
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1040.0, 660.0))),
            events,
            ..Default::default()
        },
        |ctx| eframe::App::update(app, ctx, &mut native_frame),
    )
}
fn narrow_reader_point(output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
    let viewport = Rect::from_min_size(pos2(0.0, 0.0), vec2(1040.0, 660.0));
    output.shapes.iter().find_map(|shape| {
        text_position(&shape.shape, label)
            .filter(|point| viewport.contains(*point) && shape.clip_rect.contains(*point))
    })
}
fn narrow_reader_click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let mut point = None;
    for attempt in 0..70 {
        let output = narrow_reader_frame(ctx, app, Vec::new());
        if let Some(found) = narrow_reader_point(&output, label) {
            point = Some(found);
            break;
        }
        let bounds = ctx
            .memory(|memory| memory.area_rect(egui::Id::new("reader-publish-window")))
            .unwrap();
        // 目录的搜索和分页固定在结果上方；滚轮必须命中真实结果 clip，
        // 窗口中心可能仍在审核摘要里，不能假定它就是可滚动正文。
        let hover = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text
                        .galley
                        .job
                        .text
                        .lines()
                        .any(|line| line.ends_with(".html"))
                        && shape.clip_rect.intersects(bounds) =>
                {
                    Some(shape.clip_rect.intersect(bounds).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| pos2(bounds.right() - 48.0, bounds.center().y));
        narrow_reader_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(hover),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, if attempt < 35 { -120.0 } else { 120.0 }),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    let point = point.unwrap_or_else(|| panic!("1040×660 viewport cannot reach {label}"));
    for pressed in [true, false] {
        narrow_reader_frame(
            ctx,
            app,
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
