use super::*;

fn footer_top(ctx: &egui::Context) -> f32 {
    egui::containers::panel::PanelState::load(ctx, egui::Id::new("catalog-import-actions"))
        .expect("fixed import footer exists")
        .rect
        .top()
}
fn assert_details_above_footer(output: &egui::FullOutput, footer: f32) {
    for shape in &output.shapes {
        if let egui::Shape::Text(text) = &shape.shape {
            if text.galley.job.text.starts_with("原值：")
                || text.galley.job.text.starts_with("新值：")
                || text.galley.job.text.starts_with("property.")
                || text.galley.job.text.contains("行字段详情")
            {
                assert!(
                    shape.clip_rect.bottom() <= footer + 0.5,
                    "field paints into fixed footer: text={:?} clip={:?} footer={footer}",
                    text.galley.job.text,
                    shape.clip_rect
                );
            }
        }
    }
}

#[test]
fn expanded_source_header_keeps_all_detail_scrolling_above_fixed_footer() {
    let (ctx, mut app) = app();
    let csv = CSV.replace("远行旅人", &"长中文显示值保持完整".repeat(40));
    load(&ctx, &mut app, &csv);
    map(&mut app);
    preview(&ctx, &mut app);
    app.catalog_import.source_name = "/tmp/worldedit-验证目录/资料.csv".into();
    app.catalog_import.native_path = app.catalog_import.source_name.clone();
    click(&ctx, &mut app, "按完整文件路径读取 CSV（备用）");
    let size = egui::vec2(1188.0, 848.0);
    for _ in 0..24 {
        frame(&ctx, &mut app, size, vec![]);
    }
    let footer = footer_top(&ctx);
    let mut reached_last_value = false;
    for _ in 0..30 {
        let output = frame(&ctx, &mut app, size, vec![]);
        assert_details_above_footer(&output, footer);
        if output.shapes.iter().any(|shape| {
            point(&shape.shape, "新值：21")
                .is_some_and(|p| shape.clip_rect.contains(p) && p.y < footer)
        }) {
            reached_last_value = true;
            break;
        }
        frame(
            &ctx,
            &mut app,
            size,
            vec![
                Event::PointerMoved(egui::pos2(1050.0, footer - 16.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -90.0),
                    modifiers: Default::default(),
                },
            ],
        );
    }
    assert!(
        reached_last_value,
        "完整长字段后的数值差异仍须通过详情滚动到达"
    );
    assert_eq!(
        app.catalog_import.plan.as_ref().unwrap().rows[0]
            .fields
            .len(),
        2
    );
    // Both expanded header and narrow/short layouts must keep the exact same
    // field data without letting paint escape into the confirmation controls.
    click(&ctx, &mut app, "快照与写入边界");
    for size in [
        egui::vec2(1188.0, 848.0),
        egui::vec2(760.0, 720.0),
        egui::vec2(1188.0, 620.0),
    ] {
        for _ in 0..24 {
            frame(&ctx, &mut app, size, vec![]);
        }
        let output = frame(&ctx, &mut app, size, vec![]);
        assert_details_above_footer(&output, footer_top(&ctx));
        for label in ["已审阅整批字段差异与存档影响", "确认整批应用", "刷新预览"]
        {
            assert!(
                output
                    .shapes
                    .iter()
                    .any(|shape| point(&shape.shape, label)
                        .is_some_and(|p| shape.clip_rect.contains(p))),
                "固定操作不可见：{label}"
            );
        }
        assert!(app.history.is_empty());
    }
}
