//! 检查真正章标题widget的绘制、裁剪和点击，正文同名文字不能满足断言。
use super::navigation_input::{gesture, native_frame, native_settle, setup};
use super::*;

type Geometry = (u64, egui::Id, egui::Rect, egui::Rect);
pub(super) fn geometry(ctx: &egui::Context, entry: &str) -> Geometry {
    let value = ctx
        .data(|data| {
            data.get_temp::<Geometry>(egui::Id::new(("manuscript-row-geometry", "book", entry)))
        })
        .expect("当前页必须真实绘制章标题widget");
    let frame = ctx.cumulative_frame_nr();
    let context =
        ctx.data(|data| data.get_temp::<String>(egui::Id::new("navigation-matrix-context")));
    assert!(
        value.0.saturating_add(1) >= frame,
        "不能用上一页遗留几何证明当前按钮：entry={entry} record={} current={frame} {context:?}",
        value.0
    );
    assert!(
        value.2.intersect(value.3).is_positive(),
        "章标题必须与自己的可见clip相交：{value:?}"
    );
    let response = ctx
        .read_response(value.1)
        .expect("标题widget应有真实交互响应");
    assert_eq!(response.rect, value.2);
    value
}
fn assert_painted_title(output: &egui::FullOutput, rect: egui::Rect) {
    assert!(
        output.shapes.iter().any(|shape| {
            let mut glyphs = Vec::new();
            text_shapes(&shape.shape, &mut glyphs);
            glyphs.iter().any(|text| {
                text.galley.job.text.contains("特别长章名")
                    && (text
                        .galley
                        .rect
                        .translate(text.pos.to_vec2())
                        .intersect(shape.clip_rect)
                        .intersect(rect))
                    .is_positive()
            })
        }),
        "标题区域必须画出章名文字，不能用正文同名标题代替"
    );
}

#[test]
fn list_title_widgets_stay_visible_clickable_in_narrow_columns_and_horizontal_scroll() {
    let (ctx, mut app) = setup();
    let path = app.project.root.join(".world/manuscripts/book.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&path).unwrap().bytes()).unwrap();
    value["entries"][0]["title"] = serde_json::json!("特别长章名与真实身份".repeat(12));
    value["entries"][0]["summary"] = serde_json::json!("足够长的作者摘要".repeat(24));
    app.project
        .set_authoring_document(&path, serde_json::to_vec(&value).unwrap())
        .unwrap();
    app.project.save().unwrap();
    app.recompile();
    app.manuscript.navigation.session.columns.identity = true;
    app.manuscript.navigation.session.columns.source = true;
    for width in [1400.0, 960.0] {
        let size = vec2(width, 1000.0);
        app.manuscript.layout = Layout::List;
        let output = native_settle(&ctx, &mut app, size);
        let (_, _, rect, clip) = geometry(&ctx, "first");
        assert!(clip.width() <= 220.0, "测试应真实覆盖窄标题列");
        assert_painted_title(&output, rect.intersect(clip));
        let original_left = rect.left();
        let pointer = pos2(clip.right() + 24.0, rect.center().y);
        native_frame(
            &ctx,
            &mut app,
            size,
            vec![
                Event::PointerMoved(pointer),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(-320.0, 0.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        for _ in 0..10 {
            native_frame(&ctx, &mut app, size, vec![]);
        }
        let offset = ctx
            .data(|data| {
                data.get_temp::<egui::Vec2>(egui::Id::new(("manuscript-list-scroll", "book")))
            })
            .unwrap();
        assert!(
            offset.x > 1.0,
            "必须实际滚动其他列，不能只修改DTO或复用首帧"
        );
        let output = native_settle(&ctx, &mut app, size);
        let (_, _, rect, clip) = geometry(&ctx, "first");
        assert!(
            (rect.left() - original_left).abs() < 2.0,
            "章标题首列应固定，不随其他列移出视口"
        );
        assert_painted_title(&output, rect.intersect(clip));
        app.manuscript.books.get_mut("book").unwrap().selected_entry = Some("second".into());
        gesture(&ctx, &mut app, size, rect.intersect(clip).center());
        assert_eq!(
            app.manuscript.books["book"].selected_entry.as_deref(),
            Some("first")
        );
        app.manuscript.layout = Layout::Tree;
        native_settle(&ctx, &mut app, size);
        geometry(&ctx, "first");
        app.manuscript.layout = Layout::List;
        let output = native_settle(&ctx, &mut app, size);
        let (_, _, rect, clip) = geometry(&ctx, "first");
        assert_painted_title(&output, rect.intersect(clip));
    }
    std::fs::remove_dir_all(&app.project.root).unwrap();
}
