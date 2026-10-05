use super::*;

#[test]
fn source_outline_draft_rebuild_invalid_source_empty_and_undo_redo() {
    let mut h = Harness::new(source());
    h.open();
    let previous = h.app.project.clone();
    let replacement = source().replace("第二章", "改稿之后的第二章");
    h.change(&replacement);
    assert!(!h.app.navigate_source_outline(&h.ctx, 6));
    assert!(h
        .app
        .source_outline
        .notice
        .as_deref()
        .unwrap()
        .contains("源码已变化"));
    h.settle();
    let cache = h.app.source_outline.cache.as_ref().unwrap();
    assert_eq!(cache.source, replacement);
    assert!(cache
        .outline
        .entries
        .iter()
        .any(|entry| entry.display == "改稿之后的第二章"));
    assert!(h.app.project.is_dirty());
    h.app.remember(previous);
    h.app.edit_undo(false);
    h.settle();
    assert_eq!(
        h.app.source_outline.cache.as_ref().unwrap().source,
        source()
    );
    h.app.edit_undo(true);
    h.settle();
    assert_eq!(
        h.app.source_outline.cache.as_ref().unwrap().source,
        replacement
    );
    h.change("event \"unfinished\n");
    h.settle();
    let cache = h.app.source_outline.cache.as_ref().unwrap();
    assert_ne!(cache.outline.status, SourceOutlineStatus::Ready);
    assert!(cache.outline.entries.is_empty());
    h.press(Key::Enter, Modifiers::NONE);
    assert!(h.app.source_outline.open);
    h.change("");
    let output = h.settle();
    assert!(all_text(&output).contains("本文件没有显式声明"));
    h.press(Key::Enter, Modifiers::NONE);
    assert!(h.app.source_outline.open);
    assert_eq!(h.source(), "");
}

#[test]
fn source_outline_external_changes_inactive_files_and_changed_identity_never_jump() {
    let mut h = Harness::new(source());
    h.select(8, 3);
    h.open();
    let before = h.range();
    std::fs::write(&h.app.active_file, "event changed\n  外改\n").unwrap();
    assert!(!h.app.navigate_source_outline(&h.ctx, 6));
    assert_eq!(h.range(), before);
    assert!(h.app.personal.history.is_empty());
    h.app.refresh_source_outline(true);
    assert_ne!(
        h.app.source_outline.cache.as_ref().unwrap().outline.status,
        SourceOutlineStatus::Ready
    );
    assert_eq!(h.source(), source());
    std::fs::write(&h.app.active_file, source()).unwrap();
    h.app.refresh_source_outline(true);
    let path = h
        .app
        .project
        .add_file(std::path::Path::new("other.wl"))
        .unwrap();
    h.app.active_file = path;
    assert!(!h.app.navigate_source_outline(&h.ctx, 0));
    h.settle();
    assert!(!h.app.source_outline.open);
}

#[test]
fn source_outline_back_after_edit_keeps_new_content_selection_and_ime_draft() {
    let mut h = Harness::new(source());
    h.select(8, 2);
    h.open();
    h.press(Key::End, Modifiers::NONE);
    h.settle();
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    let new = source().replace("中文尾稿", "新写的中文尾稿");
    h.change(&new);
    h.select(3, 1);
    let edited_selection = h.range();
    h.press(Key::ArrowLeft, Modifiers::ALT);
    h.settle();
    assert_eq!(h.source(), new);
    assert_eq!(
        h.range(),
        edited_selection,
        "同文件更新后的选区不能被旧偏移覆盖"
    );
    assert!(h.app.message.as_deref().unwrap().contains("来源版本已变化"));
    h.open();
    h.press(Key::End, Modifiers::NONE);
    h.settle();
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    let history = h.app.personal.history.len();
    h.app.ime_source_draft = Some((
        h.app.active_file.clone(),
        new.clone() + "保留组合稿",
        new.clone(),
    ));
    h.app.author_back(&h.ctx);
    assert_eq!(h.app.personal.history.len(), history);
    assert!(h
        .app
        .ime_source_draft
        .as_ref()
        .unwrap()
        .1
        .ends_with("保留组合稿"));
    assert_eq!(h.source(), new);
}

#[test]
fn source_outline_ime_events_in_query_and_source_never_navigate_or_eat_draft() {
    let mut h = Harness::new(source());
    h.select(7, 3);
    h.open();
    h.press(Key::End, Modifiers::NONE);
    for ime in [
        egui::ImeEvent::Enabled,
        egui::ImeEvent::Preedit("中".into()),
        egui::ImeEvent::Commit("中文".into()),
        egui::ImeEvent::Disabled,
    ] {
        h.frame(vec![
            Event::Ime(ime),
            key_event(Key::Enter, Modifiers::NONE, true),
            key_event(Key::Escape, Modifiers::NONE, true),
        ]);
        assert!(h.app.source_outline.open);
        assert!(h.app.personal.history.is_empty());
        assert_eq!(h.source(), source());
    }
    h.press(Key::Escape, Modifiers::NONE);
    assert!(!h.app.source_outline.open);
    h.frame(vec![
        Event::Ime(egui::ImeEvent::Enabled),
        Event::Ime(egui::ImeEvent::Preedit("潮汐".into())),
    ]);
    assert!(h.app.ime_composing);
    h.app.open_source_outline(&h.ctx);
    assert!(!h.app.source_outline.open);
    assert_eq!(h.app.source_outline_current(&h.ctx), None);
    let saved_draft = h.app.ime_source_draft.clone();
    h.frame(vec![key_event(
        Key::O,
        Modifiers::COMMAND | Modifiers::SHIFT,
        true,
    )]);
    assert!(!h.app.source_outline.open);
    assert_eq!(h.app.ime_source_draft, saved_draft);
    assert_eq!(h.source(), source());
}

#[test]
fn source_outline_unapplied_draft_blocks_old_coordinates_even_after_composition_ends() {
    let mut h = Harness::new(source());
    h.open();
    let draft = format!("// 新增头部🧭\n{}", source());
    h.app.ime_source_draft = Some((h.app.active_file.clone(), draft.clone(), source().into()));
    assert!(!h.app.navigate_source_outline(&h.ctx, 6));
    let output = h.settle();
    assert!(all_text(&output).contains("定位暂不可用"));
    assert_eq!(h.app.source_outline_current(&h.ctx), None);
    assert_eq!(h.app.ime_source_draft.as_ref().unwrap().1, draft);
    assert_eq!(h.source(), source());
}
