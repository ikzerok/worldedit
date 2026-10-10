use super::*;

#[test]
fn migration_keyboard_escape_withdraws_only_preview_and_preserves_request() {
    let mut h = Harness::new(false);
    h.preview();
    let state = h.state();
    let fields = h.fields();
    let request: Value = serde_json::from_str(fields.values().next().unwrap()).unwrap();
    assert_eq!(request["enable_language_1_11"], true);
    let plan = h.current_migration();
    h.tab_to(READ);
    h.key(Key::ArrowDown, Modifiers::NONE);
    let out = h.key(Key::Escape, Modifiers::NONE);
    assert!(!labels(&out).contains(READ));
    assert!(!labels(&out).contains(CONFIRM));
    assert!(!h
        .app
        .manuscript
        .writing_view
        .dialogue_plan_is_current(&plan.source_path, &plan));
    let id = h.ctx.memory(|m| m.focused()).unwrap();
    let response = h.focused_response.clone().unwrap();
    assert_eq!(response.id, id, "same-pass preview owner");
    let rect =
        visible_label(&out, "预览语句变更").expect("Escape returns to a visible preview action");
    assert!(response.has_focus() && control_owns_label(&h.ctx, &response, rect));
    assert_eq!(
        h.fields(),
        fields,
        "including explicit language option, speaker and typed parts"
    );
    assert_eq!(h.state(), state);
    h.key(Key::Enter, Modifiers::NONE);
    h.tab_to(READ);
    assert_eq!(h.fields(), fields);
    assert_eq!(h.state(), state);
}

#[test]
fn migration_keyboard_text_owner_keeps_arrows_space_and_ime() {
    let mut h = Harness::new(false);
    let owner = h.preview();
    let state = h.state();
    h.click(BODY);
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(owner));
    h.key(Key::ArrowDown, Modifiers::NONE);
    let before = egui::TextEdit::load_state(&h.ctx, owner)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    h.key(Key::ArrowUp, Modifiers::NONE);
    let up = egui::TextEdit::load_state(&h.ctx, owner)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_ne!(up, before, "Up remains a text cursor action");
    h.key(Key::ArrowDown, Modifiers::NONE);
    let text = h.literal();
    h.space();
    assert_eq!(h.literal().len(), text.len() + 1);
    assert_eq!(
        h.literal().chars().filter(|c| *c == ' ').count(),
        text.chars().filter(|c| *c == ' ').count() + 1
    );
    h.click("预览语句变更");
    assert!(visible_label(&h.settle(), "正式语句变更预览").is_some());
    h.current_migration();
    let literal = h.literal();
    h.click(&literal);
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(owner));
    let current = h.fields();
    h.frame(vec![Event::Ime(egui::ImeEvent::Preedit("候选".into()))]);
    h.key(Key::ArrowDown, Modifiers::NONE);
    h.key(Key::Tab, Modifiers::NONE);
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(owner));
    h.frame(vec![Event::Ime(egui::ImeEvent::Commit("提交😀".into()))]);
    assert!(h.fields().values().any(|v| v.contains("提交😀")));
    assert_ne!(h.fields(), current);
    assert_eq!(h.state(), state);
}

#[test]
fn migration_keyboard_popup_escape_has_priority_and_text_escape_keeps_fields() {
    let mut h = Harness::new(false);
    let owner = h.preview();
    let state = h.state();
    let fields = h.fields();
    h.click("旅人 · character:traveler");
    assert!(egui::Popup::is_any_open(&h.ctx));
    h.key(Key::Escape, Modifiers::NONE);
    assert!(!egui::Popup::is_any_open(&h.ctx));
    assert!(visible_label(&h.settle(), "正式语句变更预览").is_some());
    h.current_migration();
    assert_eq!(h.fields(), fields);
    assert_eq!(h.state(), state);
    h.tab_to_text(owner, BODY);
    h.key(Key::Escape, Modifiers::NONE);
    assert_eq!(h.fields(), fields);
    assert_eq!(h.state(), state);
    h.tab_to("继续保留此句");
    h.key(Key::Enter, Modifiers::NONE);
    assert_eq!(h.fields(), fields);
    assert_eq!(h.state(), state);
}

#[test]
fn migration_keyboard_plain_body_does_not_gain_preview_stop_or_lose_editing() {
    let mut h = Harness::new(false);
    let out = h.settle();
    assert!(!labels(&out).contains(READ));
    h.click("从这里写下第一段……");
    h.frame(vec![Event::Text("普通正文第一行\n第二行".into())]);
    let owner = h.ctx.memory(|m| m.focused()).unwrap();
    let before = egui::TextEdit::load_state(&h.ctx, owner)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    h.key(Key::ArrowUp, Modifiers::NONE);
    assert_ne!(
        egui::TextEdit::load_state(&h.ctx, owner)
            .unwrap()
            .cursor
            .char_range()
            .unwrap(),
        before
    );
    h.key(Key::ArrowDown, Modifiers::NONE);
    let before = h.app.manuscript.writing_buffers()[0].source().to_owned();
    h.space();
    assert_eq!(
        h.app.manuscript.writing_buffers()[0].source().len(),
        before.len() + 1
    );
    for _ in 0..80 {
        let out = h.key(Key::Tab, Modifiers::NONE);
        assert!(!labels(&out).contains(READ));
    }
    assert!(!h.app.manuscript.has_dialogue_input());
}

#[test]
fn migration_keyboard_author_note_header_has_visible_focus_before_activation() {
    let mut h = Harness::new(false);
    h.preview();
    let state = h.state();
    let fields = h.fields();
    h.tab_to("演出备注 · 作者私密");
    // tab_to proves exact response, complete clip and a local focus outline before Enter.
    h.key(Key::Enter, Modifiers::NONE);
    h.tab_to("此句含演出备注");
    assert_eq!(h.state(), state);
    assert_eq!(h.fields(), fields);
    h.current_migration();
}
