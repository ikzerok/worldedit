use super::*;

#[test]
fn catalog_query_opens_from_the_existing_catalog_and_keeps_browsing_read_only() {
    let (ctx, mut app) = app();
    app.tab = super::Tab::Catalog;
    let baseline = app.project.content_baseline();
    let sources = app.project.sources().clone();

    click(&ctx, &mut app, 14, "组合查询与待办");
    let output = frame(&ctx, &mut app, Vec::new(), 14);

    assert!(output
        .shapes
        .iter()
        .any(|shape| { text_position(&shape.shape, "筛选条件").is_some() }));
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.sources(), sources);
    assert!(app.history.is_empty());
}

#[test]
fn catalog_query_composes_core_filters_shows_reasons_and_removes_a_condition() {
    let (ctx, mut app) = app();
    app.tab = super::Tab::Catalog;
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 14, "组合查询与待办");
    click(&ctx, &mut app, 14, "＋ 添加条件");
    click(&ctx, &mut app, 14, "对象类型");
    click(&ctx, &mut app, 14, "实体 · entity");
    click(&ctx, &mut app, 14, "＋ 添加条件");
    click(&ctx, &mut app, 14, "名称 / ID / 别名");
    enter_text_at_placeholder_in_window(&ctx, &mut app, 14, "输入名称、ID 或别名", "同名");
    click(&ctx, &mut app, 14, "添加名称值");
    click(&ctx, &mut app, 14, "运行查询");
    let _ = rendered_text_in_window(&ctx, &mut app, 14, "2 个命中");
    click(&ctx, &mut app, 14, "命中原因");
    let rendered = rendered_text_in_window(&ctx, &mut app, 14, "命中：类型");

    assert!(rendered.contains("2 个命中"), "{rendered}");
    assert!(rendered.contains("命中：类型"), "{rendered}");
    assert!(rendered.contains("命中：名称/别名"), "{rendered}");
    assert!(rendered.contains("名称/别名：「同名」"), "{rendered}");
    assert!(rendered.contains("同名"), "{rendered}");
    assert!(rendered.contains("实体"), "{rendered}");
    assert!(rendered.contains("名称"), "{rendered}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());

    click(&ctx, &mut app, 14, "删除名称 / ID / 别名条件");
    let after_remove = rendered_text_in_window(&ctx, &mut app, 14, "筛选条件");
    assert!(!after_remove.contains("2 个命中"), "{after_remove}");
    assert!(after_remove.contains("类型"), "{after_remove}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn catalog_query_reports_an_empty_result_without_changing_the_project() {
    let (ctx, mut app) = app();
    app.tab = super::Tab::Catalog;
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 14, "组合查询与待办");
    click(&ctx, &mut app, 14, "＋ 添加条件");
    click(&ctx, &mut app, 14, "名称 / ID / 别名");
    enter_text_at_placeholder_in_window(&ctx, &mut app, 14, "输入名称、ID 或别名", "不存在的资料");
    click(&ctx, &mut app, 14, "添加名称值");
    click(&ctx, &mut app, 14, "运行查询");
    let rendered = rendered_text_in_window(&ctx, &mut app, 14, "没有找到匹配资料");

    assert!(rendered.contains("没有找到匹配资料"), "{rendered}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn catalog_query_uses_core_pages_and_rejects_a_result_from_an_old_project_baseline() {
    let (ctx, mut app) = app();
    app.tab = super::Tab::Catalog;
    let entry = app.project.entry.clone();
    let mut source = app.project.sources()[&entry].clone();
    for index in 0..55 {
        source.push_str(&format!(
            "entity object_{index} kind place as \"对象 {index}\"\n"
        ));
    }
    app.project.set_text(&entry, source).unwrap();
    app.recompile();

    click(&ctx, &mut app, 14, "组合查询与待办");
    click(&ctx, &mut app, 14, "＋ 添加条件");
    click(&ctx, &mut app, 14, "对象类型");
    click(&ctx, &mut app, 14, "实体 · entity");
    click(&ctx, &mut app, 14, "运行查询");
    let first_page = rendered_text_in_window(&ctx, &mut app, 14, "57 个命中");
    assert!(first_page.contains("显示 1–50"), "{first_page}");
    assert!(first_page.contains("下一页"), "{first_page}");

    let mut changed_source = app.project.sources()[&entry].clone();
    changed_source.push_str("entity object_new kind place as \"新对象\"\n");
    app.project.set_text(&entry, changed_source).unwrap();
    app.recompile();
    let stale_page = rendered_text_in_window(&ctx, &mut app, 14, "结果已过期");
    assert!(stale_page.contains("结果已过期"), "{stale_page}");
    assert!(!stale_page.contains("下一页"), "{stale_page}");

    click(&ctx, &mut app, 14, "从第一页重新查询");
    let refreshed = rendered_text_in_window(&ctx, &mut app, 14, "58 个命中");
    assert!(refreshed.contains("58 个命中"), "{refreshed}");
    assert!(refreshed.contains("显示 1–50"), "{refreshed}");
}

#[test]
fn catalog_query_saves_shared_definitions_and_keeps_local_favorites_out_of_the_project() {
    let (ctx, mut app) = app();
    app.tab = super::Tab::Catalog;
    let root = app.project.root.clone();
    let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
    click(&ctx, &mut app, 14, "组合查询与待办");
    click(&ctx, &mut app, 14, "＋ 添加条件");
    click(&ctx, &mut app, 14, "对象类型");
    click(&ctx, &mut app, 14, "实体 · entity");
    click(&ctx, &mut app, 14, "保存为共享查询");
    enter_text_at_placeholder_in_window(&ctx, &mut app, 14, "稳定 ID", "people");
    enter_text_at_placeholder_in_window(&ctx, &mut app, 14, "查询名称", "所有资料");
    click(&ctx, &mut app, 14, "保存共享定义");

    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert!(app
        .project
        .saved_query_index()
        .queries
        .contains_key("people"));
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
    assert_eq!(app.history.len(), 1);
    click(&ctx, &mut app, 14, "共享查询定义 · 1 项");
    click(&ctx, &mut app, 14, "☆ 收藏到本机");
    let baseline = app.project.content_baseline();
    assert_eq!(app.history.len(), 1);

    app.project.save().unwrap();
    app.project = Project::open(&root).unwrap();
    app.reset_views();
    app.recompile();
    app.tab = super::Tab::Catalog;
    click(&ctx, &mut app, 14, "组合查询与待办");
    click(&ctx, &mut app, 14, "共享查询定义 · 1 项");
    let output = frame(&ctx, &mut app, Vec::new(), 14);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("所有资料 · people"), "{rendered}");
    click(&ctx, &mut app, 14, "本地收藏 · 1 项");
    let favorites = frame(&ctx, &mut app, Vec::new(), 14);
    let mut favorite_text = String::new();
    for shape in &favorites.shapes {
        collect_text(&shape.shape, &mut favorite_text);
    }
    assert!(
        favorite_text.contains("所有资料 · people"),
        "{favorite_text}"
    );
    if !favorite_text.contains("载入") {
        click(&ctx, &mut app, 14, "共享查询定义 · 1 项");
    }
    click(&ctx, &mut app, 14, "载入");
    assert_eq!(app.project.content_baseline(), baseline);
    click(&ctx, &mut app, 14, "运行查询");
    let reopened_query = rendered_text_in_window(&ctx, &mut app, 14, "2 个命中");
    assert!(reopened_query.contains("2 个命中"), "{reopened_query}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.history.len(), 0);
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
}

#[test]
fn catalog_todo_groups_core_items_and_jumps_to_the_exact_source_without_writing() {
    let (ctx, mut app) = app();
    app.tab = super::Tab::Catalog;
    let entry = app.project.entry.clone();
    app.project
        .set_text(
            &entry,
            concat!(
                "entity keepers kind organization as \"守灯会\"\n",
                "event start\n",
                "  参见 [[entity:missing_place|失落地点]] 与 [[entity:missing_place|另一地点]]。\n",
                "  -> END\n",
            )
            .into(),
        )
        .unwrap();
    let manifest = app.project.root.join(".world/project.json");
    app.project
        .set_authoring_document(
            &manifest,
            br#"{"schema_version":1,"language_version":"1.10","required_features":["content.entities.v1","collaboration.comments.v1","collaboration.proposals.v1"],"maps":{},"graph_views":{},"comments":{"detached":".world/comments/detached.json","resolved":".world/comments/resolved.json"},"proposals":{"open":".world/proposals/open.json","accepted":".world/proposals/accepted.json"}}"#.to_vec(),
        )
        .unwrap();
    app.project
        .create_authoring_document(
            &app.project.root.join(".world/comments/detached.json"),
            r#"{"schema_version":1,"id":"detached","author":"甲","body":"检查失落地点","anchor":{"kind":"object","target":{"kind":"entity","id":"missing_place"}},"resolved":false}"#.as_bytes().to_vec(),
        )
        .unwrap();
    app.project
        .create_authoring_document(
            &app.project.root.join(".world/comments/resolved.json"),
            r#"{"schema_version":1,"id":"resolved","author":"甲","body":"已处理","anchor":{"kind":"object","target":{"kind":"entity","id":"missing_place"}},"resolved":true}"#.as_bytes().to_vec(),
        )
        .unwrap();
    for (id, status) in [("open", "open"), ("accepted", "accepted")] {
        app.project
            .create_authoring_document(
                &app.project.root.join(format!(".world/proposals/{id}.json")),
                format!(
                    r#"{{"schema_version":1,"id":"{id}","author":"甲","reason":"审阅改动","status":"{status}","changes":[{{"path":"world.wl","domain":"content","base":"旧文本","proposed":"新文本"}}]}}"#
                )
                .into_bytes(),
            )
            .unwrap();
    }
    app.recompile();
    let baseline = app.project.content_baseline();
    let sources = app.project.sources();
    click(&ctx, &mut app, 14, "组合查询与待办");
    click(&ctx, &mut app, 14, "统一待办");
    let rendered = rendered_text_in_window(&ctx, &mut app, 14, "待审提案 · 1 项");

    for label in [
        "断链 · 2 项",
        "待建资料 · 1 项",
        "失锚批注 · 1 项",
        "待审提案 · 1 项",
    ] {
        assert!(rendered.contains(label), "missing {label}: {rendered}");
    }
    assert!(!rendered.contains("accepted"), "{rendered}");
    assert!(!rendered.contains("resolved"), "{rendered}");
    assert!(rendered.contains("world.wl:3"), "{rendered}");

    click(&ctx, &mut app, 14, "定位断链来源");
    assert_eq!(app.tab, super::Tab::Edit);
    assert_eq!(app.active_file, entry);
    let (line, column) = app.jump.unwrap();
    assert_eq!(line, 3);
    assert!(column > 0);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.sources(), sources);
    assert!(app.history.is_empty());
}

pub(super) fn scroll_catalog_to(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    label: &str,
) -> String {
    let mut rendered = String::new();
    for delta in [-90.0, 90.0] {
        for _ in 0..250 {
            let output = frame(ctx, app, Vec::new(), window);
            rendered.clear();
            for shape in &output.shapes {
                collect_text(&shape.shape, &mut rendered);
            }
            if visible_text_position(&output, label).is_some() {
                return rendered;
            }
            // 从结果区域滚动到底，继续滚动必须能到达外层操作区。
            let point = ctx.screen_rect().center() + vec2(100.0, 60.0);
            let _ = frame(
                ctx,
                app,
                vec![
                    Event::PointerMoved(point),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, delta),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                window,
            );
        }
    }
    panic!("组合查询控件滚动后仍不可达：{label}；{rendered}");
}

#[test]
fn small_catalog_viewport_can_page_save_and_favorite_after_scrolling() {
    for window in [34, 35] {
        let (ctx, mut app) = app();
        app.tab = super::Tab::Catalog;
        let source = (0..68)
            .map(|index| format!("entity object_{index:02} kind place as \"对象 {index:02}\"\n"))
            .collect::<String>();
        app.project
            .set_text(&app.project.entry.clone(), source)
            .unwrap();
        app.recompile();
        let baseline = app.project.content_baseline();
        let sources = app.project.sources();
        let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
        click(&ctx, &mut app, 34, "组合查询与待办");
        click(&ctx, &mut app, 34, "＋ 添加条件");
        click(&ctx, &mut app, 34, "对象类型");
        click(&ctx, &mut app, 34, "实体 · entity");
        click(&ctx, &mut app, 34, "运行查询");
        let first = rendered_text_in_window(&ctx, &mut app, window, "68 个命中");
        assert!(first.contains("显示 1–50"), "{first}");
        scroll_catalog_to(&ctx, &mut app, window, "下一页");
        click(&ctx, &mut app, window, "下一页");
        let second = scroll_catalog_to(&ctx, &mut app, window, "显示 51–68");
        assert!(second.contains("68 个命中"), "{second}");
        scroll_catalog_to(&ctx, &mut app, window, "上一页");
        click(&ctx, &mut app, window, "上一页");
        let first_again = scroll_catalog_to(&ctx, &mut app, window, "显示 1–50");
        assert!(first_again.contains("68 个命中"), "{first_again}");
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(app.project.sources(), sources);
        assert!(app.history.is_empty());

        scroll_catalog_to(&ctx, &mut app, window, "保存为共享查询");
        click(&ctx, &mut app, window, "保存为共享查询");
        scroll_catalog_to(&ctx, &mut app, window, "稳定 ID");
        enter_text_at_placeholder_in_window(&ctx, &mut app, window, "稳定 ID", "paged");
        scroll_catalog_to(&ctx, &mut app, window, "查询名称");
        enter_text_at_placeholder_in_window(&ctx, &mut app, window, "查询名称", "分页资料");
        scroll_catalog_to(&ctx, &mut app, window, "保存共享定义");
        click(&ctx, &mut app, window, "保存共享定义");
        assert!(
            app.project
                .saved_query_index()
                .queries
                .contains_key("paged"),
            "{:?}",
            app.io_error
        );
        assert_eq!(app.history.len(), 1);
        assert_eq!(app.project.sources(), sources);
        assert_eq!(
            app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
            fingerprint
        );
        let saved_baseline = app.project.content_baseline();
        scroll_catalog_to(&ctx, &mut app, window, "共享查询定义 · 1 项");
        click(&ctx, &mut app, window, "共享查询定义 · 1 项");
        scroll_catalog_to(&ctx, &mut app, window, "☆ 收藏到本机");
        click(&ctx, &mut app, window, "☆ 收藏到本机");
        scroll_catalog_to(&ctx, &mut app, window, "本地收藏 · 1 项");
        assert_eq!(app.project.content_baseline(), saved_baseline);
        assert_eq!(app.history.len(), 1);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_app_does_not_persist_editor_memory_with_favorites() {
    let (_, app) = app();
    assert!(!eframe::App::persist_egui_memory(&app));
}
