use super::*;
#[test]
fn production_keyboard_scope_pages_source_return_and_locale_rejection() {
    let _serial = serial();
    let mut h = Keyboard::new(45, false, true);
    let disk = h.flow.disk();
    let baseline = h.flow.app.project.content_baseline();
    let initial = h.author_state();
    h.tab_field(ROLE_HINT);
    h.type_value("a");
    h.generate_keyboard();
    assert!(h.flow.app.production_is_current(), "{}", h.flow.notice());
    let first = h.flow.snapshot();
    assert_eq!(first.summary().matching_rows, 46);
    assert_eq!(first.summary().definition_count, 25);
    h.tab_to("角色台本阅读区 · ↑↓滚动");
    let outer = |h: &Keyboard| {
        h.offsets()
            .into_iter()
            .find(|(name, _, _)| name == "production-script-workbench")
            .unwrap()
            .2
    };
    let before = outer(&h);
    h.key(Key::ArrowDown, Modifiers::NONE);
    assert!(outer(&h).y > before.y);
    h.key(Key::ArrowUp, Modifiers::NONE);
    assert_eq!(outer(&h), before);
    h.enter("下一页台词");
    assert_eq!(h.flow.app.manuscript.production.offset, 40);
    assert_eq!(h.flow.snapshot().key(), first.key());
    h.enter("上一页台词");
    h.enter("核对纳入的定义与调用关系");
    h.enter("后20定义");
    assert_eq!(h.flow.app.manuscript.production.details_offset, 20);
    h.enter("前20定义");
    h.enter("完整调用表 · 26处");
    h.enter("后20关系");
    let out = h.settle();
    assert!(
        visible(&out, "21–26 / 26").is_some(),
        "{}",
        h.diagnostic(&out)
    );
    h.enter("前20关系");
    h.enter("书稿筛选章节");
    h.enter("从当前完整书稿筛选中明确勾选章节");
    h.enter("后20章");
    h.enter("前20章");
    h.enter("Selected One · one");
    h.enter("Selected Two · two");
    h.generate_keyboard();
    assert_eq!(h.flow.snapshot().summary().selected_chapter_occurrences, 2);
    assert_eq!(h.flow.snapshot().summary().matching_rows, 46);
    h.enter("纳入共享片段定义");
    h.generate_keyboard();
    assert!(!h.flow.snapshot().summary().includes_fragment_closure);
    assert_eq!(h.flow.snapshot().summary().matching_rows, 45);
    h.enter("全工程活动定义");
    h.generate_keyboard();
    assert!(h.flow.snapshot().summary().includes_fragment_closure);
    // Search is a real field selected by its actual focus witness, not query state.
    h.enter("当前正文目标");
    h.enter("纳入共享片段定义");
    h.generate_keyboard();
    h.tab_field("完整范围查找");
    h.type_value("Shared");
    h.generate_keyboard();
    assert_eq!(h.flow.snapshot().summary().matching_rows, 1);
    let query = h.flow.snapshot().key().to_owned();
    let snapshot = h.flow.snapshot();
    let rows = h.flow.rows();
    assert_eq!(rows.len(), 1);
    let hit = h
        .flow
        .app
        .project
        .production_script_source_hit(
            &h.flow.app.production_buffers(),
            &h.flow.app.production_drafts(),
            &snapshot,
            &rows[0].row_key,
        )
        .unwrap();
    assert_eq!(hit.path, h.flow.app.project.root.join("shared.wl"));
    assert!(!hit.draft);
    let source = h.flow.app.project.document(&hit.path).unwrap().to_owned();
    h.enter("回到原文");
    assert!(!h.flow.app.manuscript.production.open);
    // A shared fragment has no standalone manuscript chapter. The existing
    // verified fallback is the real source editor, with the same exact hit.
    assert!(h.flow.app.tab == crate::app::Tab::Edit);
    assert_eq!(h.flow.app.active_file, hit.path);
    assert_eq!(h.flow.app.project.document(&hit.path).unwrap(), source);
    let id = egui::Id::new(("source", &hit.path));
    assert_eq!(h.flow.ctx.memory(|memory| memory.focused()), Some(id));
    let cursor = egui::TextEdit::load_state(&h.flow.ctx, id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    let start = cursor.primary.index.min(cursor.secondary.index);
    let end = cursor.primary.index.max(cursor.secondary.index);
    let byte = |index| {
        source
            .char_indices()
            .map(|(offset, _)| offset)
            .chain([source.len()])
            .nth(index)
            .unwrap()
    };
    assert_eq!(byte(start)..byte(end), hit.range);
    // Source editing deliberately owns Tab for indentation. Its documented
    // author-navigation shortcut returns without inserting text or changing focus policy.
    h.key(Key::ArrowLeft, Modifiers::ALT);
    assert_eq!(h.flow.app.project.document(&hit.path).unwrap(), source);
    assert!(h.flow.app.manuscript.production.open);
    assert_eq!(h.flow.snapshot().key(), query);
    assert_eq!(h.flow.app.manuscript.production.offset, 0);
    assert_eq!(h.flow.app.manuscript.production.search, "Shared");
    h.tab_field("完整范围查找");
    h.type_value("");
    assert!(h.flow.app.manuscript.production.search.is_empty());
    h.tab_field(LOCALE_HINT);
    h.type_value("en");
    assert_eq!(h.flow.app.manuscript.production.locale, "en");
    h.generate_keyboard();
    assert!(h
        .flow
        .rows()
        .iter()
        .any(|r| r.stable_line_id.as_deref() == Some("main_1")
            && r.selected_parts.is_empty()
            && r.status == ProductionStatus::Translated));
    assert!(h
        .flow
        .rows()
        .iter()
        .any(|r| r.status == ProductionStatus::Missing));
    h.enter("准备私密交付");
    h.enter("预览完整交付字节");
    assert!(h.flow.app.manuscript.production.artifact.is_none());
    assert!(h.flow.notice().contains("LOCALE_INCOMPLETE"));
    h.enter("交付范围与语言选项");
    h.enter("明确允许缺译、过期或无效行回退源文");
    h.generate_keyboard();
    h.enter("预览完整交付字节");
    assert!(h.flow.app.manuscript.production.artifact.is_some());
    h.tab_field(LOCALE_HINT);
    h.type_value("unknown-locale");
    h.generate_keyboard();
    assert!(!h.flow.app.production_is_current());
    assert!(h.flow.notice().contains("UNKNOWN_LOCALE"));
    assert_eq!(h.flow.app.project.content_baseline(), baseline);
    assert_eq!(h.flow.disk(), disk);
    let after = h.author_state();
    for key in ["sources", "fields", "history", "version", "dirty"] {
        assert_eq!(after[key], initial[key]);
    }
    for (path, identity) in initial["buffers"].as_object().unwrap() {
        assert_eq!(&after["buffers"][path], identity);
    }
}
