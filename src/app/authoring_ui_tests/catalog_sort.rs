use super::catalog::{await_catalog_query, scroll_catalog_to};
use super::*;

fn prepare_sort_app(count: usize) -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app();
    let mut source = String::new();
    for index in 0..count {
        source.push_str(&format!(
            "entity e{index:03} kind place as \"资料{:03}\"\n",
            count - index
        ));
    }
    app.project
        .set_text(&app.project.entry.clone(), source)
        .unwrap();
    app.recompile();
    app.tab = Tab::Catalog;
    click(&ctx, &mut app, 14, "组合查询与待办");
    click(&ctx, &mut app, 14, "＋ 添加条件");
    click(&ctx, &mut app, 14, "对象类型");
    click(&ctx, &mut app, 14, "实体 · entity");
    (ctx, app)
}

// 排序表头先于异步查询结果更新；等待真实结果行，不能把表头出现当作查询完成。
fn sorted_rows(ctx: &egui::Context, app: &mut WorldeditApp, header: &str) -> String {
    let text = await_catalog_query(ctx, app, 14, header);
    assert!(
        text.contains(header)
            && text.contains("4 个命中")
            && text.contains("资料001")
            && text.contains("资料004"),
        "查询已结束，但排序或真实结果行不符合预期：{text}"
    );
    text
}

#[test]
fn catalog_sort_headers_toggle_and_navigation_follows_object_identity_without_writes() {
    let (ctx, mut app) = prepare_sort_app(4);
    let baseline = app.project.content_baseline();
    let sources = app.project.sources();
    click(&ctx, &mut app, 14, "运行查询");
    let _ = await_catalog_query(&ctx, &mut app, 14, "4 个命中");
    click(&ctx, &mut app, 14, "名称");
    let ascending = sorted_rows(&ctx, &mut app, "名称 ↑");
    assert!(ascending.find("资料001").unwrap() < ascending.find("资料004").unwrap());
    click(&ctx, &mut app, 14, "资料001");
    assert_eq!(app.entity_editor.as_ref().unwrap().draft.id, "e003");
    app.entity_editor = None;
    click(&ctx, &mut app, 14, "名称 ↑");
    let descending = sorted_rows(&ctx, &mut app, "名称 ↓");
    assert!(descending.find("资料004").unwrap() < descending.find("资料001").unwrap());
    click(&ctx, &mut app, 14, "资料001");
    assert_eq!(app.entity_editor.as_ref().unwrap().draft.id, "e003");
    app.entity_editor = None;
    click(&ctx, &mut app, 14, "排序：名称降序");
    let _ = frame(
        &ctx,
        &mut app,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        14,
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.sources(), sources);
    assert!(app.history.is_empty());
}

#[test]
fn catalog_sort_narrow_menu_restarts_pages_and_shared_definition_round_trips() {
    for window in [34, 35] {
        let (ctx, mut app) = prepare_sort_app(108);
        let baseline = app.project.content_baseline();
        let sources = app.project.sources();
        click(&ctx, &mut app, window, "运行查询");
        let _ = await_catalog_query(&ctx, &mut app, window, "108 个命中");
        for _ in 0..2 {
            scroll_catalog_to(&ctx, &mut app, window, "下一页");
            click(&ctx, &mut app, window, "下一页");
        }
        assert!(scroll_catalog_to(&ctx, &mut app, window, "显示 101–108").contains("显示 101–108"));
        scroll_catalog_to(&ctx, &mut app, window, "排序：默认顺序");
        click(&ctx, &mut app, window, "排序：默认顺序");
        click(&ctx, &mut app, window, "名称升序");
        let text = await_catalog_query(&ctx, &mut app, window, "显示 1–50");
        assert!(text.contains("显示 1–50"), "{text}");
        assert!(scroll_catalog_to(&ctx, &mut app, window, "资料001").contains("资料001"));
        assert_eq!(app.project.content_baseline(), baseline);
        assert!(app.history.is_empty());
        scroll_catalog_to(&ctx, &mut app, window, "保存为共享查询");
        click(&ctx, &mut app, window, "保存为共享查询");
        scroll_catalog_to(&ctx, &mut app, window, "稳定 ID");
        enter_text_at_placeholder_in_window(&ctx, &mut app, window, "稳定 ID", "sorted");
        scroll_catalog_to(&ctx, &mut app, window, "查询名称");
        enter_text_at_placeholder_in_window(&ctx, &mut app, window, "查询名称", "排序资料");
        scroll_catalog_to(&ctx, &mut app, window, "保存共享定义");
        click(&ctx, &mut app, window, "保存共享定义");
        assert_eq!(app.history.len(), 1);
        assert_eq!(
            app.project.saved_query_index().queries["sorted"]
                .draft
                .query
                .schema_version,
            2
        );
        app.project.save().unwrap();
        app.project = Project::open(&app.project.root).unwrap();
        app.reset_views();
        app.recompile();
        app.tab = Tab::Catalog;
        click(&ctx, &mut app, window, "组合查询与待办");
        click(&ctx, &mut app, window, "共享查询定义 · 1 项");
        scroll_catalog_to(&ctx, &mut app, window, "载入");
        click(&ctx, &mut app, window, "载入");
        let text = rendered_text_in_window(&ctx, &mut app, window, "排序：名称升序");
        assert!(text.contains("排序：名称升序"), "{text}");
        click(&ctx, &mut app, window, "排序：名称升序");
        click(&ctx, &mut app, window, "默认顺序");
        let _ = await_catalog_query(&ctx, &mut app, window, "显示 1–50");
        scroll_catalog_to(&ctx, &mut app, window, "保存共享定义");
        click(&ctx, &mut app, window, "保存共享定义");
        let draft = &app.project.saved_query_index().queries["sorted"].draft;
        assert_eq!(draft.query.schema_version, 1);
        assert!(draft.query.sort.is_none());
        assert_eq!(app.project.sources(), sources);
    }
}
