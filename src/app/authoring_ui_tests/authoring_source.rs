use super::*;

#[test]
fn ime_commit_after_external_refresh_keeps_external_source_and_preserves_local_draft() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let source = "entity a kind place as \"同名\"\nevent start\n  原始正文";
    app.project.set_text(&entry, source.into()).unwrap();
    app.project.save().unwrap();
    app.tab = super::Tab::Edit;

    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(source.chars().count()),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let _ = frame(
        &ctx,
        &mut app,
        vec![
            Event::Ime(egui::ImeEvent::Enabled),
            Event::Ime(egui::ImeEvent::Preedit("@同名".into())),
        ],
        11,
    );
    assert!(app.ime_source_draft.is_some());
    assert_eq!(app.project.document(&entry).unwrap(), source);

    let external = format!("{source}\n外部版本");
    std::fs::write(&entry, external.as_bytes()).unwrap();
    assert!(app.project.refresh().unwrap().is_empty());
    app.recompile();
    let external_baseline = app.project.content_baseline();

    let _ = frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Commit("@同名".into()))],
        11,
    );

    assert_eq!(app.project.content_baseline(), external_baseline);
    assert_eq!(app.project.document(&entry).unwrap(), external);
    let (draft_path, draft, _) = app.ime_source_draft.as_ref().unwrap();
    assert_eq!(draft_path, &entry);
    assert!(
        draft.contains("@同名"),
        "local IME input must remain recoverable"
    );
    assert!(app
        .io_error
        .as_deref()
        .is_some_and(|error| error.contains("外部修改")));
    assert_eq!(std::fs::read_to_string(&entry).unwrap(), external);

    click(&ctx, &mut app, 11, "放弃本地草稿并恢复外部版本");
    assert!(app.ime_source_draft.is_none());
    assert_eq!(app.project.document(&entry).unwrap(), external);
    assert_eq!(std::fs::read_to_string(&entry).unwrap(), external);
}

#[test]
fn selected_source_text_can_create_a_linked_entity_and_one_undo_restores_raw_text() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let source = "entity a kind place as \"同名\"\nevent start\n  发现失落城池，随后找到遗迹";
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;
    let selected = "失落城池";
    let byte_start = source.find(selected).unwrap();
    let start = source[..byte_start].chars().count();
    let end = start + selected.chars().count();
    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(end),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let baseline = app.project.content_baseline();
    let _ = frame(&ctx, &mut app, Vec::new(), 11);
    click(&ctx, &mut app, 11, "从选中文本建档");
    assert_eq!(app.project.document(&entry).unwrap(), source);
    assert_eq!(app.project.content_baseline(), baseline);
    let form = app.entity_editor.as_ref().unwrap();
    let id = form.draft.id.clone();
    assert_eq!(form.draft.display, selected);
    assert_eq!(
        form.source_selection.as_ref().unwrap().expected_text,
        selected
    );

    click(&ctx, &mut app, 0, "应用资料");
    assert!(app.entity_editor.is_none(), "{:?}", app.io_error);
    assert!(app
        .project
        .document(&entry)
        .unwrap()
        .contains(&format!("[[entity:{id}|{selected}]]")));
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities
        .contains_key(&id));
    assert_eq!(app.history.len(), 1);
    app.undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&entry).unwrap(), source);
}

#[test]
fn selected_source_text_can_open_and_apply_the_entity_form_without_a_mouse() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let source = "entity a kind place as \"同名\"\nevent start\n  发现失落城池，随后找到遗迹";
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;
    let selected = "失落城池";
    let byte_start = source.find(selected).unwrap();
    let start = source[..byte_start].chars().count();
    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(start),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let shift_right = Event::Key {
        key: egui::Key::ArrowRight,
        physical_key: Some(egui::Key::ArrowRight),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::SHIFT,
    };
    let _ = frame(&ctx, &mut app, Vec::new(), 11);
    assert!(ctx.memory(|memory| memory.has_focus(editor)));
    let _ = frame(
        &ctx,
        &mut app,
        vec![shift_right; selected.chars().count()],
        11,
    );
    let range = egui::TextEdit::load_state(&ctx, editor)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!(
        range.primary.index.abs_diff(range.secondary.index),
        selected.chars().count()
    );
    assert!(ctx.memory(|memory| memory.has_focus(editor)));
    assert_eq!(
        source
            .chars()
            .skip(range.primary.index.min(range.secondary.index))
            .take(range.primary.index.abs_diff(range.secondary.index))
            .collect::<String>(),
        selected
    );
    let shortcut = Event::Key {
        key: egui::Key::Enter,
        physical_key: Some(egui::Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    };

    let _ = frame(&ctx, &mut app, vec![shortcut.clone()], 11);
    assert!(app.entity_editor.is_some());
    assert_eq!(app.project.document(&entry).unwrap(), source);
    assert_eq!(app.entity_editor.as_ref().unwrap().draft.display, selected);

    let id = app.entity_editor.as_ref().unwrap().draft.id.clone();
    let _ = frame(&ctx, &mut app, Vec::new(), 0);
    let _ = frame(&ctx, &mut app, vec![shortcut], 0);
    assert!(app.entity_editor.is_none());
    assert!(app
        .project
        .document(&entry)
        .unwrap()
        .contains(&format!("[[entity:{id}|{selected}]]")));
    assert_eq!(app.history.len(), 1);
}

#[test]
fn clicking_a_source_link_opens_reading_and_returns_to_the_same_editor_cursor() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let link = "[[entity:a|同名]]";
    let source = format!("entity a kind place as \"同名\"\nevent start\n  看见{link}，继续");
    app.project.set_text(&entry, source.clone()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 11);
    }
    let output = frame(&ctx, &mut app, Vec::new(), 11);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| source_text_position(&shape.shape, &source, link))
        .expect("源码链接必须在编辑器中可定位");
    for pressed in [true, false] {
        let _ = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            11,
        );
    }
    assert_eq!(app.reading_target, Some(TargetRef::new("entity", "a")));
    let (return_path, return_cursor) = app.reading_return.clone().unwrap();
    assert_eq!(return_path, entry);

    click(&ctx, &mut app, 8, "返回源码编辑");
    assert!(app.reading_target.is_none());
    assert_eq!(app.tab, super::Tab::Edit);
    assert_eq!(app.active_file, entry);
    assert_eq!(
        egui::TextEdit::load_state(&ctx, egui::Id::new(("source", &entry)))
            .unwrap()
            .cursor
            .char_range()
            .unwrap()
            .primary
            .index,
        return_cursor
    );
    assert!(app.reading_return.is_none());
}

#[test]
fn source_links_can_be_opened_and_returned_to_by_keyboard() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let link = "[[entity:a|同名]]";
    let source = format!("entity a kind place as \"同名\"\nevent start\n  看见{link}，继续");
    app.project.set_text(&entry, source.clone()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;
    let byte = source.find(link).unwrap();
    let cursor = source[..byte].chars().count() + 5;
    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(cursor),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let shortcut = Event::Key {
        key: egui::Key::Enter,
        physical_key: Some(egui::Key::Enter),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    };

    let _ = frame(&ctx, &mut app, vec![shortcut], 11);
    assert_eq!(app.reading_target, Some(TargetRef::new("entity", "a")));
    let (_, return_cursor) = app.reading_return.clone().unwrap();
    let _ = frame(
        &ctx,
        &mut app,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: Some(egui::Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        8,
    );
    assert!(app.reading_target.is_none());
    assert_eq!(app.tab, super::Tab::Edit);
    assert_eq!(
        egui::TextEdit::load_state(&ctx, editor)
            .unwrap()
            .cursor
            .char_range()
            .unwrap()
            .primary
            .index,
        return_cursor
    );
}

#[test]
fn cancelling_a_selected_text_draft_preserves_the_unmarked_source_and_project() {
    let (ctx, mut app) = app();
    let source = "entity a kind place as \"同名\"\nevent start\n  发现失落城池，随后找到遗迹";
    let entry = open_selected_entity_form(&ctx, &mut app, source, "失落城池");
    let baseline = app.project.content_baseline();
    app.entity_editor.as_mut().unwrap().draft.description = "尚未提交".into();
    click(&ctx, &mut app, 0, "取消");
    assert!(app.entity_editor.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&entry).unwrap(), source);
    assert!(app.history.is_empty());
}

#[test]
fn escape_closes_a_selected_text_draft_without_mutating_the_source() {
    let (ctx, mut app) = app();
    let source = "entity a kind place as \"同名\"\nevent start\n  发现失落城池，随后找到遗迹";
    let entry = open_selected_entity_form(&ctx, &mut app, source, "失落城池");
    let baseline = app.project.content_baseline();
    app.entity_editor.as_mut().unwrap().draft.display = "暂存名称".into();
    let _ = frame(
        &ctx,
        &mut app,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: Some(egui::Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        0,
    );
    assert!(app.entity_editor.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&entry).unwrap(), source);
    assert!(app.history.is_empty());
}

#[test]
fn external_source_change_rejects_selected_entity_apply_and_keeps_the_draft() {
    let (ctx, mut app) = app();
    let source = "entity a kind place as \"同名\"\nevent start\n  发现失落城池，随后找到遗迹";
    let entry = open_selected_entity_form(&ctx, &mut app, source, "失落城池");
    let baseline = app.project.content_baseline();
    let id = app.entity_editor.as_ref().unwrap().draft.id.clone();
    app.entity_editor.as_mut().unwrap().draft.description = "保留在草稿中".into();
    std::fs::create_dir_all(&app.project.root).unwrap();
    std::fs::write(&entry, b"external source version").unwrap();

    click(&ctx, &mut app, 0, "应用资料");
    assert!(app.entity_editor.is_some());
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.description,
        "保留在草稿中"
    );
    assert!(app
        .io_error
        .as_deref()
        .is_some_and(|error| error.contains("外部修改")));
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(!app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities
        .contains_key(&id));
    std::fs::remove_file(entry).unwrap();
}

#[test]
fn missing_entity_link_offers_a_repair_draft_with_the_stable_missing_id() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    app.project
        .set_text(
            &entry,
            "entity a kind place as \"同名\"\nevent start\n  参考[[entity:missing_place|失落城池]]。"
                .into(),
        )
        .unwrap();
    app.recompile();
    let target = TargetRef::new("entity", "missing_place");
    app.open_reading(target.clone());
    let output = frame(&ctx, &mut app, Vec::new(), 12);
    assert!(output.shapes.iter().any(|shape| {
        text_position(
            &shape.shape,
            "资料已失效：entity:missing_place。可能已被删除或更改 ID。",
        )
        .is_some()
    }));
    assert!(output.shapes.iter().any(|shape| {
        text_position(&shape.shape, "按此 ID 新建实体以修复引用").is_some()
    }));
    click(&ctx, &mut app, 12, "按此 ID 新建实体以修复引用");
    let form = app.entity_editor.as_ref().unwrap();
    assert_eq!(form.draft.id, "missing_place");
    assert_eq!(form.draft.display, "失落城池");
    click(&ctx, &mut app, 0, "应用资料");
    assert!(app.entity_editor.is_none(), "{:?}", app.io_error);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .object(&target)
        .is_some());
    assert!(!app
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == "A218"));
}
