//! 用真实临时工程和审核后台走完整公开目录路径；没有公共样例工程。
use super::*;

pub(super) fn directory_app() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app();
    let mut source = String::new();
    for index in 0..26 {
        source.push_str(&format!(
            "event e{index:02} as \"同名公开页 ÉCLAIR\"\n  公开正文编号 {index:02} MiXeD。\n  -> END\n"
        ));
    }
    source.push_str("character empty as \"同名公开页 空白\"\n  property secret = \"HIDDEN_FIELD_SENTINEL\"\nevent hidden as \"HIDDEN_TITLE_SENTINEL\"\n  HIDDEN_BODY_SENTINEL\n  -> END\n");
    app.project
        .set_text(&app.active_file.clone(), source)
        .unwrap();
    app.project.save().unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    app.open_reader_publish();
    enter_text_at_placeholder_in_window(
        &ctx,
        &mut app,
        26,
        "搜索名称、别名、类型或 ID",
        "同名公开页",
    );
    click_reader_body(&ctx, &mut app, "选择全部筛选资料");
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);
    click(&ctx, &mut app, 26, "页面目录 / 查找");
    (ctx, app)
}

fn text(ctx: &egui::Context, app: &mut WorldeditApp) -> String {
    let output = frame(ctx, app, vec![], 26);
    let mut text = String::new();
    for shape in output.shapes {
        collect_text(&shape.shape, &mut text);
    }
    text
}

fn query(ctx: &egui::Context, app: &mut WorldeditApp, previous: &str, query: &str) {
    if previous.is_empty() {
        enter_text_at_placeholder_in_window(ctx, app, 26, "查找公开页面", query);
    } else {
        replace_text_area(ctx, app, 26, previous, query);
    }
}

#[test]
fn reader_directory_actual_public_projection_excludes_secrets_and_returns_to_filtered_page() {
    let (ctx, mut app) = directory_app();
    let baseline = app.project.content_baseline();
    let version = app.version;
    let history = app.history.len();
    let disk = std::fs::read(&app.active_file).unwrap();
    let selected = Some(TargetRef::new("event", "e00"));
    app.reading_target = selected.clone();
    let mut draft = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    draft.replace_source(draft.source().replace("编号 00", "保留未应用的稿件"));
    let saved_draft = draft.clone();
    app.manuscript.restore_writing_buffers(&[draft]);
    assert!(text(&ctx, &mut app).contains("公开页总数 30 · 当前匹配 30"));
    query(&ctx, &mut app, "", "HIDDEN");
    let output = text(&ctx, &mut app);
    assert!(output.contains("公开页总数 30 · 当前匹配 0"), "{output}");
    assert!(output.contains("当前公开页中没有匹配内容"));
    assert!(!output.contains("HIDDEN_FIELD_SENTINEL"));
    assert!(!output.contains("HIDDEN_BODY_SENTINEL"));
    query(&ctx, &mut app, "HIDDEN", "空白");
    assert!(text(&ctx, &mut app).contains("空正文"));
    click_reader_body(&ctx, &mut app, "打开此页");
    assert!(text(&ctx, &mut app).contains("此页没有静态阅读正文。"));
    click(&ctx, &mut app, 26, "返回页面目录");
    query(&ctx, &mut app, "空白", "éclair");
    assert!(text(&ctx, &mut app).contains("公开页总数 30 · 当前匹配 28"));
    click(&ctx, &mut app, 26, "下一组结果");
    assert!(text(&ctx, &mut app).contains("目录第 2 / 3 页"));
    click_reader_body(&ctx, &mut app, "打开此页");
    assert!(text(&ctx, &mut app).contains("公开正文编号 12"));
    click(&ctx, &mut app, 26, "返回页面目录");
    let output = text(&ctx, &mut app);
    assert!(output.contains("éclair"));
    assert!(output.contains("目录第 2 / 3 页"));
    query(&ctx, &mut app, "éclair", "编号 12");
    assert!(text(&ctx, &mut app).contains("当前匹配 1"));
    click_reader_body(&ctx, &mut app, "打开此页");
    click(&ctx, &mut app, 26, "下一阅读页");
    assert!(
        text(&ctx, &mut app).contains("公开正文编号 13"),
        "unfiltered reader order must survive a one-result query"
    );
    assert_eq!(baseline, app.project.content_baseline());
    assert_eq!(version, app.version, "query/navigation must not recompile");
    assert_eq!(history, app.history.len());
    assert_eq!(disk, std::fs::read(&app.active_file).unwrap());
    assert_eq!(app.reading_target, selected);
    let buffers = app.manuscript.writing_buffers();
    assert_eq!(buffers.len(), 1);
    assert_eq!(buffers[0].source(), saved_draft.source());
    assert_eq!(buffers[0].baseline(), saved_draft.baseline());
    assert_eq!(buffers[0].generation(), saved_draft.generation());
}

#[test]
fn reader_directory_new_preview_source_change_cancel_and_project_reset_clear_old_query() {
    let (ctx, mut app) = directory_app();
    query(&ctx, &mut app, "", "编号 25");
    click(&ctx, &mut app, 26, "1 选择内容");
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);
    click(&ctx, &mut app, 26, "页面目录 / 查找");
    assert!(text(&ctx, &mut app).contains("公开页总数 30 · 当前匹配 30"));
    query(&ctx, &mut app, "", "编号 24");
    // 源码尚未重新编译也必须使已审核包失效。
    let changed = app
        .project
        .document(&app.active_file)
        .unwrap()
        .replace("编号 24", "已更新的公开正文");
    app.project
        .set_text(&app.active_file.clone(), changed)
        .unwrap();
    let output = text(&ctx, &mut app);
    assert!(output.contains("公开页审核已过期"), "{output}");
    assert!(!output.contains("当前匹配"));
    click(&ctx, &mut app, 26, "取消发布");
    app.recompile();
    app.open_reader_publish();
    click(&ctx, &mut app, 26, "生成 / 更新预览");
    wait_for_reader_publish(&ctx, &mut app);
    click(&ctx, &mut app, 26, "页面目录 / 查找");
    assert!(text(&ctx, &mut app).contains("公开页总数 30 · 当前匹配 30"));
    query(&ctx, &mut app, "", "已更新的公开正文");
    assert!(text(&ctx, &mut app).contains("当前匹配 1"));
    assert!(app.cancel_reader_publish());
    app.reset_views();
    app.open_reader_publish();
    assert!(!text(&ctx, &mut app).contains("当前匹配"));
}
