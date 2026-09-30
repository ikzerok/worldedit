//! 工作台布局回归，确保收紧导航后仍保留文件入口与创作动作。
use super::{app, frame, text_position_contains, visible_text_position, Tab};

#[test]
fn compact_sidebar_keeps_files_and_primary_actions_visible() {
    let (ctx, mut app) = app();
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 32);
    }
    let output = frame(&ctx, &mut app, Vec::new(), 32);
    for label in ["工程文件", "保存全部", "发布给读者", "时间线", "书稿工作台"]
    {
        let point = visible_text_position(&output, label)
            .unwrap_or_else(|| panic!("窄窗缺少可见入口：{label}"));
        assert!(
            point.x >= 0.0 && point.x < 800.0 && point.y < 600.0,
            "{label}: {point:?}"
        );
    }
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
