use super::*;

#[test]
fn world_link_preview_keyboard_same_typed_buffer_single_page_and_cancel() {
    let (mut h, plan, applied) = setup(false);
    let staged = h.app.manuscript.writing_buffers()[0].source().to_owned();
    let before = state(&h);
    let request = serde_json::to_value(plan.request()).unwrap();
    let changes = &plan.changes[0];
    assert!(changes.before.as_ref().unwrap().len() < 16 * 1024 && changes.after.len() < 16 * 1024);
    assert_local_cycle(&mut h);
    seek(&mut h, READER);
    eprintln!("W_READING_STAGE single named reader reached");
    expand(&mut h);
    read_pages(&mut h, changes.before.as_ref().unwrap(), &changes.after);
    assert_eq!(state(&h), before);
    assert_eq!(current_plan(&h).plan_digest, plan.plan_digest);
    h.key(Key::Escape, Modifiers::NONE);
    let w = h.app.manuscript.world_links.as_ref().unwrap();
    assert!(!w.open);
    assert_eq!(serde_json::to_value(w.request().unwrap()).unwrap(), request);
    assert_eq!(w.plan.as_ref().unwrap().plan_digest, plan.plan_digest);
    assert_eq!(state(&h), before);
    let selected =
        crate::app::search::editor_selection(&h.ctx).expect("Esc returns exact actual selection");
    assert_eq!(selected.path, plan.request().selection.path);
    assert_eq!(
        selected.range,
        plan.request().selection.start..plan.request().selection.end
    );
    assert_eq!(&selected.source[selected.range.clone()], "旅人");
    assert_eq!(selected.source, staged);
    // Existing draft-history methods independently prove the earlier T edge is intact.
    h.app.edit_undo(false);
    assert_eq!(h.app.manuscript.writing_buffers()[0].source(), applied);
    assert_eq!(
        (h.app.search_state.undo.len(), h.app.search_state.redo.len()),
        (0, 1)
    );
    h.app.edit_undo(true);
    assert_eq!(h.app.manuscript.writing_buffers()[0].source(), staged);
    assert_eq!(
        (h.app.search_state.undo.len(), h.app.search_state.redo.len()),
        (1, 0)
    );
    assert_eq!(h.app.project.document(&h.app.active_file).unwrap(), applied);
    assert_eq!(state(&h)["disk"], before["disk"]);
    eprintln!("W_READING_STAGE single all glyphs, cancel selection, existing T history restored");
}

#[test]
fn world_link_preview_keyboard_utf8_pages_scaled_input_layers_and_stale_source() {
    let (mut h, plan, applied) = setup(true);
    let change = &plan.changes[0];
    assert!(change.before.as_ref().unwrap().len() > 16 * 1024);
    assert!(change.after.len() > 16 * 1024 && change.after.len() < 32 * 1024);
    h.size = egui::vec2(800.0, 600.0);
    h.app.personal.settings.appearance.ui_scale = 1.25;
    h.app.personal.settings.appearance.density = crate::theme::Density::Compact;
    h.settle();
    let before = state(&h);
    assert_local_cycle(&mut h);
    seek(&mut h, READER);
    expand(&mut h);
    read_pages(&mut h, change.before.as_ref().unwrap(), &change.after);
    assert_eq!(state(&h), before);
    assert_eq!(current_plan(&h).plan_digest, plan.plan_digest);
    eprintln!("W_READING_STAGE pages complete; next actual query/IME guard");
    let query_owner = seek_query(&mut h);
    h.key(Key::A, Modifiers::COMMAND);
    h.frame(vec![Event::Text("traveler".into())]);
    assert_eq!(
        h.app.manuscript.world_links.as_ref().unwrap().query,
        "traveler"
    );
    h.key(Key::ArrowLeft, Modifiers::NONE);
    assert_eq!(
        egui::TextEdit::load_state(&h.ctx, query_owner)
            .unwrap()
            .cursor
            .char_range()
            .unwrap()
            .primary
            .index,
        7
    );
    h.frame(vec![
        Event::Ime(egui::ImeEvent::Enabled),
        Event::Ime(egui::ImeEvent::Preedit("候选".into())),
    ]);
    h.key(Key::Escape, Modifiers::NONE);
    assert!(
        h.app.manuscript.world_links.as_ref().unwrap().open,
        "IME owns Escape, not reader/close"
    );
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(query_owner));
    h.frame(vec![Event::Ime(egui::ImeEvent::Commit("字".into()))]);
    h.frame(vec![Event::Ime(egui::ImeEvent::Disabled)]);
    assert_eq!(
        h.app.manuscript.world_links.as_ref().unwrap().query,
        "travele字r"
    );
    assert_eq!(state(&h), before);
    assert_eq!(current_plan(&h).plan_digest, plan.plan_digest);
    h.key(Key::P, Modifiers::COMMAND);
    assert!(h.app.command_palette.open);
    h.key(Key::Escape, Modifiers::NONE);
    assert!(!h.app.command_palette.open && h.app.manuscript.world_links.as_ref().unwrap().open);
    assert_eq!(state(&h), before);
    eprintln!("W_READING_STAGE query IME and upper command cancel complete");
    seek(&mut h, "返回正文，保留输入");
    h.key(Key::Enter, Modifiers::NONE);
    assert!(!h.app.manuscript.world_links.as_ref().unwrap().open);
    assert_eq!(state(&h), before);
    // A real later edit invalidates the fixed source; reopening does not rebind it.
    select_name(&mut h);
    h.key(Key::End, Modifiers::COMMAND);
    h.frame(vec![Event::Text(" source changed".into())]);
    let modified = h.app.manuscript.writing_buffers()[0].source().to_owned();
    assert!(modified.contains("source changed"));
    let modified_state = state(&h);
    let out = h.settle();
    if visible_label(&out, "选词工具").is_none() {
        h.click("正文工具");
    }
    h.click("选词工具");
    h.click("关联世界资料…");
    let w = h.app.manuscript.world_links.as_ref().unwrap();
    assert!(w.open && w.plan.is_none());
    assert_eq!(w.selection.start, plan.request().selection.start);
    assert_eq!(w.selection.end, plan.request().selection.end);
    assert!(w
        .error
        .as_deref()
        .is_some_and(|s| s.contains("当前稿或资料已变化")));
    h.click("预览关联计划");
    let w = h.app.manuscript.world_links.as_ref().unwrap();
    assert!(
        w.plan.is_none() && w.error.is_some(),
        "old fixed generation must be refused"
    );
    assert_eq!(state(&h), modified_state);
    assert_eq!(h.app.project.document(&h.app.active_file).unwrap(), applied);
    assert_eq!(state(&h)["disk"], before["disk"]);
    h.key(Key::Escape, Modifiers::NONE);
    assert_eq!(h.app.manuscript.writing_buffers()[0].source(), modified);
    eprintln!("W_READING_STAGE stale-source refusal and cancel preserve later B");
}
