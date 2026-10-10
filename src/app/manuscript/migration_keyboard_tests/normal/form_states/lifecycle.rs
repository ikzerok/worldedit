use super::*;

#[test]
fn form_state_keyboard_initial_invalid_valid_and_invalidated_long_fields() {
    let mut h = long_form();
    let state = h.state();
    let disk_before = disk(&h);
    // All eight actual fields of one five-part Say, including link identity and direction.
    let mut owners = std::collections::BTreeMap::new();
    for value in [
        FIRST,
        "1",
        MIDDLE,
        "character",
        "traveler",
        "shown link",
        LAST,
        NOTE,
    ] {
        owners.insert(value, field(&mut h, value));
        no_reader(&h.frame(vec![]));
    }
    for value in [
        LAST,
        "shown link",
        "traveler",
        "character",
        MIDDLE,
        "1",
        FIRST,
    ] {
        assert_eq!(seek(&mut h, value, true, true), owners[value]);
    }
    assert_eq!(h.state(), state);
    assert_eq!(disk(&h), disk_before);
    let expression = field(&mut h, "1");
    replace_current(&mut h, "1 +");
    assert!(immediate(&h), "invalid F still belongs to the live Form");
    let invalid = request(&h);
    let invalid_f = fields(&h);
    let buffer = h.app.manuscript.writing_buffers().pop().unwrap();
    let error = h
        .app
        .project
        .preview_dialogue_edit(&buffer, &invalid)
        .unwrap_err();
    let expected = error.to_string();
    assert!(!expected.is_empty());
    let (mut out, _) = key(&mut h, Key::Enter, Modifiers::COMMAND);
    if visible_label(&out, &expected).is_none() {
        assert_eq!(
            out.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
            std::time::Duration::ZERO,
            "failed Preview must request diagnostic reveal"
        );
        out = h.frame(vec![]);
    }
    for stage in ["requested diagnostic paint", "next idle paint"] {
        assert_eq!(h.ctx.memory(|m| m.focused()), Some(expression), "{stage}");
        assert!(
            visible_label(&out, &expected).is_some(),
            "complete stable core diagnostic {stage}: {:?}; offset={}",
            texts(&out),
            h.offset()
        );
        no_reader(&out);
        assert_eq!(fields(&h), invalid_f);
        assert_eq!(h.state(), state);
        assert_eq!(disk(&h), disk_before);
        out = h.frame(vec![]);
    }
    // Feedback keeps the original receiver; the next real input reveals it again.
    let out = h.frame(vec![Event::Text(" 2".into())]);
    let input_pass = h.ctx.cumulative_pass_nr();
    let followed_requested_paint = h
        .focused_response
        .as_ref()
        .is_some_and(|r| !r.interact_rect.contains_rect(r.rect));
    let input_geometry = input_evidence(&h, &out);
    let out = finish_reveal(&mut h, out);
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(expression));
    assert!(visible_label(&out, "1 + 2").is_some(),
        "input_pass={input_pass} final_pass={} followed_requested_paint={followed_requested_paint}; before={input_geometry}; after={}",
        h.ctx.cumulative_pass_nr(), input_evidence(&h, &out));
    let mut expected_request = serde_json::to_value(&invalid).unwrap();
    expected_request["operation"]["draft"]["parts"][1]["source"] = json!("1 + 2");
    assert_eq!(serde_json::to_value(request(&h)).unwrap(), expected_request);
    enter(&mut h, PREVIEW);
    let plan = normal_plan(&h);
    assert!(plan.can_apply && !plan.no_change);
    assert!(immediate(&h), "valid preview shares the same Form geometry");
    h.tab_to(NORMAL_READ);
    let first = field(&mut h, FIRST);
    replace_current(&mut h, "retained changed first literal");
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(first));
    assert!(!current(&h, &plan));
    assert!(
        immediate(&h),
        "field-invalidated preview retains only Form geometry"
    );
    no_reader(&h.frame(vec![]));
    let f = fields(&h);
    field(&mut h, NOTE);
    control(&mut h, "取消此句输入");
    assert_eq!(fields(&h), f);
    assert_eq!(h.state(), state);
    enter(&mut h, PREVIEW);
    // Ctrl+Enter intentionally selected continuation; explicitly turn it off for this stage.
    h.enter("纳入此句后继续写下一句");
    let plan = normal_plan(&h);
    stage_apply_history(&mut h, &plan);
    assert_eq!(disk(&h), disk_before);
}

#[test]
fn form_state_keyboard_no_change_cancel_stale_and_explicit_rebind() {
    let mut h = long_form();
    let initial = h.state();
    let disk_before = disk(&h);
    let clean = copied_request(&mut h);
    enter(&mut h, PREVIEW);
    let buffer = h.app.manuscript.writing_buffers().pop().unwrap();
    let plan = h
        .app
        .project
        .preview_dialogue_edit(&buffer, &clean)
        .unwrap();
    let is_current = current(&h, &plan);
    assert!(plan.no_change && plan.can_apply && is_current,
        "no_change={} can_apply={} UI_current={is_current}; clean={clean:?}; plan_request={:?}; before={:?}; after={:?}; retained={:?}",
        plan.no_change, plan.can_apply, plan.request, plan.before, plan.after, fields(&h));
    h.enter(STAGE);
    assert_eq!(h.state(), initial);
    assert_eq!(disk(&h), disk_before);
    assert!(!h.app.manuscript.has_dialogue_input());
    assert!(!immediate(&h), "no-change stage removed the Form");
    h.click("编辑此句");
    field(&mut h, FIRST);
    replace_current(&mut h, "retained author literal");
    let f = fields(&h);
    enter(&mut h, "取消此句输入");
    enter(&mut h, "继续保留此句");
    assert_eq!(fields(&h), f);
    assert_eq!(h.state(), initial);
    enter(&mut h, PREVIEW);
    let old = normal_plan(&h);
    h.tab_to(NORMAL_READ);
    h.key(Key::Escape, Modifiers::NONE);
    assert!(!current(&h, &old));
    assert_eq!(fields(&h), f);
    control(&mut h, PREVIEW);
    h.key(Key::Enter, Modifiers::NONE);
    assert!(current(&h, &old));
    let source = h.app.manuscript.writing_buffers()[0].source().to_owned();
    let path = h.app.manuscript.writing_buffers()[0].path().to_owned();
    h.click("源码");
    h.settle();
    assert!(
        !immediate(&h),
        "retained F does not change Source scroll policy"
    );
    let source_id = egui::Id::new(("writing-source", &path, "event", "arrival"));
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(source_id));
    assert!(egui::TextEdit::load_state(&h.ctx, source_id).is_some());
    key(&mut h, Key::A, Modifiers::COMMAND);
    let changed = source.replace("first line", "source changed first");
    h.frame(vec![Event::Text(changed.clone())]);
    assert_eq!(h.app.manuscript.writing_buffers()[0].source(), changed);
    h.click("写作");
    h.settle();
    let after_source = h.state();
    assert!(
        immediate(&h),
        "stale request still has an actual rebind Form"
    );
    assert_eq!(fields(&h), f);
    let buffer = h.app.manuscript.writing_buffers().pop().unwrap();
    assert!(h
        .app
        .project
        .preview_dialogue_edit(&buffer, &old.request)
        .is_err());
    no_reader(&h.frame(vec![]));
    control(&mut h, "明确选择当前位置");
    h.key(Key::Enter, Modifiers::NONE);
    assert!(egui::Popup::is_any_open(&h.ctx));
    let projection = h
        .app
        .project
        .project_dialogue_buffer(&buffer, &old.request.target)
        .unwrap();
    assert_eq!(projection.statements.len(), 1);
    let statement = &projection.statements[0];
    let label = format!(
        "第{}行 · {:?} · {}",
        statement.source.line,
        statement.kind,
        statement.localization_id.as_deref().unwrap_or("无持久 ID")
    );
    // A real visible popup item is selected; no opaque statement ID is injected.
    h.click(&label);
    assert_eq!(fields(&h), f, "choosing a candidate does not bind yet");
    enter(&mut h, "绑定到所选当前位置并重新核对");
    let mut rebound = old.request.clone();
    rebound.expected_baseline = projection.baseline;
    rebound.generation = projection.generation;
    if let DialogueOperation::Update { statement_id, .. } = &mut rebound.operation {
        *statement_id = statement.id.clone();
    } else {
        panic!("Update fixture")
    }
    assert_eq!(request(&h), rebound);
    assert_eq!(h.state(), after_source);
    assert_eq!(disk(&h), disk_before);
    enter(&mut h, PREVIEW);
    normal_plan(&h);
    enter(&mut h, "取消此句输入");
    let kept = fields(&h);
    enter(&mut h, "继续保留此句");
    assert_eq!(fields(&h), kept);
    enter(&mut h, "取消此句输入");
    enter(&mut h, "确认取消此句输入");
    assert!(fields(&h).is_empty());
    assert!(!immediate(&h), "explicit discard removed the Form");
    assert_eq!(h.state(), after_source);
    assert_eq!(disk(&h), disk_before);
}
