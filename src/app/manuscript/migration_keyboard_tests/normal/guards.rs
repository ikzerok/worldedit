use super::*;

#[test]
fn normal_preview_keyboard_escape_popup_text_and_ime_keep_the_original_owner() {
    let mut h = Harness::normal("");
    let owner = h.begin_fields();
    h.click("预览语句变更");
    let plan = normal_plan(&h);
    let state = h.state();
    let fields = h.fields();
    h.tab_to(NORMAL_READ);
    h.key(Key::ArrowDown, Modifiers::NONE);
    let offset = h.offset();
    h.key(Key::ArrowDown, Modifiers::CTRL);
    assert!((h.offset() - offset).abs() < 0.1);
    let out = h.key(Key::Escape, Modifiers::NONE);
    assert!(!labels(&out).contains(NORMAL_READ));
    assert!(!h
        .app
        .manuscript
        .writing_view
        .dialogue_plan_is_current(&plan.source_path, &plan));
    let r = h.focused_response.clone().unwrap();
    let rect = visible_label(&out, "预览语句变更").expect("visible Preview after Escape");
    assert!(r.has_focus() && control_owns_label(&h.ctx, &r, rect));
    assert_eq!(h.fields(), fields);
    assert_eq!(h.state(), state);
    h.key(Key::Enter, Modifiers::NONE);
    normal_plan(&h);
    h.tab_to("旅人 · character:traveler");
    h.key(Key::Enter, Modifiers::NONE);
    assert!(egui::Popup::is_any_open(&h.ctx));
    h.key(Key::Escape, Modifiers::NONE);
    assert!(!egui::Popup::is_any_open(&h.ctx));
    normal_plan(&h);
    assert_eq!(h.fields(), fields);
    assert_eq!(h.state(), state);
    h.tab_to_text(owner, BODY);
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
    h.key(Key::Escape, Modifiers::NONE);
    assert_eq!(h.fields(), fields);
    assert_eq!(h.state(), state);
    h.tab_to("继续保留此句");
    h.key(Key::Enter, Modifiers::NONE);
    h.tab_to_text(owner, BODY);
    let text = h.literal();
    h.space();
    assert_eq!(h.literal().len(), text.len() + 1);
    h.click("预览语句变更");
    normal_plan(&h);
    let literal = h.literal();
    h.tab_to_text(owner, &literal);
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(owner));
    h.frame(vec![Event::Ime(egui::ImeEvent::Preedit("候选".into()))]);
    h.key(Key::ArrowDown, Modifiers::NONE);
    h.key(Key::Tab, Modifiers::NONE);
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(owner));
    h.frame(vec![Event::Ime(egui::ImeEvent::Commit("提交😀".into()))]);
    assert!(h.fields().values().any(|s| s.contains("提交😀")));
    assert_eq!(h.state(), state);
}

#[test]
fn normal_preview_keyboard_source_change_invalidates_navigation_without_losing_fields() {
    let mut h = setup(Case::UpdateText);
    h.tab_to(NORMAL_READ);
    let plan = normal_plan(&h);
    let fields = h.fields();
    let project = h.app.project.content_baseline();
    let disk_before = disk(&h);
    let source = h.app.manuscript.writing_buffers()[0].source().to_owned();
    h.click("源码");
    h.click(&source);
    let owner = h.ctx.memory(|m| m.focused()).unwrap();
    assert!(egui::TextEdit::load_state(&h.ctx, owner).is_some());
    h.key(Key::A, Modifiers::COMMAND);
    let changed = source.replace(OLD, "changed in source");
    h.frame(vec![Event::Text(changed.clone())]);
    assert_eq!(h.app.manuscript.writing_buffers()[0].source(), changed);
    h.click("写作");
    let out = h.settle();
    assert!(!labels(&out).contains(NORMAL_READ));
    assert_eq!(
        h.app
            .manuscript
            .writing_view
            .retained_runtime_drafts(&h.app.project.root),
        fields,
        "the entire original F request is retained exactly"
    );
    let buffer = h.app.manuscript.writing_buffers().pop().unwrap();
    let mut expected_inputs = fields.clone();
    let relative = buffer.path().strip_prefix(&h.app.project.root).unwrap();
    assert!(expected_inputs
        .insert(
            format!("正文文件 · {}", relative.display()),
            json!([
                "writing_buffer",
                buffer.path(),
                buffer.baseline(),
                buffer.generation(),
                buffer.source()
            ])
            .to_string()
        )
        .is_none());
    assert_eq!(
        h.fields(),
        expected_inputs,
        "only the exact newly edited B is added"
    );
    assert_eq!(h.app.project.content_baseline(), project);
    assert_eq!(disk(&h), disk_before);
    // This exact old request is retained, but cannot produce a new current plan
    // against the changed buffer or revive a reading-domain credential.
    let buffer = h.app.manuscript.writing_buffers().pop().unwrap();
    assert!(h
        .app
        .project
        .preview_dialogue_edit(&buffer, &plan.request)
        .is_err());
}
