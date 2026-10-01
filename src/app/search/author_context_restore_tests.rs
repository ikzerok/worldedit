use super::*;

fn focus_prose(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    primary: usize,
    secondary: usize,
) -> egui::Id {
    let (target, path) = app.manuscript.active_writing_target().unwrap();
    let buffer = app.manuscript.writing_buffer_mut(&path).unwrap().clone();
    let projection = app
        .project
        .project_writing_buffer(&buffer, &target)
        .unwrap();
    let offset = projection.blocks[0].range.start;
    let id = egui::Id::new(("writing-prose", &path, &target.kind, &target.id, offset));
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap();
    state.cursor.set_char_range(Some(egui::text::CCursorRange {
        primary: egui::text::CCursor::new(primary),
        secondary: egui::text::CCursor::new(secondary),
        h_pos: None,
    }));
    state.store(ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
    frame(ctx, app, vec![]);
    id
}

#[test]
fn search_back_restores_explicit_mode_unicode_selection_direction_and_scroll() {
    let (ctx, mut app, _) = manuscript();
    let path = app.active_file.clone();
    let old = app
        .manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .source()
        .to_owned();
    let long = old.replace(
        "序章 needle 中文。",
        &(0..90)
            .map(|i| format!("第{i}段中文 needle。\n  "))
            .collect::<String>(),
    );
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source(long);
    frame(&ctx, &mut app, vec![]);
    let id = focus_prose(&ctx, &mut app, 3, 8);
    // 通过会话入口驱动真正 ScrollArea，而不是只给结果断言写入数字。
    let mut session = app.manuscript_session();
    session.scroll_y = 160.0;
    app.restore_manuscript_session(session);
    frame(&ctx, &mut app, vec![]);
    let before = app.manuscript_session();
    assert!(before.scroll_y > 100.0);
    assert_eq!(before.cursor.as_ref().unwrap().cursor, 3);
    assert_eq!(before.cursor.as_ref().unwrap().secondary, Some(8));
    open(&ctx, &mut app, "if true", true, false);
    app.navigate_search(&ctx, false);
    frame(&ctx, &mut app, vec![]);
    app.close_search(&ctx);
    app.author_back(&ctx);
    frame(&ctx, &mut app, vec![]);
    let restored = app.manuscript_session();
    assert_eq!(restored.mode, Mode::Prose);
    assert_eq!(restored.selected_id, before.selected_id);
    assert!(
        (restored.scroll_y - before.scroll_y).abs() < 1.0,
        "restored {} != original {}",
        restored.scroll_y,
        before.scroll_y
    );
    for _ in 0..6 {
        frame(&ctx, &mut app, vec![]);
    }
    assert!(
        (app.manuscript_session().scroll_y - before.scroll_y).abs() < 1.0,
        "an old search animation must not resume after Back"
    );
    let range = egui::TextEdit::load_state(&ctx, id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!((range.primary.index, range.secondary.index), (3, 8));
}

#[test]
fn search_back_after_edit_or_same_text_new_generation_never_restores_old_offsets() {
    for same_text in [false, true] {
        let (ctx, mut app, _) = manuscript();
        let path = app.active_file.clone();
        let id = focus_prose(&ctx, &mut app, 3, 8);
        open(&ctx, &mut app, "if true", true, false);
        app.navigate_search(&ctx, false);
        frame(&ctx, &mut app, vec![]);
        app.close_search(&ctx);
        let buffer = app.manuscript.writing_buffer_mut(&path).unwrap();
        let old = buffer.source().to_owned();
        buffer.replace_source(old.replace("序章", "后续编辑序章"));
        if same_text {
            buffer.replace_source(old);
        }
        let expected = buffer.source().to_owned();
        let generation = buffer.generation();
        app.author_back(&ctx);
        assert!(app
            .message
            .as_deref()
            .unwrap()
            .contains("未恢复旧选区和滚动"));
        frame(&ctx, &mut app, vec![]);
        assert_eq!(app.manuscript_session().mode, Mode::Prose);
        let range = egui::TextEdit::load_state(&ctx, id)
            .unwrap()
            .cursor
            .char_range()
            .unwrap();
        assert_eq!((range.primary.index, range.secondary.index), (0, 0));
        let current = app.manuscript.writing_buffer_mut(&path).unwrap();
        assert_eq!(current.source(), expected);
        assert_eq!(current.generation(), generation);
    }
}

#[test]
fn search_back_external_baseline_change_and_missing_target_keep_author_content() {
    let (ctx, mut app, other) = manuscript();
    let path = app.active_file.clone();
    focus_prose(&ctx, &mut app, 2, 7);
    open(&ctx, &mut app, "if true", true, false);
    app.navigate_search(&ctx, false);
    frame(&ctx, &mut app, vec![]);
    app.close_search(&ctx);
    app.project
        .set_text(&other, "event third\n  外部新版本。\n".into())
        .unwrap();
    app.author_back(&ctx);
    assert!(app
        .message
        .as_deref()
        .unwrap()
        .contains("未恢复旧选区和滚动"));
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.manuscript_session().mode, Mode::Prose);
    assert!(app.project.document(&other).unwrap().contains("外部新版本"));
    assert!(app
        .manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .source()
        .contains("序章"));

    open(&ctx, &mut app, "if true", true, false);
    app.navigate_search(&ctx, false);
    frame(&ctx, &mut app, vec![]);
    app.close_search(&ctx);
    let before = app.manuscript_session();
    let book = app.project.root.join(".world/book.json");
    let bytes = br#"{"schema_version":1,"id":"book","title":"Book","entries":[{"id":"first","kind":"chapter","title":"New","target_ref":{"kind":"event","id":"third"}}]}"#.to_vec();
    app.project.set_authoring_document(&book, bytes).unwrap();
    app.author_back(&ctx);
    assert_eq!(app.manuscript_session().mode, before.mode);
    assert_eq!(app.manuscript_session().selected_id, before.selected_id);
    assert!(app.message.as_deref().unwrap().contains("来源目标已变化"));
}

#[test]
fn orphan_unapplied_draft_hit_never_opens_old_project_text_or_adds_history() {
    let (ctx, mut app) = app();
    let path = app.active_file.clone();
    let mut buffer = app.project.open_source_writing_buffer(&path).unwrap();
    buffer.replace_source("event start\n  未应用独立稿 needle。\n".into());
    app.manuscript.restore_writing_buffers(&[buffer.clone()]);
    open(&ctx, &mut app, "needle", true, true);
    let history = app.personal.history.len();
    app.navigate_search(&ctx, false);
    assert_eq!(app.tab, Tab::Edit);
    assert_eq!(app.personal.history.len(), history);
    assert!(app
        .search_state
        .error
        .as_deref()
        .unwrap()
        .contains("未切换到旧工程内容"));
    assert_eq!(
        app.manuscript.writing_buffer_mut(&path).unwrap().source(),
        buffer.source()
    );
}

#[test]
fn source_search_return_after_external_edit_never_uses_old_cursor_or_scroll() {
    let (ctx, mut app) = app();
    let path = app.active_file.clone();
    frame(&ctx, &mut app, vec![]);
    let id = egui::Id::new(("source", &path));
    let mut state = egui::TextEdit::load_state(&ctx, id).unwrap();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(4),
            egui::text::CCursor::new(12),
        )));
    state.store(&ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
    frame(&ctx, &mut app, vec![]);
    app.personal.source_scroll = [50.0, 200.0];
    open(&ctx, &mut app, "needle", true, false);
    app.navigate_search(&ctx, false);
    frame(&ctx, &mut app, vec![]);
    app.close_search(&ctx);
    app.project
        .set_text(&path, "event start\n  全新文本，不对应旧选区。\n".into())
        .unwrap();
    app.author_back(&ctx);
    let state = egui::TextEdit::load_state(&ctx, id).unwrap();
    let range = state.cursor.char_range().unwrap();
    assert_eq!((range.primary.index, range.secondary.index), (0, 0));
    assert_eq!(app.personal.source_scroll, [0.0, 0.0]);
    assert!(app
        .message
        .as_deref()
        .unwrap()
        .contains("未恢复旧选区和滚动"));
    assert!(app.project.document(&path).unwrap().contains("全新文本"));
}

#[test]
fn clean_unregistered_file_search_explains_source_fallback_and_returns_to_manuscript() {
    let (ctx, mut app, _) = manuscript();
    let orphan = app
        .project
        .add_file(std::path::Path::new("unregistered.wl"))
        .unwrap();
    app.project
        .set_text(&orphan, "event unregistered\n  独立干净命中。\n".into())
        .unwrap();
    app.recompile();
    frame(&ctx, &mut app, vec![]);
    let before = app.manuscript_session();
    let baseline = app.project.content_baseline();
    app.message = Some("不相关旧提示".into());
    open(&ctx, &mut app, "独立干净命中", true, true);
    app.navigate_search(&ctx, false);
    assert_eq!(app.tab, Tab::Edit);
    assert_eq!(app.active_file, orphan);
    assert!(app
        .message
        .as_deref()
        .unwrap()
        .contains("没有可确认的书稿章节"));
    assert!(!app.message.as_deref().unwrap().contains("不相关旧提示"));
    frame(&ctx, &mut app, vec![]);
    app.close_search(&ctx);
    app.author_back(&ctx);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.tab, Tab::Manuscript);
    assert_eq!(app.manuscript_session().selected_id, before.selected_id);
    assert_eq!(app.manuscript_session().mode, before.mode);
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn structure_search_spans_prose_blocks_but_not_target_bodies() {
    let (ctx, mut app, _) = manuscript();
    select(&ctx, &mut app, "first", Mode::Structure);
    open(&ctx, &mut app, "中文。\n  if true\n    分支", true, false);
    app.navigate_search(&ctx, false);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.manuscript_session().mode, Mode::Structure);
    let selection = selection::editor_selection(&ctx).unwrap();
    assert_eq!(
        &selection.source[selection.range],
        "中文。\n  if true\n    分支"
    );
    app.project_query = "END\n\nevent second".into();
    app.navigate_search(&ctx, false);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.manuscript_session().mode, Mode::Source);
    assert!(app.message.as_deref().unwrap().contains("跨越章节声明体"));
}

#[test]
fn back_while_search_open_then_escape_returns_focus_to_restored_mode() {
    let (ctx, mut app, _) = manuscript();
    let prose_id = focus_prose(&ctx, &mut app, 2, 5);
    open(&ctx, &mut app, "if true", true, false);
    app.navigate_search(&ctx, false);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.manuscript_session().mode, Mode::Source);
    app.author_back(&ctx);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.manuscript_session().mode, Mode::Prose);
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(!app.search_open);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(prose_id));
}

#[test]
fn back_to_deleted_source_tombstone_keeps_current_file_and_tab() {
    let (ctx, mut app) = app();
    let first = app
        .project
        .add_file(std::path::Path::new("first.wl"))
        .unwrap();
    let second = app
        .project
        .add_file(std::path::Path::new("second.wl"))
        .unwrap();
    app.project
        .set_text(&first, "event first\n  first text\n".into())
        .unwrap();
    app.project
        .set_text(&second, "event second\n  second needle\n".into())
        .unwrap();
    app.active_file = first.clone();
    frame(&ctx, &mut app, vec![]);
    open(&ctx, &mut app, "needle", true, true);
    let hit = app
        .current_search_hits()
        .unwrap()
        .into_iter()
        .find(|hit| hit.path == second)
        .unwrap();
    app.go_search_hit(&ctx, &hit);
    app.close_search(&ctx);
    assert_eq!(app.active_file, second);
    app.project.delete_document(&first).unwrap();
    assert!(app.project.documents.get(&first).unwrap().is_deleted());
    app.author_back(&ctx);
    assert_eq!(app.active_file, second);
    assert_eq!(app.tab, Tab::Edit);
    assert!(app.message.as_deref().unwrap().contains("来源文件已不存在"));
}

#[test]
fn captured_selection_rejects_new_buffer_generation_even_when_text_returns() {
    let (ctx, mut app, _) = manuscript();
    let path = app.active_file.clone();
    focus_prose(&ctx, &mut app, 3, 8);
    open(&ctx, &mut app, "needle", false, false);
    app.search_state.scope = Scope::Selection;
    assert!(app.search_request().is_ok());
    let buffer = app.manuscript.writing_buffer_mut(&path).unwrap();
    let source = buffer.source().to_owned();
    buffer.replace_source(format!("{source}\n"));
    buffer.replace_source(source);
    assert!(app.search_request().unwrap_err().contains("草稿版本已变化"));
}

#[test]
fn source_search_back_restores_viewport_and_selection_without_resuming_hit_scroll() {
    let (ctx, mut app) = app();
    let path = app.active_file.clone();
    let source = format!(
        "event start\n{}  last_needle\n",
        (0..90)
            .map(|i| format!("  中文第{i}段。\n"))
            .collect::<String>()
    );
    app.project.set_text(&path, source).unwrap();
    frame(&ctx, &mut app, vec![]);
    let id = egui::Id::new(("source", &path));
    let mut state = egui::TextEdit::load_state(&ctx, id).unwrap();
    state.cursor.set_char_range(Some(egui::text::CCursorRange {
        primary: egui::text::CCursor::new(3),
        secondary: egui::text::CCursor::new(8),
        h_pos: None,
    }));
    state.store(&ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
    app.personal.source_scroll = [0.0, 160.0];
    app.personal.restore_source = true;
    frame(&ctx, &mut app, vec![]);
    let before = app.personal.source_scroll;
    assert!(before[1] > 100.0);
    open(&ctx, &mut app, "last_needle", true, false);
    app.navigate_search(&ctx, false);
    frame(&ctx, &mut app, vec![]);
    app.close_search(&ctx);
    app.author_back(&ctx);
    for _ in 0..7 {
        frame(&ctx, &mut app, vec![]);
        assert!(
            (app.personal.source_scroll[1] - before[1]).abs() < 1.0,
            "restored {} != original {}",
            app.personal.source_scroll[1],
            before[1]
        );
    }
    let range = egui::TextEdit::load_state(&ctx, id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!((range.primary.index, range.secondary.index), (3, 8));
}

#[test]
fn manuscript_back_to_deleted_source_keeps_current_chapter_and_cached_drafts() {
    let (ctx, mut app, other) = manuscript();
    let original = app.active_file.clone();
    open(&ctx, &mut app, "跨文件", true, true);
    app.navigate_search(&ctx, false);
    frame(&ctx, &mut app, vec![]);
    app.close_search(&ctx);
    assert_eq!(
        app.manuscript_session().selected_id.as_deref(),
        Some("third")
    );
    assert!(app.manuscript.writing_buffer_mut(&original).is_some());
    app.project.delete_document(&original).unwrap();
    app.author_back(&ctx);
    assert_eq!(app.tab, Tab::Manuscript);
    assert_eq!(
        app.manuscript_session().selected_id.as_deref(),
        Some("third")
    );
    assert_eq!(app.manuscript.active_writing_target().unwrap().1, other);
    assert!(app
        .message
        .as_deref()
        .unwrap()
        .contains("原来源文件已不存在"));
    assert_eq!(app.manuscript.writing_buffers().len(), 2);
}
