use super::*;
use crate::theme::StylePreset;

#[test]
fn manuscript_is_centered_paper_and_technical_uses_the_available_plane() {
    let mut layouts = Vec::new();
    for style in [
        StylePreset::Studio,
        StylePreset::Manuscript,
        StylePreset::Technical,
    ] {
        let (ctx, mut app) = blank();
        app.personal.settings.style = style;
        app.personal.settings.reading_width = 600.0;
        create_start(&ctx, &mut app);
        app.manuscript.reader_open = false;
        let output = settle(&ctx, &mut app);
        let target = TargetRef::new("event", "start");
        let buffer = app
            .manuscript
            .writing_buffers
            .get(&app.project.entry)
            .unwrap();
        let slot = app
            .project
            .project_writing_buffer(buffer, &target)
            .unwrap()
            .empty_prose_slot
            .unwrap();
        let id = egui::Id::new((
            "writing-prose",
            buffer.path(),
            "event",
            "start",
            slot.offset(),
        ));
        let editor = ctx.read_response(id).expect("real prose TextEdit").rect;
        let tool = output
            .shapes
            .iter()
            .find_map(|shape| {
                let mut texts = Vec::new();
                text_shapes(&shape.shape, &mut texts);
                texts
                    .into_iter()
                    .find(|text| text.galley.text() == "写作")
                    .map(|text| text.pos)
            })
            .expect("writing mode control");
        assert!(
            tool.y < editor.top(),
            "tools must remain above the continuous document surface"
        );
        assert!(editor.right() <= 1600.0 && editor.width() > 200.0);
        let baseline = app.project.content_baseline();
        frame(
            &ctx,
            &mut app,
            vec![Event::Text("同一正文，不同工作台。".into())],
        );
        assert!(app
            .manuscript
            .writing_buffers
            .get(&app.project.entry)
            .unwrap()
            .source()
            .contains("同一正文，不同工作台。"));
        assert_eq!(app.project.content_baseline(), baseline);
        layouts.push(editor);
    }
    assert!(
        layouts[1].left() > layouts[0].left() + 40.0,
        "Manuscript must really center the writing paper: {layouts:?}"
    );
    assert!(
        layouts[2].width() > layouts[0].width() + 100.0,
        "Technical must use the full pane: {layouts:?}"
    );
    assert!(
        layouts[2].left() < layouts[0].left(),
        "Technical is edge aligned: {layouts:?}"
    );
}

#[test]
fn edit_plan_back_and_clear_cancel_preserve_all_values() {
    let (ctx, mut app) = blank();
    titles(&ctx, &mut app);
    click(&ctx, &mut app, "明确使用现有起点 start");
    click(&ctx, &mut app, "预览创建计划");
    click(&ctx, &mut app, "修改创建计划");
    click(&ctx, &mut app, "清空创建输入");
    click(&ctx, &mut app, "保留创建输入");
    let form = app.manuscript.creation.as_ref().unwrap();
    assert_eq!(form.book_title, "海边的灯");
    assert_eq!(form.chapter_title, "潮汐初起");
    assert_eq!(form.target, Some(TargetRef::new("event", "start")));
    assert!(!form.reviewing);
    assert!(app.project.manuscript_indices().is_empty());
    click(&ctx, &mut app, "清空创建输入");
    click(&ctx, &mut app, "确认清空创建输入");
    let form = app.manuscript.creation.as_ref().unwrap();
    assert!(!form.touched && form.book_title.is_empty() && form.chapter_title.is_empty());
}

#[test]
fn spaces_and_blank_lines_before_first_ime_text_keep_one_editor_and_source() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    let focused = ctx.memory(|memory| memory.focused()).unwrap();
    frame(&ctx, &mut app, vec![Event::Text(" ".into())]);
    key(&ctx, &mut app, egui::Key::Enter);
    key(&ctx, &mut app, egui::Key::Enter);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(focused));
    frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Enabled)]);
    frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Preedit("海".into()))],
    );
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(focused));
    frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Commit("海雾".into()))],
    );
    let source = app
        .manuscript
        .writing_buffers
        .get(&app.project.entry)
        .unwrap()
        .source();
    assert!(source.contains("   \n  \n  海雾\n  -> END"), "{source:?}");
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(focused));
    assert_eq!(
        app.project.document(&app.project.entry).unwrap(),
        "event start\n  -> END\n"
    );
}

#[test]
fn undo_creation_keeps_unapplied_source_visible_for_recovery() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    frame(
        &ctx,
        &mut app,
        vec![Event::Text("撤销后仍要保留的段落".into())],
    );
    app.undo(false);
    assert!(app.project.manuscript_indices().is_empty());
    assert!(app.manuscript.has_unsubmitted_work());
    assert!(labels(&settle(&ctx, &mut app)).contains("保留的正文草稿"));
    click(&ctx, &mut app, "world.wl");
    assert!(labels(&settle(&ctx, &mut app)).contains("撤销后仍要保留的段落"));
    click(&ctx, &mut app, "丢弃这份保留草稿");
    click(&ctx, &mut app, "继续保留草稿");
    assert!(app.manuscript.has_unsubmitted_work());
    click(&ctx, &mut app, "丢弃这份保留草稿");
    click(&ctx, &mut app, "确认丢弃保留草稿");
    assert!(!app.manuscript.has_unsubmitted_work());
}

#[test]
fn creation_plan_keeps_risks_visible_and_expands_complete_technical_identity() {
    let (ctx, mut app) = blank();
    titles(&ctx, &mut app);
    click(&ctx, &mut app, "新建空白正文");
    click(&ctx, &mut app, "预览创建计划");
    let form = app.manuscript.creation.as_ref().unwrap();
    let (request, plan) = form.preview.as_ref().unwrap();
    let original_request = serde_json::to_string(request).unwrap();
    let digest = plan.plan_digest.clone();
    let before = format!("{:016x}", plan.runtime_fingerprint_before);
    let after = format!("{:016x}", plan.runtime_fingerprint_after);
    let files = plan.changed_files.clone();
    let baseline = app.project.content_baseline();
    let text = labels(&settle(&ctx, &mut app));
    assert!(text.contains("核对新章节") && text.contains("海边的灯") && text.contains("潮汐初起"));
    assert!(text.contains("运行入口保持不变") && text.contains("新事件会改变运行指纹"));
    assert!(
        !text.contains(&before) && !text.contains(&after),
        "technical hashes start collapsed"
    );
    for path in files {
        assert!(text.contains(path.to_str().unwrap()));
    }
    click(&ctx, &mut app, "运行与稳定身份");
    let text = labels(&settle(&ctx, &mut app));
    assert!(text.contains(&before) && text.contains(&after));
    assert!(text.contains("书稿：manuscript:book · 章节：chapter"));
    assert!(text.contains("正文对象：event:chapter") && text.contains("分节：根目录"));
    let (request, plan) = app
        .manuscript
        .creation
        .as_ref()
        .unwrap()
        .preview
        .as_ref()
        .unwrap();
    assert_eq!(serde_json::to_string(request).unwrap(), original_request);
    assert_eq!(plan.plan_digest, digest);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}
