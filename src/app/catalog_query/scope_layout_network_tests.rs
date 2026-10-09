//! 独立操作页复用原ComboBox/筛选；关闭真实popup后保留页面与已选值。
use super::*;

#[test]
fn full_app_scope_network_200_percent_keeps_graph_materials_context_and_controls_reachable() {
    let (ctx, mut app) = ready(true);
    let baseline = app.project.content_baseline();
    let output = settle(&ctx, &mut app);
    for label in [
        "worldedit",
        "保存全部",
        "范围操作",
        "关系图",
        "范围列表",
        "关系上下文",
        "关系操作",
        "港口",
    ] {
        visible(&output, label);
    }
    clip(&ctx, "catalog-scope-network-clip");
    click(&ctx, &mut app, "关系操作");
    operations_page(&ctx, &mut app);
    click_material(&ctx, &mut app, "1 层");
    click(&ctx, &mut app, "2 层");
    assert_eq!(app.network_state.filters.depth, 2);
    operations_page(&ctx, &mut app);
    assert_eq!(app.network_state.result.as_ref().unwrap().nodes.len(), 3);
    click_material(&ctx, &mut app, "2 层");
    escape_menu(&ctx, &mut app);
    assert_eq!(app.network_state.filters.depth, 2);
    operations_page(&ctx, &mut app);
    click_material(&ctx, &mut app, "2 层");
    dismiss_outside(&ctx, &mut app);
    assert_eq!(app.network_state.filters.depth, 2);
    operations_page(&ctx, &mut app);
    // 返回再进入同一操作页，保留值且所有原控件仍实际可用。
    click(&ctx, &mut app, "返回关系图");
    clip(&ctx, "catalog-scope-network-clip");
    click(&ctx, &mut app, "关系操作");
    operations_page(&ctx, &mut app);
    click_material(&ctx, &mut app, "双向读取");
    click(&ctx, &mut app, "只看入向");
    assert_eq!(
        app.network_state.filters.direction,
        worldline_core::RelationQueryDirection::Incoming
    );
    operations_page(&ctx, &mut app);
    assert_eq!(app.network_state.result.as_ref().unwrap().nodes.len(), 1);
    click_material(&ctx, &mut app, "只看入向");
    click(&ctx, &mut app, "只看出向");
    assert_eq!(
        app.network_state.filters.direction,
        worldline_core::RelationQueryDirection::Outgoing
    );
    operations_page(&ctx, &mut app);
    assert_eq!(app.network_state.result.as_ref().unwrap().nodes.len(), 3);
    let pointer = clip(&ctx, "catalog-scope-material-clip").center();
    scroll_find(&ctx, &mut app, "上一页", pointer);
    scroll_find(&ctx, &mut app, "下一页", pointer);
    click_material(&ctx, &mut app, "关系类型筛选");
    click(&ctx, &mut app, "道路 · route");
    assert_eq!(
        app.network_state.filters.relation_types,
        vec!["route".to_owned()]
    );
    operations_page(&ctx, &mut app);
    click_material(&ctx, &mut app, "关系类型筛选");
    escape_menu(&ctx, &mut app);
    assert_eq!(
        app.network_state.filters.relation_types,
        vec!["route".to_owned()]
    );
    operations_page(&ctx, &mut app);
    click_material(&ctx, &mut app, "关系类型筛选");
    dismiss_outside(&ctx, &mut app);
    assert_eq!(
        app.network_state.filters.relation_types,
        vec!["route".to_owned()]
    );
    operations_page(&ctx, &mut app);
    click_material(&ctx, &mut app, "关系类型筛选");
    click(&ctx, &mut app, "清除筛选");
    assert!(app.network_state.filters.relation_types.is_empty());
    operations_page(&ctx, &mut app);
    click(&ctx, &mut app, "返回关系图");
    click(&ctx, &mut app, "范围列表");
    settle(&ctx, &mut app);
    let pointer = clip(&ctx, "catalog-scope-material-clip").center();
    scroll_find(&ctx, &mut app, "编辑真实资料", pointer);
    scroll_find(&ctx, &mut app, "定位 港口地图 / port", pointer);
    click(&ctx, &mut app, "关系上下文");
    settle(&ctx, &mut app);
    let pointer = clip(&ctx, "catalog-scope-material-clip").center();
    scroll_find(&ctx, &mut app, "邻港 · 仅关系上下文", pointer);
    let source = scroll_find(&ctx, &mut app, "打开来源", pointer);
    press(&ctx, &mut app, source);
    assert_eq!(app.tab, Tab::Edit);
    assert!(app.query_scope_active());
    scope_menu(&ctx, &mut app);
    click(&ctx, &mut app, "返回巡检位置");
    assert_eq!(app.tab, Tab::Network);
    click(&ctx, &mut app, "关系图");
    settle(&ctx, &mut app);
    clip(&ctx, "catalog-scope-network-clip");
    scope_menu(&ctx, &mut app);
    click(&ctx, &mut app, "清除范围并恢复");
    assert_eq!(app.tab, Tab::Catalog);
    assert!(!app.query_scope_active());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}
