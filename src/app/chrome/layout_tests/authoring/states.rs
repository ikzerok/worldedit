use super::*;

#[test]
fn edit_state_truth_table_saves_only_applied_bytes_and_keeps_draft_details() {
    for drafts in [false, true] {
        for dirty in [false, true] {
            let mut h = Harness::new(drafts, dirty);
            let before = h.unchanged();
            let output = h.settle();
            let state = h.app.save_state();
            assert_eq!((state.drafts, state.project), (drafts, dirty));
            assert!(text_position(&output, state.label()).is_some());
            h.click_role("global/save-state");
            let output = h.settle();
            assert!(text_position(&output, "输入与工程保存").is_some());
            for name in h.app.dirty_draft_names() {
                assert!(text_position(&output, &format!("• {name}")).is_some());
            }
            assert_eq!(h.unchanged(), before, "opening status is read-only");
            h.frame(vec![Event::Key {
                key: egui::Key::Escape,
                physical_key: Some(egui::Key::Escape),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            let source = h
                .app
                .project
                .document(&h.app.active_file)
                .unwrap()
                .to_owned();
            assert!(h.app.save());
            assert_eq!(std::fs::read_to_string(&h.app.active_file).unwrap(), source);
            assert!(!h.app.project.is_dirty());
            assert_eq!(h.app.has_open_authoring_form(), drafts);
            assert_eq!(h.unchanged().1, before.1);
            let output = h.settle();
            assert!(text_position(&output, h.app.save_state().label()).is_some());
        }
    }
}

#[test]
fn save_conflict_keeps_both_states_input_bytes_and_persistent_error() {
    let mut h = Harness::new(true, true);
    h.settle();
    let before = h.unchanged();
    let external = "event external\n  外部变化\n  -> END\n";
    std::fs::write(&h.app.active_file, external).unwrap();
    assert!(!h.app.save());
    assert!(h.app.io_error.is_some());
    assert_eq!(h.unchanged(), before);
    assert_eq!(
        std::fs::read_to_string(&h.app.active_file).unwrap(),
        external
    );
    for _ in 0..3 {
        let output = h.settle();
        assert!(text_position(&output, "输入待应用 · 工程待保存").is_some());
        assert!(h.app.io_error.is_some(), "error cannot be a timed toast");
    }
}

#[test]
fn manuscript_identity_is_not_repeated_and_arrangement_actions_are_one_step_reachable() {
    let mut h = Harness::new(false, false);
    let before = h.unchanged();
    let output = h.settle();
    fn titles(shape: &egui::Shape) -> usize {
        match shape {
            egui::Shape::Text(text) => usize::from(text.galley.text() == LONG_TITLE),
            egui::Shape::Vec(shapes) => shapes.iter().map(titles).sum(),
            _ => 0,
        }
    }
    assert_eq!(
        output
            .shapes
            .iter()
            .map(|shape| titles(&shape.shape))
            .sum::<usize>(),
        1
    );
    let responses = trace(&h.ctx);
    let book = h.response("manuscript/book");
    let help = h.response("manuscript/help");
    assert!(
        (book.rect.center().y - help.rect.center().y).abs() < 1.0,
        "default management has one action row"
    );
    assert!(responses
        .iter()
        .any(|r| r.role == "manuscript/create-chapter"));
    h.click_role("manuscript/arrange");
    let output = h.settle();
    for label in ["新建书稿", "插入分节", "插入章节", "恢复书稿草稿"] {
        assert!(text_position(&output, label).is_some(), "{label}");
    }
    let pos = text_position(&output, "插入分节").unwrap();
    for pressed in [true, false] {
        h.frame(vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }
    let output = h.settle();
    assert!(text_position(&output, "编排待应用").is_some());
    assert!(h.response("manuscript/apply").enabled);
    assert_eq!(
        h.unchanged(),
        before,
        "inserting an arrangement remains an unapplied draft"
    );
}

#[test]
fn global_save_state_keeps_keyboard_identity_when_label_and_width_change() {
    let mut h = Harness::new(true, true);
    h.settle();
    let id = h.response("global/save-state").id;
    let commands = h.response("global/commands").id;
    h.ctx.memory_mut(|memory| memory.request_focus(id));
    for size in [egui::vec2(400.0, 300.0), egui::vec2(1280.0, 760.0)] {
        h.size = size;
        h.settle();
        assert_eq!(h.response("global/save-state").id, id);
        assert_eq!(h.response("global/commands").id, commands);
        assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(id));
    }
    assert!(h.app.save());
    h.settle();
    assert_eq!(h.response("global/save-state").id, id);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(id));
}

#[test]
fn command_save_in_real_app_keeps_unapplied_text_and_updates_only_project_state() {
    let mut h = Harness::new(true, true);
    h.settle();
    let before = h.unchanged();
    let applied = h
        .app
        .project
        .document(&h.app.active_file)
        .unwrap()
        .to_owned();
    h.full_frame(vec![Event::Key {
        key: egui::Key::S,
        physical_key: Some(egui::Key::S),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }]);
    assert!(!h.app.project.is_dirty());
    assert!(h.app.has_open_authoring_form());
    assert_eq!(h.unchanged().1, before.1);
    assert_eq!(
        std::fs::read_to_string(&h.app.active_file).unwrap(),
        applied
    );
}

#[test]
fn synthetic_source_composition_keeps_its_editor_when_action_bars_fold() {
    let mut h = Harness::new(false, false);
    h.settle();
    let source = egui::Id::new(("writing-source", &h.app.active_file, "event", "arrival"));
    h.ctx.memory_mut(|memory| memory.request_focus(source));
    h.full_frame(vec![]);
    let baseline = h.app.project.content_baseline();
    h.full_frame(vec![
        Event::Ime(egui::ImeEvent::Enabled),
        Event::Ime(egui::ImeEvent::Preedit("海雾".into())),
    ]);
    h.size = egui::vec2(400.0, 300.0);
    h.full_frame(vec![Event::Ime(egui::ImeEvent::Preedit("海雾升起".into()))]);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(source));
    h.full_frame(vec![
        Event::Ime(egui::ImeEvent::Commit("海雾升起".into())),
        Event::Ime(egui::ImeEvent::Disabled),
    ]);
    h.size = egui::vec2(1280.0, 760.0);
    h.full_frame(vec![]);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(source));
    assert!(h
        .app
        .manuscript
        .writing_buffers()
        .iter()
        .any(|buffer| buffer.source().contains("海雾升起")));
    assert_eq!(h.app.project.content_baseline(), baseline);
}
