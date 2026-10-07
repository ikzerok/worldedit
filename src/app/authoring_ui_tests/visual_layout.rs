//! 工作台布局回归，确保收紧导航后仍保留文件入口与创作动作。
use super::{app, click, frame, text_position_contains, visible_text_position, Tab};

#[test]
fn compact_sidebar_keeps_files_and_primary_actions_visible() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 32);
    }
    let output = frame(&ctx, &mut app, Vec::new(), 32);
    for label in [
        "工程文件",
        "保存全部",
        "导出与发布",
        "世界资料",
        "书稿工作台",
    ] {
        let point = visible_text_position(&output, label)
            .unwrap_or_else(|| panic!("窄窗缺少可见入口：{label}"));
        assert!(
            point.x >= 0.0 && point.x < 800.0 && point.y < 600.0,
            "{label}: {point:?}"
        );
    }
    click(&ctx, &mut app, 32, "导出与发布");
    let menu = frame(&ctx, &mut app, Vec::new(), 32);
    assert!(visible_text_position(&menu, "发布给读者").is_some());
    assert!(visible_text_position(&menu, "导出工程  ↗").is_some());
    click(&ctx, &mut app, 32, "导出与发布");
    // 新建作品默认在书稿，写作组默认展开；更多组可滚动而不是挤掉文件入口。
    click(&ctx, &mut app, 32, "世界资料");
    let scroll = egui::pos2(100.0, 280.0);
    let _ = frame(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(scroll),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -700.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
        32,
    );
    for _ in 0..8 {
        let _ = frame(&ctx, &mut app, Vec::new(), 32);
    }
    let scrolled = frame(&ctx, &mut app, Vec::new(), 32);
    assert!(
        visible_text_position(&scrolled, "结构与审阅").is_some(),
        "展开的世界资料后可通过真实滚轮找到结构组"
    );
    assert!(
        visible_text_position(&scrolled, "工程文件").is_some(),
        "导航滚动不挤掉文件入口"
    );
    click(&ctx, &mut app, 32, "结构与审阅");
    // 刚展开的子项在滚动区域底部；继续真实滚轮，不点击不可见galley坐标。
    let _ = frame(
        &ctx,
        &mut app,
        vec![
            egui::Event::PointerMoved(scroll),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -160.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
        32,
    );
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 32);
    }
    let expanded = frame(&ctx, &mut app, Vec::new(), 32);
    let point = visible_text_position(&expanded, "时间线").expect("展开后时间线必须可见");
    assert!(point.x < 800.0 && point.y < 600.0);
    click(&ctx, &mut app, 32, "时间线");
    assert_eq!(app.tab, Tab::Timeline);
    assert_eq!(
        app.project.content_baseline(),
        baseline,
        "仅导航不能修改作品"
    );
}

#[test]
fn catalog_heading_precedes_actions_in_narrow_work_area() {
    let (ctx, mut app) = app();
    app.tab = Tab::Catalog;
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 35);
    }
    let output = frame(&ctx, &mut app, Vec::new(), 35);
    let heading = output
        .shapes
        .iter()
        .filter_map(|shape| text_position_contains(&shape.shape, "资料与状态"))
        .find(|point| point.x > 220.0)
        .expect("内容区应有独立标题");
    let action = visible_text_position(&output, "组合查询与待办").unwrap();
    assert!(heading.y < action.y, "标题必须独立于操作工具栏");
}
