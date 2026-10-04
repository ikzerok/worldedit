use super::*;
use crate::app::writing_workspace::Mode;

#[test]
fn current_review_navigation_uses_same_draft_and_back_restores_mode_cursor() {
    let (ctx, mut app) = app_with_source(SOURCE);
    let size = vec2(1040.0, 660.0);
    app.personal.settings.focus = true;
    settle(&ctx, &mut app, size);
    let path = app.active_file.clone();
    let original = app.project.document(&path).unwrap().to_owned();
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source(original.replace("灯塔亮起", "灯塔亮起未应用稿"));
    app.manuscript.writing_view.restore_mode(Mode::Source);
    settle(&ctx, &mut app, size);
    let id = egui::Id::new(("writing-source", &path, "event", "start"));
    ctx.memory_mut(|memory| memory.request_focus(id));
    let mut edit = egui::TextEdit::load_state(&ctx, id).unwrap_or_default();
    edit.cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(5),
            egui::text::CCursor::new(12),
        )));
    edit.store(&ctx, id);
    settle(&ctx, &mut app, size);
    key(
        &ctx,
        &mut app,
        size,
        egui::Key::R,
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
    );
    let before = app.manuscript_session();
    assert_eq!(before.cursor.as_ref().unwrap().cursor, 12);
    let baseline = app.project.content_baseline();
    let draft = app.manuscript.writing_buffers()[0].source().to_owned();
    let selected = request(&app, "公开营救结局");
    app.jump_review_source(&ctx, &selected).unwrap();
    assert!(app.review_return_available());
    settle(&ctx, &mut app, size);
    let selection = crate::app::search::editor_selection(&ctx).unwrap();
    assert_eq!(selection.path, path);
    assert_eq!(
        selection.source.get(selection.range).unwrap(),
        selected.source.excerpt
    );
    click(&ctx, &mut app, size, "返回审稿");
    settle(&ctx, &mut app, size);
    let after = app.manuscript_session();
    assert_eq!(after.selected_id, before.selected_id);
    assert_eq!(after.preview_tab, before.preview_tab);
    assert_eq!(after.mode, before.mode);
    // 预览不重绘隐藏编辑器；返回到编辑后再恢复原光标与选区方向。
    key(
        &ctx,
        &mut app,
        size,
        egui::Key::R,
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
    );
    settle(&ctx, &mut app, size);
    let restored = egui::TextEdit::load_state(&ctx, id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!((restored.primary.index, restored.secondary.index), (12, 5));
    assert_eq!(app.manuscript.writing_buffers()[0].source(), draft);
    assert_eq!(app.project.document(&path).unwrap(), original);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn review_source_rejects_stale_draft_ime_forged_range_and_external_disk_change() {
    let (ctx, mut app) = app_with_source(SOURCE);
    let size = vec2(1800.0, 900.0);
    settle(&ctx, &mut app, size);
    let valid = request(&app, "灯塔亮起");
    let before = app.manuscript_session();
    let history = app.personal.history.len();
    app.ime_composing = true;
    assert!(app
        .jump_review_source(&ctx, &valid)
        .unwrap_err()
        .contains("输入法"));
    app.ime_composing = false;
    let mut forged = valid.clone();
    forged.source.byte_start += 1;
    assert!(app.jump_review_source(&ctx, &forged).is_err());
    let path = app.active_file.clone();
    let buffer = app.manuscript.writing_buffer_mut(&path).unwrap();
    buffer.replace_source(format!("// 新位置\n{}", buffer.source()));
    assert!(app
        .jump_review_source(&ctx, &valid)
        .unwrap_err()
        .contains("过期"));
    assert_eq!(app.manuscript_session().selected_id, before.selected_id);
    assert_eq!(app.personal.history.len(), history);
    settle(&ctx, &mut app, size);
    let updated = request(&app, "灯塔亮起");
    app.project.save().unwrap();
    std::fs::write(&path, SOURCE.replace("林芜", "外部人物名")).unwrap();
    let draft = app.manuscript.writing_buffers()[0].source().to_owned();
    assert!(app
        .jump_review_source(&ctx, &updated)
        .unwrap_err()
        .contains("外部"));
    assert_eq!(app.manuscript.writing_buffers()[0].source(), draft);
    assert_eq!(app.personal.history.len(), history);
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn invalid_draft_shows_explicit_old_review_and_never_navigates_it() {
    let (ctx, mut app) = app_with_source(SOURCE);
    app.personal.settings.focus = true;
    let size = vec2(1040.0, 660.0);
    settle(&ctx, &mut app, size);
    app.manuscript.narrow_preview = true;
    settle(&ctx, &mut app, size);
    let valid = request(&app, "灯塔亮起");
    let path = app.active_file.clone();
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source(format!("{SOURCE}\n  if (\n"));
    let output = settle(&ctx, &mut app, size);
    let text = texts(&output);
    assert!(
        text.contains("预览过期")
            && text.contains("上次有效预览")
            && text.contains("旧来源跳转已停用"),
        "{text}"
    );
    assert!(app.manuscript.preview_cache.current.is_empty());
    assert!(app.jump_review_source(&ctx, &valid).is_err());
    assert!(app.manuscript.writing_buffers()[0]
        .source()
        .contains("if ("));
    assert!(app.history.is_empty());
}

#[test]
fn source_click_is_deferred_until_book_session_is_back_in_state() {
    let (ctx, mut app) = app_with_source(SOURCE);
    app.personal.settings.focus = true;
    app.manuscript.narrow_preview = true;
    let size = vec2(1040.0, 660.0);
    settle(&ctx, &mut app, size);
    click(&ctx, &mut app, size, "定位原文");
    assert!(app.review_return_available());
    let origin = app.personal.history.last().unwrap();
    let session: session::ManuscriptSession =
        serde_json::from_value(origin.manuscript.clone()).unwrap();
    assert_eq!(session.selected_id.as_deref(), Some("first"));
    assert_eq!(session.preview_tab, Some(true));
    assert!(!app.manuscript.narrow_preview);
    assert_eq!(app.manuscript.writing_view.session_mode(), Mode::Source);
}

#[test]
fn unrelated_character_file_external_conflict_blocks_body_source_navigation() {
    let (ctx, mut app) = app_with_source(SOURCE);
    let people = app
        .project
        .add_file(std::path::Path::new("people.wl"))
        .unwrap();
    let path = app.active_file.clone();
    app.project
        .set_text(
            &path,
            format!(
                "include \"people.wl\"\n{}",
                SOURCE.replace("character lin as \"林芜\"\n", "")
            ),
        )
        .unwrap();
    app.project
        .set_text(&people, "character lin as \"林芜\"\n".into())
        .unwrap();
    app.project.save().unwrap();
    app.project
        .set_text(&people, "character lin as \"本地未保存名\"\n".into())
        .unwrap();
    app.recompile();
    let size = vec2(1800.0, 900.0);
    settle(&ctx, &mut app, size);
    let source = request(&app, "灯塔亮起");
    let baseline = app.project.content_baseline();
    std::fs::write(&people, "character lin as \"外部新稿\"\n").unwrap();
    let conflicts = app.project.refresh().unwrap();
    assert!(conflicts.contains(&people));
    assert_eq!(
        app.project.content_baseline(),
        baseline,
        "冲突保留本地text，不可仅依赖content baseline"
    );
    assert!(app
        .jump_review_source(&ctx, &source)
        .unwrap_err()
        .contains("外部"));
    assert_eq!(
        app.project.document(&people).unwrap(),
        "character lin as \"本地未保存名\"\n"
    );
    assert!(app.personal.history.is_empty());
    assert!(app.history.is_empty());
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn draft_moved_declaration_navigates_core_proven_file_without_using_old_target_path() {
    let (ctx, mut app) = app_with_source(SOURCE);
    let other = app
        .project
        .add_file(std::path::Path::new("moved.wl"))
        .unwrap();
    let path = app.active_file.clone();
    app.project
        .set_text(&path, format!("include \"moved.wl\"\n{SOURCE}"))
        .unwrap();
    app.project
        .set_text(&other, "// 移入目的文件\n".into())
        .unwrap();
    app.recompile();
    let size = vec2(1800.0, 900.0);
    settle(&ctx, &mut app, size);
    let start = SOURCE.find("event start").unwrap();
    let end = SOURCE.find("event second").unwrap();
    let moved_event = SOURCE[start..end].to_owned();
    let buffer = app.manuscript.writing_buffer_mut(&path).unwrap();
    buffer.replace_source(buffer.source().replace(&moved_event, ""));
    let mut destination = app.project.open_source_writing_buffer(&other).unwrap();
    destination.replace_source(moved_event);
    app.manuscript.restore_writing_buffers(&[destination]);
    app.manuscript.writing_view.restore_mode(Mode::Source);
    settle(&ctx, &mut app, size);
    let source = request(&app, "灯塔亮起");
    assert_eq!(std::path::PathBuf::from(&source.source.file), other);
    app.jump_review_source(&ctx, &source).unwrap();
    settle(&ctx, &mut app, size);
    let selection = crate::app::search::editor_selection(&ctx).unwrap();
    assert_eq!(selection.path, other);
    assert_eq!(
        selection.source.get(selection.range).unwrap(),
        source.source.excerpt
    );
    assert_eq!(
        app.manuscript
            .writing_buffers()
            .iter()
            .filter(|buffer| buffer.is_changed())
            .count(),
        2
    );
    assert!(app.project.document(&path).unwrap().contains("event start"));
    assert!(!app
        .project
        .document(&other)
        .unwrap()
        .contains("event start"));
    app.author_back(&ctx);
    settle(&ctx, &mut app, size);
    assert_eq!(
        app.manuscript.active_writing_target().unwrap().1,
        path,
        "返回恢复原编辑文件，不把旧坐标放进声明移动后的文件"
    );
    assert!(app.history.is_empty());
}

#[test]
fn over_budget_chapter_is_explicitly_incomplete_and_preserves_the_entire_draft() {
    let (ctx, mut app) = app_with_source(SOURCE);
    app.personal.settings.focus = true;
    // 只观察审稿页；并排的合法正文编辑区仍必须显示完整当前输入。
    let size = vec2(900.0, 660.0);
    settle(&ctx, &mut app, size);
    app.manuscript.narrow_preview = true;
    settle(&ctx, &mut app, size);
    let old = request(&app, "灯塔亮起");
    let path = app.active_file.clone();
    let source = format!(
        "event start\n{}  -> END\nevent second\n  -> END\n",
        "  超预算新正文。\n".repeat(10_100)
    );
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source(source.clone());
    let text = texts(&settle(&ctx, &mut app, size));
    assert!(
        text.contains("本章审稿未完成") && text.contains("超过规模预算"),
        "{text}"
    );
    assert!(
        !text.contains("超预算新正文"),
        "不能把残稿伪装成完整当前正文"
    );
    assert!(app.jump_review_source(&ctx, &old).is_err());
    assert_eq!(app.manuscript.writing_buffers()[0].source(), source);
    assert!(app.history.is_empty());
}
