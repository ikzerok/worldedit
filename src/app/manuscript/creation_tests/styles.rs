use super::*;
use crate::theme::StylePreset;

fn style_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    width: f32,
    events: Vec<Event>,
    preferences: bool,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 1500.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            let _theme = crate::theme::configure_appearance(ctx, app.personal.appearance());
            app.manuscript_tab(ctx);
            if preferences {
                app.preferences_window(ctx);
            }
        },
    )
}

fn style_settle(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    width: f32,
    preferences: bool,
) -> egui::FullOutput {
    for _ in 0..4 {
        style_frame(ctx, app, width, vec![], preferences);
    }
    style_frame(ctx, app, width, vec![], preferences)
}

fn preference_click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let output = style_settle(ctx, app, 1600.0, true);
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| {
            let mut texts = Vec::new();
            text_shapes(&shape.shape, &mut texts);
            texts
                .into_iter()
                .find(|text| text.galley.text() == label)
                .map(|text| text.pos + text.galley.rect.center().to_vec2())
        })
        .unwrap_or_else(|| panic!("missing preference {label}: {}", labels(&output)));
    for pressed in [true, false] {
        style_frame(
            ctx,
            app,
            1600.0,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            true,
        );
    }
}

#[test]
fn focus_is_single_area_and_recalls_chapters_management_creation_and_preview() {
    let (ctx, mut app) = blank();
    retention::two_chapters(&ctx, &mut app, false);
    app.personal.settings.style = StylePreset::Focus;
    app.personal.settings.focus = false;
    app.manuscript.reader_open = true;
    app.manuscript.narrow_preview = false;
    let baseline = app.project.content_baseline();
    let output = labels(&settle(&ctx, &mut app));
    for action in [
        "选择章节",
        "新建章节",
        "书稿管理",
        "阅读预览",
        "编辑",
        "预览",
    ] {
        assert!(
            output.lines().any(|line| line == action),
            "missing {action}: {output}"
        );
    }
    assert!(
        !output.contains("全分支审稿"),
        "wide Focus must not force parallel preview"
    );
    click(&ctx, &mut app, "选择章节");
    click(&ctx, &mut app, "潮汐初起");
    assert_eq!(
        app.manuscript_session().selected_id.as_deref(),
        Some("chapter")
    );
    frame(
        &ctx,
        &mut app,
        vec![Event::Text("Focus 中保留的正文".into())],
    );
    let draft = app.manuscript.writing_buffers()[0].source().to_owned();
    assert!(draft.contains("Focus 中保留的正文"));
    click(&ctx, &mut app, "预览");
    let output = labels(&settle(&ctx, &mut app));
    assert!(output.contains("全分支审稿") && output.contains("Focus 中保留的正文"));
    assert!(
        !output.lines().any(|line| line == "写作"),
        "preview and editor are not both rendered"
    );
    click(&ctx, &mut app, "编辑");
    click(&ctx, &mut app, "书稿管理");
    assert!(labels(&settle(&ctx, &mut app)).contains("插入分节"));
    click(&ctx, &mut app, "新建章节");
    assert_eq!(
        app.manuscript
            .creation
            .as_ref()
            .unwrap()
            .existing_book
            .as_deref(),
        Some("book")
    );
    click(&ctx, &mut app, "返回，保留输入");
    assert_eq!(app.manuscript.writing_buffers()[0].source(), draft);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(!app.personal.settings.focus);
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn focus_appearance_preview_and_cancel_do_not_rewrite_reading_or_focus_preferences() {
    for reader_open in [false, true] {
        for narrow_preview in [false, true] {
            let (ctx, mut app) = blank();
            create_start(&ctx, &mut app);
            app.personal.settings.focus = false;
            app.manuscript.reader_open = reader_open;
            app.manuscript.narrow_preview = narrow_preview;
            let baseline = app.project.content_baseline();
            app.personal.preferences_open = true;
            preference_click(&ctx, &mut app, "Focus 专注");
            style_settle(&ctx, &mut app, 1600.0, true);
            assert!(app.focus_style());
            assert_eq!(app.personal.settings.style, StylePreset::Studio);
            assert_eq!(
                (app.manuscript.reader_open, app.manuscript.narrow_preview),
                (reader_open, narrow_preview)
            );
            assert!(!app.personal.settings.focus);
            preference_click(&ctx, &mut app, "取消");
            style_settle(&ctx, &mut app, 1600.0, true);
            assert!(!app.focus_style());
            assert_eq!(app.personal.appearance().style, StylePreset::Studio);
            assert_eq!(
                (app.manuscript.reader_open, app.manuscript.narrow_preview),
                (reader_open, narrow_preview)
            );
            assert!(!app.personal.settings.focus);
            assert_eq!(app.project.content_baseline(), baseline);
        }
    }
}

fn prose_rect(ctx: &egui::Context, app: &WorldeditApp) -> egui::Rect {
    let (target, path) = app.manuscript.active_writing_target().unwrap();
    let buffer = app.manuscript.writing_buffers.get(&path).unwrap();
    let projection = app.project.project_writing_buffer(buffer, &target).unwrap();
    let offset = projection
        .empty_prose_slot
        .as_ref()
        .map(|slot| slot.offset())
        .unwrap_or_else(|| {
            projection
                .blocks
                .iter()
                .find(|block| block.kind == worldline_core::manuscript::WritingBlockKind::Prose)
                .unwrap()
                .range
                .start
        });
    ctx.read_response(egui::Id::new((
        "writing-prose",
        &path,
        &target.kind,
        &target.id,
        offset,
    )))
    .unwrap()
    .rect
}

fn title_rects(output: &egui::FullOutput) -> Vec<egui::Rect> {
    output
        .shapes
        .iter()
        .flat_map(|shape| {
            let mut texts = Vec::new();
            text_shapes(&shape.shape, &mut texts);
            texts
                .into_iter()
                .filter(|text| text.galley.text() == "潮汐初起")
                .map(|text| text.galley.rect.translate(text.pos.to_vec2()))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn ledger_chapter_label_moves_above_narrow_prose_without_duplicate_document_title() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    app.personal.settings.style = StylePreset::Ledger;
    app.manuscript.reader_open = false;
    let baseline = app.project.content_baseline();
    let wide = style_settle(&ctx, &mut app, 1600.0, false);
    let editor = prose_rect(&ctx, &app);
    let titles = title_rects(&wide);
    assert_eq!(
        titles.len(),
        2,
        "one outline label plus one Ledger document label"
    );
    let rail = titles
        .iter()
        .max_by(|a, b| a.left().total_cmp(&b.left()))
        .unwrap();
    assert!(
        rail.right() < editor.left() && editor.width() >= 480.0,
        "{rail:?} vs {editor:?}"
    );
    let narrow = style_settle(&ctx, &mut app, 520.0, false);
    let editor = prose_rect(&ctx, &app);
    let titles = title_rects(&narrow);
    assert_eq!(
        titles.len(),
        1,
        "collapsed outline and a single title above the document"
    );
    assert!(titles[0].bottom() < editor.top());
    assert!((titles[0].left() - editor.left()).abs() < 24.0);
    assert!(editor.width() > 300.0 && editor.right() <= 520.0);
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn ledger_review_keeps_one_chapter_label_and_the_current_unapplied_body() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    frame(
        &ctx,
        &mut app,
        vec![Event::Text("Ledger 当前稿正文".into())],
    );
    app.personal.settings.style = StylePreset::Ledger;
    app.manuscript.reader_open = true;
    app.manuscript.narrow_preview = true;
    let output = style_settle(&ctx, &mut app, 900.0, false);
    assert_eq!(
        title_rects(&output).len(),
        2,
        "outline plus one real review chapter label"
    );
    let text = labels(&output);
    assert!(text.contains("Ledger 当前稿正文") && text.contains("当前稿 · 包含未应用输入"));
    assert!(app.manuscript.writing_buffers()[0].is_changed());
    assert_eq!(
        app.project.document(&app.project.entry).unwrap(),
        "event start\n  -> END\n"
    );
}
