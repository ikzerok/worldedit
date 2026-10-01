use super::*;

fn full_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)),
            events,
            ..Default::default()
        },
        |ctx| {
            app.top_bar(ctx);
            app.status_bar(ctx);
            app.docked_reading(ctx);
            app.manuscript_tab(ctx);
        },
    )
}
fn contents(output: &egui::FullOutput) -> String {
    let mut text = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut text);
    }
    text
}

#[test]
fn current_draft_preview_tracks_valid_unapplied_text_and_marks_invalid_stale() {
    let (ctx, mut app) = manuscript_app();
    let baseline = app.project.content_baseline();
    frame(&ctx, &mut app, Vec::new(), 13);
    let path = app.active_file.clone();
    let buffer = app.manuscript.writing_buffer_mut(&path).unwrap();
    let valid = buffer.source().replace("甲乙", "即时草稿freshneedle");
    buffer.replace_source(valid.clone());
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let text = contents(&output);
    assert!(text.contains("当前稿 · 包含未应用输入"), "{text}");
    assert!(
        text.matches("即时草稿freshneedle").count() >= 2,
        "编辑和预览都要取当前稿：{text}"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source(valid.replace("  scene harbor", "  if (\n  scene harbor"));
    let text = contents(&frame(&ctx, &mut app, Vec::new(), 13));
    assert!(text.contains("预览过期"), "{text}");
    assert!(text.contains("上次有效预览"), "{text}");
    assert!(
        text.contains("即时草稿freshneedle"),
        "相同目标上次有效稿保留"
    );
    assert!(app.manuscript.writing_buffers()[0]
        .source()
        .contains("if ("));
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn current_chapter_is_default_and_whole_book_is_explicit() {
    let (ctx, mut app) = manuscript_app();
    let first = contents(&frame(&ctx, &mut app, Vec::new(), 13));
    assert!(!first.contains("远航。"), "当前章不应混入整书正文：{first}");
    click(&ctx, &mut app, 13, "整书");
    assert!(contents(&frame(&ctx, &mut app, Vec::new(), 13)).contains("远航。"));
    click(&ctx, &mut app, 13, "当前章节");
    click(&ctx, &mut app, 13, "离港");
    let text = contents(&frame(&ctx, &mut app, Vec::new(), 13));
    assert!(text.contains("远航。"));
    assert!(!text.contains("甲乙"), "切章后不可借用另一目标预览");
}

#[test]
fn focus_prioritizes_body_at_laptop_size_and_wide_preview_is_beside_it() {
    for body_size in [16.0, 28.0] {
        for light in [false, true] {
            let (ctx, mut app) = manuscript_app();
            app.personal.settings.focus = true;
            app.personal.settings.body_size = body_size;
            if light {
                ctx.set_visuals(egui::Visuals::light());
            } else {
                ctx.set_visuals(egui::Visuals::dark());
            }
            let size = vec2(1040.0, 660.0);
            let mut output = full_frame(&ctx, &mut app, size, Vec::new());
            for _ in 0..3 {
                output = full_frame(&ctx, &mut app, size, Vec::new());
            }
            let text = contents(&output);
            assert!(!text.contains("插入分节"), "专注隐藏编排管理");
            assert!(!text.contains("正文块可直接修改"), "专注渐进显示说明");
            let body = visible_text_position(&output, "甲乙").expect("笔记本视口正文可见");
            assert!(body.y < 260.0, "大字/主题下正文不能被管理挤下去: {body:?}");
            let wide = full_frame(&ctx, &mut app, vec2(1800.0, 900.0), Vec::new());
            let body = visible_text_position(&wide, "甲乙").unwrap();
            let preview = visible_text_position(&wide, "当前工程稿").expect("宽屏预览可见");
            assert!(preview.x > body.x + 500.0, "预览须在正文右侧而非下方");
            assert!(preview.y < 260.0);
        }
    }
}

#[test]
fn narrow_preview_tab_preserves_draft_and_returns_to_editor() {
    let (ctx, mut app) = manuscript_app();
    app.personal.settings.focus = true;
    let size = vec2(800.0, 600.0);
    full_frame(&ctx, &mut app, size, Vec::new());
    let path = app.active_file.clone();
    let buffer = app.manuscript.writing_buffer_mut(&path).unwrap();
    buffer.replace_source(buffer.source().replace("甲乙", "窄窗未应用稿"));
    for label in ["预览", "编辑"] {
        let output = full_frame(&ctx, &mut app, size, Vec::new());
        let position = visible_text_position(&output, label).unwrap();
        let events = vec![
            Event::PointerMoved(position),
            Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        full_frame(&ctx, &mut app, size, events);
        full_frame(
            &ctx,
            &mut app,
            size,
            vec![Event::PointerButton {
                pos: position,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        let text = contents(&full_frame(&ctx, &mut app, size, Vec::new()));
        assert!(text.contains("窄窗未应用稿"), "{label}: {text}");
    }
    assert!(app.manuscript.writing_buffers()[0].is_changed());
    assert!(app.history.is_empty());
}

#[test]
fn switching_prose_source_and_local_undo_never_restores_an_older_draft() {
    let (ctx, mut app) = manuscript_app();
    let original = app.project.document(&app.active_file).unwrap().to_owned();
    replace_manuscript_source(&ctx, &mut app, &original.replace("甲乙", "freshneedle"));
    click(&ctx, &mut app, 13, "写作");
    replace_text_area(
        &ctx,
        &mut app,
        13,
        "freshneedle [[character:traveler|林澈]]",
        "livepreview [[character:traveler|林澈]]",
    );
    click(&ctx, &mut app, 13, "源码");
    let current = app.manuscript.writing_buffers()[0].source().to_owned();
    assert!(current.contains("livepreview"));
    let invalid = format!("{current}\n  if (\n");
    replace_text_area(&ctx, &mut app, 13, "character traveler as", &invalid);
    let _ = frame(
        &ctx,
        &mut app,
        vec![Event::Key {
            key: egui::Key::Z,
            physical_key: Some(egui::Key::Z),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
        13,
    );
    assert_eq!(
        app.manuscript.writing_buffers()[0].source(),
        current,
        "局部撤销只能回到当前输入前的livepreview稿"
    );
    assert_eq!(app.project.document(&app.active_file).unwrap(), original);
}

#[test]
fn manuscript_preview_with_two_references_keeps_laptop_controls_and_draft_identity() {
    for size in [vec2(1040.0, 660.0), vec2(1800.0, 900.0)] {
        for font in [16.0, 28.0] {
            let (ctx, mut app) = manuscript_app();
            app.personal.settings.focus = true;
            app.personal.settings.body_size = font;
            app.reading_panels.pin(TargetRef::new("entity", "a"));
            app.reading_panels.pin(TargetRef::new("entity", "b"));
            let baseline = app.project.content_baseline();
            let mut output = full_frame(&ctx, &mut app, size, Vec::new());
            for _ in 0..3 {
                output = full_frame(&ctx, &mut app, size, Vec::new());
            }
            assert!(visible_text_position(&output, "固定参考").is_some());
            assert!(visible_text_position(&output, "写作").is_some());
            assert!(visible_text_position(&output, "甲乙").is_some());
            assert_eq!(app.reading_panels.ids().len(), 2);
            if size.x < 1200.0 {
                assert!(
                    visible_text_position(&output, "预览").is_some(),
                    "窄布局采用标签退化"
                );
            }
            let session = app.manuscript_session();
            app.personal.settings.references_visible = false;
            full_frame(&ctx, &mut app, size, Vec::new());
            assert_eq!(app.manuscript_session().selected_id, session.selected_id);
            assert_eq!(app.project.content_baseline(), baseline);
        }
    }
}
