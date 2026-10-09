//! 完整App、实际2×chrome、真实鼠标滚轮/点击；不把未裁剪形状或子view当作通过。
use super::*;
use egui::{vec2, Event, RawInput, Rect};
const PHYSICAL: egui::Vec2 = egui::Vec2::new(800.0, 600.0);

fn app_frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(
            egui::Pos2::ZERO,
            PHYSICAL / ctx.zoom_factor(),
        )),
        events,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut input);
    ctx.run(input, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest())
    })
}
fn settle(ctx: &egui::Context, app: &mut WorldeditApp) -> egui::FullOutput {
    for _ in 0..6 {
        app_frame(ctx, app, vec![]);
    }
    app_frame(ctx, app, vec![])
}
fn texts<'a>(shape: &'a egui::Shape, result: &mut Vec<&'a egui::epaint::TextShape>) {
    match shape {
        egui::Shape::Text(text) => result.push(text),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                texts(shape, result);
            }
        }
        _ => {}
    }
}
fn visible(output: &egui::FullOutput, label: &str) -> Option<Rect> {
    for clipped in &output.shapes {
        let mut values = Vec::new();
        texts(&clipped.shape, &mut values);
        for text in values {
            if text.galley.text() == label {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                if clipped.clip_rect.contains_rect(rect) {
                    return Some(rect);
                }
            }
        }
    }
    None
}
fn visible_row(output: &egui::FullOutput, needle: &str) -> bool {
    output.shapes.iter().any(|clipped| {
        let mut values = Vec::new();
        texts(&clipped.shape, &mut values);
        values.iter().any(|text| {
            text.galley.rows.iter().any(|row| {
                let value: String = row.glyphs.iter().map(|glyph| glyph.chr).collect();
                value.contains(needle)
                    && clipped
                        .clip_rect
                        .contains_rect(row.rect().translate(text.pos.to_vec2()))
            })
        })
    })
}
fn viewport(ctx: &egui::Context, kind: &str) -> Rect {
    let rect = ctx
        .data(|data| data.get_temp::<Rect>(egui::Id::new(kind)))
        .expect("实际ScrollArea必须产生视口");
    assert!(
        rect.height() > 1.0 && rect.width() > 1.0,
        "正文视口不能被头部挤空：{rect:?}"
    );
    assert!(
        ctx.screen_rect().contains_rect(rect),
        "真实材料视口须在屏幕内：{rect:?}"
    );
    rect
}
fn wheel(ctx: &egui::Context, app: &mut WorldeditApp, kind: &str, delta: f32) -> egui::FullOutput {
    let position = viewport(ctx, kind).center();
    app_frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(position),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, delta),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    settle(ctx, app)
}
fn find_with_wheel(ctx: &egui::Context, app: &mut WorldeditApp, kind: &str, label: &str) -> Rect {
    let mut output = settle(ctx, app);
    for _ in 0..160 {
        if let Some(rect) = visible(&output, label) {
            return rect;
        }
        output = wheel(ctx, app, kind, -8.0);
    }
    panic!("真实滚轮后仍无法完整看见{label}");
}
fn click_rect(ctx: &egui::Context, app: &mut WorldeditApp, rect: Rect) -> egui::FullOutput {
    app_frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(rect.center()),
            Event::PointerButton {
                pos: rect.center(),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    app_frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos: rect.center(),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    )
}
fn key(ctx: &egui::Context, app: &mut WorldeditApp, key: egui::Key, modifiers: egui::Modifiers) {
    for pressed in [true, false] {
        app_frame(
            ctx,
            app,
            vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            }],
        );
    }
}
fn ready(scale: f32) -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = fixture();
    let path = app.project.entry.clone();
    let body = (0..12)
        .map(|number| format!("  正文可读{number:02}\n"))
        .collect::<String>();
    app.project
        .set_text(&path, format!("event start\n{body}  -> END\n"))
        .unwrap();
    let second = app
        .project
        .add_file(std::path::Path::new("second-batch.wl"))
        .unwrap();
    app.project
        .set_text(&second, format!("event second_batch\n{body}  -> END\n"))
        .unwrap();
    let entries: Vec<_> = (0..10)
        .map(|number| {
            serde_json::json!({
                "id":format!("chapter_{number}"),"kind":"chapter","title":format!("测试章{number}"),
                "status":"review","target_ref":{
                    "kind":"event","id":if number < 8 {"start"} else {"second_batch"}
                }
            })
        })
        .collect();
    let book = app.project.root.join(".world/manuscripts/book.json");
    app.project
        .set_authoring_document(
            &book,
            serde_json::to_vec(&serde_json::json!({
                "schema_version":1,"id":"book","title":"Book","entries":entries
            }))
            .unwrap(),
        )
        .unwrap();
    app.reset_views();
    app.recompile();
    app.tab = Tab::Manuscript;
    app.personal.settings.reduce_motion = true;
    app.personal.settings.ui_scale = scale;
    app.personal.settings.body_size = 16.0;
    app.manuscript.reader_open = true;
    app.manuscript.narrow_preview = true;
    settle(&ctx, &mut app);
    app.manuscript.navigation.session.status = "review".into();
    settle(&ctx, &mut app);
    generate(&mut app);
    settle(&ctx, &mut app);
    (ctx, app)
}

#[test]
fn full_app_small_review_reaches_real_body_batch_and_source_return_with_wheel() {
    for scale in [1.0, 2.0] {
        let (ctx, mut app) = ready(scale);
        assert_eq!(ctx.zoom_factor(), scale);
        assert_eq!(ctx.pixels_per_point(), scale);
        assert_eq!(ctx.screen_rect().size(), PHYSICAL / scale);
        let baseline = app.project.content_baseline();
        let version = app.version;
        let report = app
            .manuscript
            .preview_cache
            .delivery
            .reviewed
            .clone()
            .unwrap();
        let query = app.manuscript.preview_cache.query_snapshot.clone().unwrap();
        let second = app.project.root.join("second-batch.wl");
        assert!(app.manuscript.writing_buffer_mut(&second).is_none());
        assert!(app.manuscript_delivery_is_current());
        assert_eq!(
            report.scope().source,
            worldline_core::manuscript::ManuscriptQuerySource::Applied
        );
        assert!(report.scope().writing_inputs.is_empty());
        let output = settle(&ctx, &mut app);
        assert!(
            visible(&output, "查找对象").is_some(),
            "必须包含真实外层chrome"
        );
        if scale == 2.0 {
            assert!(visible(&output, "范围与交付").is_some());
        }
        let next = find_with_wheel(&ctx, &mut app, "delivery-review-viewport", "下一批范围章节");
        click_rect(&ctx, &mut app, next);
        settle(&ctx, &mut app);
        assert_eq!(
            app.manuscript.preview_cache.delivery.page, 8,
            "真实点击须换到第二批"
        );
        let before_scroll = app.manuscript.review_scroll_y;
        wheel(&ctx, &mut app, "delivery-review-viewport", -8.0);
        let body = find_with_wheel(&ctx, &mut app, "delivery-review-viewport", "正文可读00");
        assert!(ctx.screen_rect().contains_rect(body));
        assert!(
            app.manuscript.review_scroll_y > before_scroll,
            "滚轮须实际推进正文"
        );
        let source = find_with_wheel(&ctx, &mut app, "delivery-review-viewport", "定位原文");
        assert!(app.manuscript.writing_buffer_mut(&second).is_none());
        app.manuscript.preview_cache.delivery.confirmed = true;
        let before = app.manuscript_session();
        click_rect(&ctx, &mut app, source);
        let saved: crate::app::manuscript::ManuscriptSession =
            serde_json::from_value(app.personal.history.last().unwrap().manuscript.clone())
                .unwrap();
        let click_after = app.manuscript.review_scroll_y;
        // 来源在释放帧布局/滚动之后捕获；点击前的动画采样不是返回锚。
        let saved_scroll = saved.review_scroll_y.unwrap();
        assert_eq!(saved_scroll, click_after);
        assert!(saved_scroll > 0.0);
        assert_eq!(saved.review_page_offset, Some(8));
        settle(&ctx, &mut app);
        assert!(app.review_return_available());
        let buffer = app.manuscript.writing_buffer_mut(&second).unwrap();
        assert!(!buffer.is_changed());
        assert_eq!(buffer.generation(), 0);
        assert_eq!(buffer.source(), app.project.document(&second).unwrap());
        assert!(app.manuscript_delivery_is_current());
        key(&ctx, &mut app, egui::Key::ArrowLeft, egui::Modifiers::ALT);
        let return_first = app.manuscript.review_scroll_y;
        settle(&ctx, &mut app);
        assert_eq!(app.manuscript.review_page_offset, 8);
        assert_eq!(app.manuscript.preview_cache.delivery.page, 8);
        assert!(
            (return_first - saved_scroll).abs() < 1.0,
            "返回首帧须恢复真实点击锚：scale={scale}, saved={saved_scroll}, after={return_first}"
        );
        assert!(
            (app.manuscript.review_scroll_y - saved_scroll).abs() < 1.0,
            "无编辑往返须保留同一批的真实滚动位置：scale={scale}, saved={saved_scroll}, after={}",
            app.manuscript.review_scroll_y
        );
        assert_eq!(
            app.manuscript_session().preview_scoped,
            before.preview_scoped
        );
        assert_eq!(app.manuscript.navigation.session.status, "review");
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(app.version, version);
        assert!(Arc::ptr_eq(
            &query,
            app.manuscript
                .preview_cache
                .query_snapshot
                .as_ref()
                .unwrap()
        ));
        assert!(Arc::ptr_eq(
            &report,
            app.manuscript
                .preview_cache
                .delivery
                .reviewed
                .as_ref()
                .unwrap()
        ));
        assert!(app.manuscript_delivery_is_current());
        assert!(app.manuscript.preview_cache.delivery.confirmed);
        assert_eq!(
            app.checked_manuscript_markdown(&ctx).unwrap(),
            report.markdown().unwrap()
        );
        let changed = app
            .project
            .document(&second)
            .unwrap()
            .replace("正文可读00", "已改正文");
        app.manuscript
            .writing_buffer_mut(&second)
            .unwrap()
            .replace_source(changed);
        assert!(!app.manuscript_delivery_is_current());
        assert!(app.checked_manuscript_markdown(&ctx).is_err());
    }
}

#[test]
fn full_app_200_percent_delivery_menu_scroll_copy_and_markdown_body_are_reachable() {
    let (ctx, mut app) = ready(2.0);
    let expected = app
        .manuscript
        .preview_cache
        .delivery
        .reviewed
        .as_ref()
        .unwrap()
        .markdown()
        .unwrap()
        .to_owned();
    let baseline = app.project.content_baseline();
    let output = settle(&ctx, &mut app);
    let menu = visible(&output, "范围与交付").expect("菜单入口完整可见");
    click_rect(&ctx, &mut app, menu);
    settle(&ctx, &mut app);
    let privacy = find_with_wheel(
        &ctx,
        &mut app,
        "delivery-menu-viewport",
        "我已核对范围和Markdown；这是作者私密审稿材料",
    );
    click_rect(&ctx, &mut app, privacy);
    settle(&ctx, &mut app);
    assert!(app.manuscript.preview_cache.delivery.confirmed);
    let copy = find_with_wheel(&ctx, &mut app, "delivery-menu-viewport", "复制同一Markdown");
    let output = click_rect(&ctx, &mut app, copy);
    assert!(output.platform_output.commands.iter().any(
        |command| matches!(command, egui::OutputCommand::CopyText(value) if value == &expected)
    ));
    find_with_wheel(
        &ctx,
        &mut app,
        "delivery-menu-viewport",
        "导出新Markdown文件",
    );
    for _ in 0..12 {
        wheel(&ctx, &mut app, "delivery-menu-viewport", 160.0);
    }
    let markdown = find_with_wheel(
        &ctx,
        &mut app,
        "delivery-menu-viewport",
        "完整Markdown原文预览",
    );
    click_rect(&ctx, &mut app, markdown);
    key(&ctx, &mut app, egui::Key::Escape, egui::Modifiers::NONE);
    let mut output = settle(&ctx, &mut app);
    assert!(app.manuscript.preview_cache.delivery.markdown_open);
    let mut found = false;
    for _ in 0..180 {
        if visible_row(&output, "正文可读00") {
            found = true;
            break;
        }
        output = wheel(&ctx, &mut app, "delivery-markdown-viewport", -8.0);
    }
    assert!(
        found,
        "真实外层chrome和滚轮下须能看到Markdown正文行，不只头部信息"
    );
    assert_eq!(app.project.content_baseline(), baseline);
}
