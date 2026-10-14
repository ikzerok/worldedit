use super::*;

#[test]
fn migration_keyboard_tab_reveals_impact_and_requires_explicit_confirmation() {
    let mut h = Harness::new(false);
    h.preview();
    let state = h.state();
    let fields = h.fields();
    h.tab_to(READ);
    let out = h.settle();
    assert!(
        h.reading_focus_visible(&out),
        "reading domain needs visible viewport focus"
    );
    for (ime, key) in [
        (egui::ImeEvent::Enabled, Key::ArrowDown),
        (egui::ImeEvent::Disabled, Key::ArrowUp),
        (egui::ImeEvent::Preedit(String::new()), Key::ArrowDown),
    ] {
        let old = h.offset();
        let owner = h.ctx.memory(|memory| memory.focused());
        h.frame(vec![
            Event::Ime(ime),
            Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
        ]);
        h.frame(vec![Event::Key {
            key,
            physical_key: Some(key),
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        let out = h.settle();
        assert_eq!(h.ctx.memory(|memory| memory.focused()), owner);
        assert!(h.reading_focus_visible(&out));
        assert!(
            if key == Key::ArrowDown {
                h.offset() > old
            } else {
                h.offset() < old
            },
            "lifecycle-only IME notification must not block the focused reading command"
        );
        assert_eq!(h.fields(), fields);
        assert_eq!(h.state(), state);
    }
    let offset = h.offset();
    h.key(Key::ArrowDown, Modifiers::CTRL);
    assert!(
        (h.offset() - offset).abs() < 0.1,
        "modified Arrow is not a reading command"
    );
    h.tab_to(READ);
    let plan = h.current_migration();
    let migration = plan.migration.as_ref().unwrap();
    h.enter(&format!(
        "关键字重新解释 · {}处",
        migration.keyword_changes.len()
    ));
    h.enter(&format!(
        "全文候选诊断 · {}项（新增{}项）",
        migration.diagnostics_after.len(),
        migration.new_diagnostics.len()
    ));
    h.enter("本次一次提交的完整文件 · 2个");
    h.tab_to(CONFIRM);
    assert_eq!(h.state(), state);
    assert_eq!(h.fields(), fields);
    assert_eq!(h.app.project.language_version(), "1.9");
    h.key(Key::Enter, Modifiers::NONE);
    h.tab_to(APPLY);
    assert_eq!(
        h.state(),
        state,
        "confirm checkbox is not a Project transaction"
    );
    h.key(Key::Enter, Modifiers::NONE);
    assert_eq!(h.app.project.language_version(), "1.11");
    assert!(h
        .app
        .project
        .document(&h.app.active_file)
        .unwrap()
        .contains("say traveler"));
    assert!(!h.app.manuscript.has_dialogue_input());
    assert_eq!(h.app.history.len(), 1);
    h.app.edit_undo(false);
    assert_eq!(h.app.project.language_version(), "1.9");
    h.app.edit_undo(true);
    assert_eq!(h.app.project.language_version(), "1.11");
}

#[test]
fn migration_keyboard_long_complete_files_are_readable_down_up_and_at_boundaries() {
    let mut h = Harness::new(true);
    h.preview();
    let state = h.state();
    let fields = h.fields();
    let request: worldline_core::manuscript::DialogueEditRequest =
        serde_json::from_str(fields.values().next().unwrap()).unwrap();
    let buffer = h.app.manuscript.writing_buffers().pop().unwrap();
    let plan = h
        .app
        .project
        .preview_dialogue_edit(&buffer, &request)
        .unwrap();
    let change = plan
        .changes
        .iter()
        .find(|change| change.path == buffer.path())
        .unwrap();
    let before = change.before.clone().unwrap();
    let after = change.after.clone();
    h.tab_to(READ);
    h.enter("本次一次提交的完整文件 · 2个");
    // Both real file details, in Tab order; no direct collapse-state injection.
    h.expand_distinct_file_bytes(2);
    let region = h.tab_to(READ).id;
    let mut seen = std::collections::BTreeSet::new();
    let mut expected = std::collections::BTreeSet::new();
    let mut stopped = 0;
    for _ in 0..1000 {
        let old = h.offset();
        let out = h.key(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(h.ctx.memory(|m| m.focused()), Some(region));
        assert!(h.reading_focus_visible(&out));
        for (full, index, row, visible) in galley_rows(&out) {
            let side = if full == before {
                Some(false)
            } else if full == after {
                Some(true)
            } else {
                None
            };
            if let Some(side) = side {
                expected.insert((side, index, row.clone()));
                if visible {
                    seen.insert((side, index, row));
                }
            }
        }
        stopped = if (h.offset() - old).abs() < 0.1 {
            stopped + 1
        } else {
            0
        };
        if stopped == 3 {
            break;
        }
    }
    assert!(expected.iter().filter(|(side, _, _)| !*side).count() > 40);
    assert!(expected.iter().filter(|(side, _, _)| *side).count() > 40);
    assert_eq!(seen,expected,"every nonempty wrapped galley row, with exact side/index/text, must actually enter its clip");
    assert_eq!(stopped, 3, "bounded scroll must reach bottom");
    let bottom = h.offset();
    assert!(bottom > 200.0);
    h.key(Key::ArrowDown, Modifiers::NONE);
    assert!((h.offset() - bottom).abs() < 0.1);
    for _ in 0..1000 {
        h.key(Key::ArrowUp, Modifiers::NONE);
        if h.offset() < 0.1 {
            break;
        }
    }
    assert!(h.offset() < 0.1, "Up can reach the original document top");
    h.key(Key::ArrowUp, Modifiers::NONE);
    assert!(h.offset() < 0.1);
    assert_eq!(h.state(), state);
    assert_eq!(h.fields(), fields);
    h.tab_to(CONFIRM);
    assert_eq!(h.app.project.language_version(), "1.9");
}
