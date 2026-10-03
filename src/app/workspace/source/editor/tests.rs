//! 真正执行高亮/TextEdit/ScrollArea的帧回归；不冒充系统窗口或物理IME验收。
use crate::app::{Tab, WorldeditApp};
use egui::{text::CCursor, text::CCursorRange, Context, Event, RawInput};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
static NEXT: AtomicUsize = AtomicUsize::new(0);

fn app() -> (Context, WorldeditApp, String) {
    let ctx = Context::default();
    ctx.style_mut(|s| {
        s.animation_time = 0.0;
        s.scroll_animation = egui::style::ScrollAnimation::none();
    });
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = std::env::temp_dir().join(format!(
        "worldedit-source-wrap-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    app.project = worldline_core::project::Project::new(&root);
    app.active_file = app.project.entry.clone();
    let source = format!(
        "entity harbor kind place as \"潮港\"\r\nevent start\r\n  {}目标🧭${{true}}[[entity:harbor|潮港]]{}\r\n\r\n{}\r\n",
        "长段落里的中文潮声和emoji🌊。".repeat(240), "ASCII_".repeat(100),
        (0..70).map(|i| format!("// 余下第{i}行\r\n")).collect::<String>(),
    );
    app.project
        .set_text(&app.active_file.clone(), source.clone())
        .unwrap();
    app.tab = Tab::Edit;
    app.personal.settings.diagnostics = false;
    app.recompile();
    (ctx, app, source)
}

fn frame(
    ctx: &Context,
    app: &mut WorldeditApp,
    width: f32,
    height: f32,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, height),
            )),
            events,
            ..Default::default()
        },
        |ctx| app.source_tab(ctx),
    )
}

fn rendered_source(output: &egui::FullOutput, text: &str) -> (egui::Pos2, Arc<egui::Galley>) {
    fn find(shape: &egui::Shape, text: &str) -> Option<(egui::Pos2, Arc<egui::Galley>)> {
        match shape {
            egui::Shape::Text(shape) if shape.galley.job.text == text => {
                Some((shape.pos, shape.galley.clone()))
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| find(s, text)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|s| find(&s.shape, text))
        .expect("源码真实galley必须可见")
}

fn range(ctx: &Context, app: &WorldeditApp) -> CCursorRange {
    egui::TextEdit::load_state(ctx, egui::Id::new(("source", &app.active_file)))
        .unwrap()
        .cursor
        .char_range()
        .unwrap()
}

fn select(ctx: &Context, app: &WorldeditApp, primary: usize, secondary: usize) {
    let id = egui::Id::new(("source", &app.active_file));
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap();
    state.cursor.set_char_range(Some(CCursorRange {
        primary: CCursor::new(primary),
        secondary: CCursor::new(secondary),
        h_pos: None,
    }));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}

fn settle(ctx: &Context, app: &mut WorldeditApp, width: f32, height: f32) -> egui::FullOutput {
    for _ in 0..3 {
        let _ = frame(ctx, app, width, height, vec![]);
    }
    frame(ctx, app, width, height, vec![])
}

#[test]
fn source_wrap_frames_preserve_bytes_and_physical_lines_across_width_and_font() {
    let (ctx, mut app, source) = app();
    let baseline = app.project.content_baseline();
    let history = app.history.len();
    let logical_lines = source.split('\n').count();
    for (wrap, width, size) in [
        (false, 1000.0, 16.0),
        (true, 700.0, 16.0),
        (true, 360.0, 28.0),
        (true, 1400.0, 16.0),
        (false, 700.0, 28.0),
    ] {
        app.personal.settings.source_wrap = wrap;
        app.personal.settings.body_size = size;
        let output = settle(&ctx, &mut app, width, 800.0);
        let (_, galley) = rendered_source(&output, &source);
        assert_eq!(galley.job.text.as_bytes(), source.as_bytes());
        assert_eq!(
            galley
                .rows
                .iter()
                .filter(|row| row.ends_with_newline)
                .count()
                + 1,
            logical_lines
        );
        if wrap {
            assert!(galley.rows.len() > logical_lines);
            assert!(
                galley.size().x <= width - 25.0,
                "正文宽{}应收进{width}",
                galley.size().x
            );
            assert_eq!(app.personal.source_scroll[0], 0.0);
        } else {
            assert_eq!(galley.rows.len(), logical_lines);
            assert!(galley.size().x > width * 4.0);
        }
        assert_eq!(
            app.project.document(&app.active_file).unwrap().as_bytes(),
            source.as_bytes()
        );
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(app.history.len(), history);
    }
}

#[test]
fn source_wrap_keeps_selection_direction_and_visible_anchor_after_reflow() {
    let (ctx, mut app, source) = app();
    app.personal.settings.source_wrap = true;
    settle(&ctx, &mut app, 900.0, 750.0);
    let index = source[..source.find("目标").unwrap()].chars().count();
    app.jump = Some((
        3,
        (index
            - source
                .split_inclusive('\n')
                .take(2)
                .map(|s| s.chars().count())
                .sum::<usize>()
            + 1) as u32,
    ));
    settle(&ctx, &mut app, 900.0, 750.0);
    select(&ctx, &app, index, index + 3);
    let output = settle(&ctx, &mut app, 900.0, 750.0);
    let (origin, galley) = rendered_source(&output, &source);
    let before_y = origin.y + galley.pos_from_cursor(CCursor::new(index)).min.y;
    assert!((100.0..750.0).contains(&before_y));
    for (width, height, size, spacing, wrap) in [
        (480.0, 750.0, 28.0, 1.8, true),
        (1250.0, 750.0, 16.0, 1.45, true),
        (900.0, 750.0, 16.0, 1.45, false),
        (900.0, 750.0, 16.0, 1.45, true),
    ] {
        app.personal.settings.body_size = size;
        app.personal.settings.line_spacing = spacing;
        app.personal.settings.source_wrap = wrap;
        let output = settle(&ctx, &mut app, width, height);
        let range = range(&ctx, &app);
        assert_eq!(
            (range.primary.index, range.secondary.index),
            (index, index + 3)
        );
        let (origin, galley) = rendered_source(&output, &source);
        let position = origin + galley.pos_from_cursor(range.primary).min.to_vec2();
        assert!(
            (50.0..height).contains(&position.y),
            "{wrap}/{width}/{size}: cursor={position:?}"
        );
        assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    }
}

#[test]
fn source_wrap_back_reflows_saved_location_and_rejects_stale_source() {
    let (ctx, mut app, source) = app();
    app.personal.settings.source_wrap = true;
    settle(&ctx, &mut app, 850.0, 750.0);
    app.jump = Some((3, 2000));
    settle(&ctx, &mut app, 850.0, 750.0);
    let at = range(&ctx, &app).primary.index;
    select(&ctx, &app, at + 4, at);
    settle(&ctx, &mut app, 850.0, 750.0);
    let location = app.author_location(Some(&ctx));
    let encoded = serde_json::to_string(&location).unwrap();
    assert!(!encoded.contains("长段落"));
    app.remember_author_location(location.clone());
    app.jump = Some((60, 1));
    settle(&ctx, &mut app, 850.0, 750.0);
    app.personal.settings.body_size = 28.0;
    app.author_back(&ctx);
    let output = settle(&ctx, &mut app, 500.0, 750.0);
    let selected = range(&ctx, &app);
    assert_eq!(
        (selected.primary.index, selected.secondary.index),
        (at + 4, at)
    );
    let (origin, galley) = rendered_source(&output, &source);
    let y = origin.y + galley.pos_from_cursor(selected.primary).center().y;
    assert!((50.0..750.0).contains(&y), "Back重排后光标必须可见：{y}");
    app.remember_author_location(location);
    let edited = source.replace("长段落", "后来改过的段落");
    app.project
        .set_text(&app.active_file.clone(), edited.clone())
        .unwrap();
    app.author_back(&ctx);
    settle(&ctx, &mut app, 500.0, 750.0);
    assert_eq!(range(&ctx, &app).primary.index, 0);
    assert_eq!(app.project.document(&app.active_file).unwrap(), edited);
    assert!(app
        .message
        .as_deref()
        .unwrap()
        .contains("未恢复旧选区和滚动"));
}

#[test]
fn source_wrap_reflow_preserves_unapplied_ime_draft() {
    let (ctx, mut app, source) = app();
    let draft = format!("{source}输入法尚未提交的中文🧭");
    app.ime_source_baseline = Some((app.active_file.clone(), source.clone()));
    app.ime_source_draft = Some((app.active_file.clone(), draft.clone(), source.clone()));
    app.ime_composing = true;
    let baseline = app.project.content_baseline();
    for (wrap, size, width) in [
        (true, 16.0, 900.0),
        (true, 28.0, 480.0),
        (false, 16.0, 700.0),
    ] {
        app.personal.settings.source_wrap = wrap;
        app.personal.settings.body_size = size;
        let output = settle(&ctx, &mut app, width, 750.0);
        assert_eq!(rendered_source(&output, &draft).1.job.text, draft);
        assert_eq!(app.project.document(&app.active_file).unwrap(), source);
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(app.ime_source_draft.as_ref().unwrap().1, draft);
        assert!(app.ime_composing);
    }
}

#[test]
fn source_wrap_search_selection_wins_over_previous_view_when_width_changes() {
    let (ctx, mut app, source) = app();
    app.personal.settings.source_wrap = true;
    settle(&ctx, &mut app, 1000.0, 800.0);
    let start = source.find("目标🧭").unwrap();
    let end = start + "目标🧭".len();
    crate::app::search::request_selection(
        &ctx,
        app.active_file.clone(),
        source.clone(),
        start..end,
    );
    app.personal.settings.body_size = 28.0;
    let output = settle(&ctx, &mut app, 500.0, 800.0);
    let selected = range(&ctx, &app);
    assert_eq!(
        selected.as_sorted_char_range(),
        source[..start].chars().count()..source[..end].chars().count()
    );
    let (origin, galley) = rendered_source(&output, &source);
    let rect = galley
        .pos_from_cursor(selected.primary)
        .translate(origin.to_vec2());
    assert!(
        (80.0..800.0).contains(&rect.center().y),
        "搜索命中应在新galley视口内：{rect:?}"
    );
    let captured = crate::app::search::editor_selection(&ctx).unwrap();
    assert_eq!(&captured.source[captured.range], "目标🧭");
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
}

#[test]
fn source_wrap_reflow_keeps_scrolled_context_when_cursor_is_offscreen() {
    let (ctx, mut app, source) = app();
    app.personal.settings.source_wrap = true;
    settle(&ctx, &mut app, 850.0, 750.0);
    select(&ctx, &app, 0, 0);
    app.personal.source_scroll = [0.0, 1800.0];
    app.personal.restore_source = true;
    settle(&ctx, &mut app, 850.0, 750.0);
    let old = serde_json::to_value(&app.personal.source_view.as_ref().unwrap().1).unwrap();
    let anchor = old["anchor"].as_u64().unwrap() as usize;
    assert!(anchor > 50, "可见锚点应来自滚动后的正文，不能跳回首字符");
    app.personal.settings.body_size = 28.0;
    let output = settle(&ctx, &mut app, 500.0, 750.0);
    assert_eq!(range(&ctx, &app).primary.index, 0);
    let (origin, galley) = rendered_source(&output, &source);
    let y = origin.y + galley.pos_from_cursor(CCursor::new(anchor)).center().y;
    assert!(
        (60.0..750.0).contains(&y),
        "原可见逻辑字符重排后必须保留在视口：{y}"
    );
    assert!(app.personal.source_scroll[1] > 1000.0);
}

#[test]
fn source_wrap_gutter_and_jump_share_actual_row_positions_including_crlf_blanks() {
    let (ctx, mut app, source) = app();
    app.personal.settings.source_wrap = true;
    for size in [16.0, 28.0] {
        app.personal.settings.body_size = size;
        for line in [3, 4, source.split('\n').count()] {
            app.jump = Some((line as u32, 1));
            let output = settle(&ctx, &mut app, 520.0, 780.0);
            let (origin, galley) = rendered_source(&output, &source);
            let index = source
                .split_inclusive('\n')
                .take(line - 1)
                .map(|s| s.chars().count())
                .sum();
            let y = origin.y + galley.pos_from_cursor(CCursor::new(index)).center().y;
            let number = line.to_string();
            let gutter_y = output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::Shape::Text(text) if text.galley.job.text == number => {
                        Some(text.pos.y + text.galley.rect.center().y)
                    }
                    _ => None,
                })
                .expect("可见物理行包括空行应有gutter行号");
            assert!(
                (y - gutter_y).abs() < 1.0,
                "字号{size}物理行{line}：正文{y}与gutter{gutter_y}"
            );
            assert_eq!(range(&ctx, &app).primary.index, index);
        }
    }
}
