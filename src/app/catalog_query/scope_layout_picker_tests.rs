//! 完整检查器须在独立滚动页使用真实对象选择器，不能把 ComboBox 放进操作 popup。
use super::*;

#[test]
fn full_app_scope_inspector_picker_search_selects_real_center_without_losing_scope() {
    let (ctx, mut app) = ready(true);
    let baseline = app.project.content_baseline();
    let snapshot = app.catalog_workbench.snapshot.clone().unwrap();
    let target = TargetRef::new("entity", "neighbor");
    let candidate = crate::app::object_picker::candidate_caption(
        app.snapshot
            .as_ref()
            .unwrap()
            .result
            .analysis
            .catalog
            .object(&target)
            .unwrap(),
        Some(&app.project.root),
    );
    click(&ctx, &mut app, "关系操作");
    operations_page(&ctx, &mut app);
    click_material(&ctx, &mut app, "完整网络检查器");
    settle(&ctx, &mut app);
    assert!(!egui::Popup::is_any_open(&ctx), "检查器是独立正文页");
    assert_eq!(
        ctx.data(|data| data.get_temp::<u8>(egui::Id::new("catalog-scope-network-pane"))),
        Some(4)
    );
    let pointer = clip(&ctx, "catalog-scope-material-clip").center();
    let picker = scroll_find(&ctx, &mut app, "港口 · entity:harbor", pointer);
    press(&ctx, &mut app, picker);
    let output = settle(&ctx, &mut app);
    visible(&output, "搜索名称、类型、ID或来源");
    assert!(egui::Popup::is_any_open(&ctx));
    frame(&ctx, &mut app, vec![Event::Text("neighbor".into())]);
    let output = settle(&ctx, &mut app);
    exact_visible(&output, "neighbor");
    let choice = visible(&output, &candidate);
    press(&ctx, &mut app, choice);
    settle(&ctx, &mut app);
    assert_eq!(app.network_state.focus, Some(target));
    assert!(!egui::Popup::is_any_open(&ctx));
    assert!(app.query_scope_active());
    assert!(std::sync::Arc::ptr_eq(
        app.catalog_workbench.snapshot.as_ref().unwrap(),
        &snapshot
    ));
    click(&ctx, &mut app, "返回关系图");
    let output = settle(&ctx, &mut app);
    clip(&ctx, "catalog-scope-network-clip");
    visible(&output, "邻港");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}
