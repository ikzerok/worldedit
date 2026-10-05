//! 长报告窄窗口中，通过真实Tab/Space/Enter完成范围确认与新文件交付。
use super::*;

const PRIVACY: &str = "我已核对预览、验证范围与私密内容，确认复制或保存此作者报告";
const DESTINATION_HINT: &str = "工作区外的绝对完整路径，以 .md 结尾";
const COPY: &str = "复制 Markdown";
const SAVE: &str = "保存新 Markdown 文件";

fn tab_to_id(ctx: &egui::Context, app: &mut WorldeditApp, id: egui::Id) {
    for _ in 0..128 {
        key(ctx, app, egui::Key::Tab, false);
        if ctx.memory(|memory| memory.focused()) == Some(id) {
            return;
        }
    }
    panic!("真实Tab没有到达指定报告控件{id:?}");
}

fn long_report_ready() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = ready();
    ctx.data_mut(|data| {
        data.insert_temp(egui::Id::new("report-test-size"), egui::vec2(1040.0, 660.0))
    });
    let source = SOURCE.replace(
        "  雾港的钟声。",
        &format!(
            "  雾港的钟声。\n{}",
            "  长篇逐行校对：雾气翻过旧堤岸，灯塔记下每一次潮汐。\n".repeat(160)
        ),
    );
    app.project
        .set_text(&app.active_file.clone(), source)
        .unwrap();
    app.recompile();
    let snapshot = &app.snapshot.as_ref().unwrap().result;
    let mut story = Story::new_with_seed(&snapshot.program, &snapshot.analysis, 31).unwrap();
    story.continue_story().unwrap();
    story.choose(0).unwrap();
    story.continue_story().unwrap();
    app.replay_debugger.saved_paths[0].trace = story.replay_trace();
    app.start_play();
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .continue_story()
        .unwrap();
    for _ in 0..3 {
        frame(&ctx, &mut app, Vec::new());
    }
    (ctx, app)
}

#[test]
fn keyboard_long_report_privacy_destination_copy_and_new_file_are_visible() {
    let (ctx, mut app) = long_report_ready();
    let before = invariant(&app);
    open_with_keyboard(&ctx, &mut app);
    let generate = tab_to(&ctx, &mut app, GENERATE);
    assert_visible(&ctx, &mut app, generate, GENERATE);
    key(&ctx, &mut app, egui::Key::Enter, false);
    let started = std::time::Instant::now();
    while app.playthrough_report.job.is_some() {
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
        frame(&ctx, &mut app, Vec::new());
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let markdown = app
        .playthrough_report
        .reviewed
        .as_ref()
        .expect("键盘生成报告")
        .report
        .markdown
        .clone();
    assert!(markdown.lines().count() > 150);
    assert!(!app.playthrough_report.privacy_confirmed);
    assert!(app.checked_report_markdown().is_err());

    // 聚焦巨大的只读正文时，外层仅滚入280px预览视口；空闲帧不继续追逐全文。
    let preview = egui::Id::new("playthrough-report-markdown");
    tab_to_id(&ctx, &mut app, preview);
    frame(&ctx, &mut app, Vec::new());
    frame(&ctx, &mut app, Vec::new());
    let preview_rect = ctx.read_response(preview).unwrap().rect;
    for _ in 0..4 {
        frame(&ctx, &mut app, Vec::new());
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(preview));
        assert_eq!(
            ctx.read_response(preview).unwrap().rect,
            preview_rect,
            "预览焦点不能使外层窗口来回滚动"
        );
    }

    let privacy = tab_to(&ctx, &mut app, PRIVACY);
    assert_visible(&ctx, &mut app, privacy, PRIVACY);
    assert!(!app.playthrough_report.privacy_confirmed);
    key(&ctx, &mut app, egui::Key::Space, false);
    assert!(app.playthrough_report.privacy_confirmed);
    let destination = egui::Id::new("playthrough-report-destination");
    tab_to_id(&ctx, &mut app, destination);
    assert_visible(&ctx, &mut app, destination, DESTINATION_HINT);
    let target = app.project.root.with_extension("review.md");
    assert!(!target.exists());
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Text(target.display().to_string())],
    );
    assert_eq!(
        app.playthrough_report.destination,
        target.display().to_string()
    );
    let browse = tab_to(&ctx, &mut app, "系统选择器（可选）");
    assert_visible(&ctx, &mut app, browse, "系统选择器（可选）");
    let copy = tab_to(&ctx, &mut app, COPY);
    assert_visible(&ctx, &mut app, copy, COPY);
    key(&ctx, &mut app, egui::Key::Enter, false);
    assert!(app
        .playthrough_report
        .notice
        .as_deref()
        .unwrap()
        .contains("已复制"));
    let save = tab_to(&ctx, &mut app, SAVE);
    assert_visible(&ctx, &mut app, save, SAVE);
    assert!(!target.exists(), "明确保存之前不能写文件");
    key(&ctx, &mut app, egui::Key::Enter, false);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), markdown);
    key(&ctx, &mut app, egui::Key::Tab, true);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(copy));
    assert_visible(&ctx, &mut app, copy, COPY);
    let close = tab_to(&ctx, &mut app, "关闭报告");
    assert_visible(&ctx, &mut app, close, "关闭报告");
    key(&ctx, &mut app, egui::Key::Enter, false);
    assert!(!app.playthrough_report.open);
    assert_eq!(invariant(&app), before);
    std::fs::remove_file(target).unwrap();
}
