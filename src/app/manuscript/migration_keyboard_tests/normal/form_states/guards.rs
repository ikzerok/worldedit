use super::*;

#[test]
fn form_state_keyboard_unplanned_scaled_input_and_popup_priority() {
    let mut h = long_form();
    h.size = egui::vec2(800.0, 600.0);
    h.app.personal.settings.appearance.ui_scale = 1.25;
    h.app.personal.settings.appearance.style = crate::theme::StylePreset::Manuscript;
    h.settle();
    let state = h.state();
    let disk_before = disk(&h);
    let owner = field(&mut h, FIRST);
    h.key(Key::ArrowDown, Modifiers::NONE);
    let cursor = egui::TextEdit::load_state(&h.ctx, owner)
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
        cursor
    );
    h.space();
    let before = request(&h);
    let text = match &before.operation {
        DialogueOperation::Update { draft, .. } => match &draft.parts[0] {
            worldline_core::manuscript::DialoguePart::Literal { text } => text.clone(),
            _ => panic!("literal"),
        },
        _ => panic!("Update"),
    };
    assert_eq!(text.len(), FIRST.len() + 1);
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(owner));
    h.frame(vec![Event::Ime(egui::ImeEvent::Preedit("候选".into()))]);
    h.key(Key::ArrowDown, Modifiers::NONE);
    h.key(Key::Tab, Modifiers::NONE);
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(owner));
    h.frame(vec![Event::Ime(egui::ImeEvent::Commit("提交😀".into()))]);
    let f = fields(&h);
    assert_eq!(f.values().next().unwrap().matches("提交😀").count(), 1);
    assert_eq!(h.state(), state);
    assert_eq!(disk(&h), disk_before);
    no_reader(&h.frame(vec![]));
    enter(&mut h, "旅人 · character:traveler");
    assert!(egui::Popup::is_any_open(&h.ctx));
    h.key(Key::Escape, Modifiers::NONE);
    assert!(!egui::Popup::is_any_open(&h.ctx));
    assert_eq!(fields(&h), f);
    assert_eq!(h.state(), state);
    control(&mut h, "演出备注 · 作者私密");
    field(&mut h, NOTE);
    enter(&mut h, "取消此句输入");
    enter(&mut h, "继续保留此句");
    assert_eq!(fields(&h), f);
    assert_eq!(h.state(), state);
    enter(&mut h, PREVIEW);
    let plan = normal_plan(&h);
    assert!(plan.can_apply);
    h.tab_to(NORMAL_READ);
    h.key(Key::Escape, Modifiers::NONE);
    assert!(!current(&h, &plan));
    assert_eq!(fields(&h), f);
    control(&mut h, PREVIEW);
    let literal = match request(&h).operation {
        DialogueOperation::Update { draft, .. } => match &draft.parts[0] {
            worldline_core::manuscript::DialoguePart::Literal { text } => text.clone(),
            _ => panic!("literal"),
        },
        _ => panic!("Update"),
    };
    assert_eq!(field(&mut h, &literal), owner);
    field(&mut h, NOTE);
    assert_eq!(h.state(), state);
    assert_eq!(disk(&h), disk_before);
}
