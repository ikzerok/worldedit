use super::*;
use crate::app::collaboration_ui::comment_lifecycle::ReviewAction;
use worldline_core::collaboration;

#[test]
fn exact_comment_source_navigation_keeps_multiline_selection_after_complete_source_frame() {
    for crlf in [false, true] {
        let (ctx, mut app) = manuscript_app();
        let path = app.active_file.clone();
        if crlf {
            let source = app.project.document(&path).unwrap().replace('\n', "\r\n");
            app.project.set_text(&path, source).unwrap();
            app.recompile();
        }
        let source = app.project.document(&path).unwrap().to_owned();
        let anchor = collaboration::capture_text_anchor(&app.project, &path, 3, 5).unwrap();
        app.new_comment_for_anchor(anchor.clone());
        app.request_review_action(ReviewAction::Source(anchor));
        assert_eq!(app.tab, Tab::Edit);
        assert!(app.jump.is_none(), "完整选区而非普通单光标跳转拥有恢复权");
        app.review_navigation_guard(&ctx);
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![], 11);
        }
        let state = egui::TextEdit::load_state(&ctx, egui::Id::new(("source", &path))).unwrap();
        let range = state.cursor.char_range().unwrap();
        let low = range.primary.index.min(range.secondary.index);
        let high = range.primary.index.max(range.secondary.index);
        let expected_start: usize = source
            .split_inclusive('\n')
            .take(2)
            .map(|line| line.chars().count())
            .sum();
        let expected_end: usize = source
            .split_inclusive('\n')
            .take(5)
            .map(|line| line.chars().count())
            .sum();
        assert_eq!((low, high), (expected_start, expected_end));
        assert_ne!(low, high, "准确原文高亮不能在source_tab末尾被降为单光标");
    }
}

#[test]
fn review_global_ime_preedit_and_commit_never_open_a_note_or_replace_its_composition() {
    let (ctx, mut app) = app();
    app.new_comment_for_anchor(collaboration::CommentAnchor::Object {
        target: TargetRef::new("entity", "a"),
    });
    app.review.comment_editor.as_mut().unwrap().draft.body = "保存的批注".into();
    click(&ctx, &mut app, 7, "保存批注");
    app.review.selected_comment = Some("comment_1".into());
    frame(&ctx, &mut app, vec![], 7);
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("review-comment-list")));
    for ime in [
        egui::ImeEvent::Preedit("中".into()),
        egui::ImeEvent::Commit("中".into()),
    ] {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
            events: vec![
                Event::Ime(ime),
                Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            app.author_shortcuts(ctx);
            app.review_tab(ctx);
        });
        assert!(
            app.review.comment_editor.is_none(),
            "输入法提交帧不能打开批注"
        );
    }
    app.command_palette.ime_frame = false;
    app.edit_comment("comment_1");
    let before = app.review.comment_editor.as_ref().unwrap().draft.clone();
    app.command_palette.ime = true;
    app.edit_comment("comment_1");
    assert!(app.review.pending_comment_action.is_some());
    assert_eq!(app.review.comment_editor.as_ref().unwrap().draft, before);
}

#[test]
fn switching_writing_mode_requires_a_new_visible_selection_before_commenting() {
    let (ctx, mut app) = manuscript_app();
    app.tab = Tab::Manuscript;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], 13);
    }
    click(&ctx, &mut app, 13, "源码");
    let (target, path) = app.manuscript.active_writing_target().unwrap();
    let id = egui::Id::new(("writing-source", &path, &target.kind, &target.id));
    let mut state = egui::TextEdit::load_state(&ctx, id).unwrap();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(8),
        )));
    state.store(&ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
    frame(&ctx, &mut app, vec![], 13);
    assert!(app.manuscript.comment_selection_is_current_mode());
    assert!(crate::app::search::editor_selection(&ctx)
        .is_some_and(|selected| selected.range.start < selected.range.end));
    click(&ctx, &mut app, 13, "写作");
    assert!(!app.manuscript.comment_selection_is_current_mode());
    app.comment_current_selection(&ctx);
    assert!(app.review.comment_editor.is_none());
    assert!(app.io_error.as_ref().unwrap().contains("编辑模式"));
}

#[test]
fn mouse_selection_comment_accepts_pure_ime_disabled_focus_end_once_and_clears_old_error() {
    let (ctx, mut app) = manuscript_app();
    app.tab = Tab::Edit;
    let source_frame = |app: &mut WorldeditApp, events: Vec<Event>| {
        ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                app.author_shortcuts(ctx);
                app.review_navigation_guard(ctx);
                app.source_tab(ctx);
            },
        )
    };
    for _ in 0..3 {
        source_frame(&mut app, vec![]);
    }
    let path = app.active_file.clone();
    let source = app.project.document(&path).unwrap().to_owned();
    let start = source[..source.find("甲乙").unwrap()].chars().count();
    let id = egui::Id::new(("source", &path));
    let mut state = egui::TextEdit::load_state(&ctx, id).unwrap();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(start + 2),
        )));
    state.store(&ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
    source_frame(&mut app, vec![]);
    let output = source_frame(&mut app, vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position_contains(&shape.shape, "为当前选区添加批注"))
        .unwrap();
    app.io_error = Some("请先完成输入法组合，再选择批注范围；当前输入保留。".into());
    for pressed in [true, false] {
        let mut events = vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        if !pressed {
            events.push(Event::Ime(egui::ImeEvent::Disabled));
        }
        source_frame(&mut app, events);
    }
    assert!(
        app.review.comment_editor.is_some(),
        "纯Disabled的首次鼠标点击应成功: {:?}",
        app.io_error
    );
    assert!(app.io_error.is_none(), "成功后不能保留本操作旧红条");
    for ime in [
        egui::ImeEvent::Enabled,
        egui::ImeEvent::Preedit("中".into()),
        egui::ImeEvent::Commit("中".into()),
    ] {
        app.review.comment_editor = None;
        app.tab = Tab::Edit;
        let _ = ctx.run(
            RawInput {
                events: vec![Event::Ime(ime)],
                ..Default::default()
            },
            |ctx| {
                app.author_shortcuts(ctx);
                app.review_navigation_guard(ctx);
                app.comment_current_selection(ctx);
            },
        );
        assert!(
            app.review.comment_editor.is_none(),
            "真实组合/提交帧仍须保护"
        );
        assert!(app
            .io_error
            .as_ref()
            .is_some_and(|error| error.contains("输入法")));
    }
}
