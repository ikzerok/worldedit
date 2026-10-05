use super::*;

#[test]
fn source_jump_changed_source_rebuild_blocks_old_enter_and_undo_redo_rebuilds() {
    let mut h = Harness::new(source());
    h.open();
    h.query("7:8");
    let old = h.app.project.clone();
    let replacement = format!("// 新头部\n{}", source());
    h.change(&replacement);
    h.frame(vec![key_event(Key::Enter, Modifiers::NONE, true)]);
    assert!(h.app.source_jump.open);
    assert!(h.app.personal.history.is_empty());
    h.settle();
    assert_eq!(
        h.app.source_jump.review.as_ref().unwrap().source,
        replacement
    );
    h.app.remember(old);
    h.app.edit_undo(false);
    h.settle();
    assert_eq!(h.app.source_jump.review.as_ref().unwrap().source, source());
    h.app.edit_undo(true);
    h.settle();
    assert_eq!(
        h.app.source_jump.review.as_ref().unwrap().source,
        replacement
    );
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(!h.app.source_jump.open);
    assert_eq!(h.source(), replacement);
}

#[test]
fn source_jump_confirmation_revalidates_external_edit_and_file_identity() {
    let mut h = Harness::new(source());
    h.select(8, 2);
    h.open();
    h.query("7:8");
    let before = h.range();
    std::fs::write(&h.app.active_file, "// 外改\n").unwrap();
    assert!(!h.app.navigate_source_jump(&h.ctx));
    assert!(h.app.source_jump.open);
    assert!(h.app.personal.history.is_empty());
    assert_eq!(h.range(), before);
    assert_eq!(h.source(), source());
    std::fs::write(&h.app.active_file, source()).unwrap();
    h.app.refresh_source_jump(true);
    h.settle();
    let other = h
        .app
        .project
        .add_file(std::path::Path::new("other.wl"))
        .unwrap();
    h.app.active_file = other;
    assert!(!h.app.navigate_source_jump(&h.ctx));
    h.settle();
    assert!(!h.app.source_jump.open);
    assert!(h.app.personal.history.is_empty());
}

#[test]
fn source_jump_back_after_edit_preserves_new_text_and_selection() {
    let mut h = Harness::new(source());
    h.select(8, 2);
    h.jump("7:8");
    let updated = source().replace("中文尾稿", "新的中文尾稿");
    h.change(&updated);
    h.select(3, 1);
    let before = h.range();
    h.press(Key::ArrowLeft, Modifiers::ALT);
    h.settle();
    assert_eq!(h.source(), updated);
    assert_eq!(h.range(), before);
    assert!(h.app.message.as_deref().unwrap().contains("来源版本已变化"));
}

#[test]
fn source_jump_ime_events_do_not_navigate_cancel_or_write_source() {
    let mut h = Harness::new(source());
    h.select(8, 2);
    h.open();
    for event in [
        egui::ImeEvent::Enabled,
        egui::ImeEvent::Preedit("中".into()),
        egui::ImeEvent::Commit("中文".into()),
        egui::ImeEvent::Disabled,
    ] {
        h.frame(vec![
            Event::Ime(event),
            key_event(Key::Enter, Modifiers::NONE, true),
            key_event(Key::Escape, Modifiers::NONE, true),
        ]);
        assert!(h.app.source_jump.open);
        assert!(h.app.personal.history.is_empty());
        assert_eq!(h.source(), source());
    }
    h.press(Key::Escape, Modifiers::NONE);
    h.frame(vec![
        Event::Ime(egui::ImeEvent::Enabled),
        Event::Ime(egui::ImeEvent::Preedit("潮汐".into())),
    ]);
    assert!(h.app.ime_composing);
    h.app.open_source_jump(&h.ctx);
    assert!(!h.app.source_jump.open);
    let draft = h.app.ime_source_draft.clone();
    h.press(Key::G, Modifiers::CTRL);
    assert!(!h.app.source_jump.open);
    assert_eq!(h.app.ime_source_draft, draft);
    assert_eq!(h.source(), source());
}

#[test]
fn source_jump_unapplied_ime_draft_blocks_old_preview_and_back() {
    let mut h = Harness::new(source());
    h.jump("7:8");
    h.open();
    let draft = format!("// 未应用头部\n{}", source());
    h.app.ime_source_draft = Some((h.app.active_file.clone(), draft.clone(), source().into()));
    let history = h.app.personal.history.len();
    assert!(!h.app.navigate_source_jump(&h.ctx));
    h.app.author_back(&h.ctx);
    assert_eq!(h.app.personal.history.len(), history);
    assert_eq!(h.app.ime_source_draft.as_ref().unwrap().1, draft);
    assert_eq!(h.source(), source());
    assert!(h.app.source_jump_position(&h.ctx).is_err());
}
