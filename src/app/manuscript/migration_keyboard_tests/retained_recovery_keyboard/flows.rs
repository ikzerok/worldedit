use super::*;

#[test]
fn retained_recovery_keyboard_external_copy_cancel_escape_reload_and_rebind() {
    let mut h = fixture(false, false);
    refresh(&mut h, false);
    let before = state(&h);
    let old = request(&h);
    // The existing Copy is actually focusable even when its original layout is offscreen.
    seek(&mut h, COPY);
    eprintln!("RECOVERY_STAGE short existing Copy fully visible");
    let out = key(&mut h, Key::Enter, Modifiers::NONE).0;
    let copies: Vec<_> = out
        .platform_output
        .commands
        .iter()
        .filter_map(|c| match c {
            egui::OutputCommand::CopyText(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(copies.len(), 1);
    assert_eq!(
        serde_json::from_str::<DialogueEditRequest>(copies[0]).unwrap(),
        old
    );
    assert_eq!(state(&h), before);
    enter(&mut h, CANCEL);
    enter(&mut h, KEEP);
    assert_eq!(
        state(&h),
        before,
        "Keep preserves complete F/B/P/D and all history"
    );
    reading::all_fields(&mut h, false);
    eprintln!("RECOVERY_STAGE short reader/Escape/Copy and Cancel/Keep complete");
    let old_b = h.app.manuscript.writing_buffers().pop().unwrap();
    assert!(!old_b.is_changed());
    let old_f = fields(&h);
    // This is the existing public reload control, not a new recovery shortcut.
    let out = h.frame(vec![]);
    if visible_label(&out, RELOAD).is_none() {
        h.click("正文工具");
    }
    h.click(RELOAD);
    let b = h.app.manuscript.writing_buffers().pop().unwrap();
    assert_eq!(b.source(), h.app.project.document(b.path()).unwrap());
    assert_eq!(b.baseline(), h.app.project.content_baseline());
    assert!(!b.is_changed());
    assert_eq!(
        fields(&h),
        old_f,
        "reload never silently rebinds the request"
    );
    assert!(h.app.project.preview_dialogue_edit(&b, &old).is_err());
    let after_reload = state(&h);
    assert_eq!(
        after_reload["project"]["sources"],
        before["project"]["sources"]
    );
    assert_eq!(after_reload["project"]["docs"], before["project"]["docs"]);
    assert_eq!(after_reload["disk"], before["disk"]);
    assert_eq!(after_reload["node"], before["node"]);
    seek(&mut h, "明确选择当前位置");
    key(&mut h, Key::Enter, Modifiers::NONE);
    assert!(egui::Popup::is_any_open(&h.ctx));
    let projection = h
        .app
        .project
        .project_dialogue_buffer(&b, &old.target)
        .unwrap();
    assert_eq!(projection.statements.len(), 1);
    let statement = &projection.statements[0];
    let caption = format!(
        "第{}行 · {:?} · {}",
        statement.source.line,
        statement.kind,
        statement.localization_id.as_deref().unwrap_or("无持久 ID")
    );
    h.click(&caption);
    assert_eq!(
        fields(&h),
        old_f,
        "choosing a candidate alone does not rebind"
    );
    enter(&mut h, "绑定到所选当前位置并重新核对");
    let mut rebound = old;
    rebound.expected_baseline = projection.baseline;
    rebound.generation = projection.generation;
    if let DialogueOperation::Update { statement_id, .. } = &mut rebound.operation {
        *statement_id = statement.id.clone();
    } else {
        panic!("Update")
    }
    assert_eq!(request(&h), rebound);
    assert_eq!(buffers(&h), after_reload["buffers"]);
    assert_eq!(disk(&h), before["disk"]);
    enter(&mut h, "预览语句变更");
    let plan = h.app.project.preview_dialogue_edit(&b, &rebound).unwrap();
    assert!(plan.can_apply && !plan.no_change);
    assert!(h
        .app
        .manuscript
        .writing_view
        .dialogue_plan_is_current(b.path(), &plan));
    assert_eq!(h.state(), after_reload["project"]);
    assert_eq!(disk(&h), before["disk"]);
    eprintln!("RECOVERY_STAGE short explicit reload/rebind/fresh preview complete");
}

#[test]
fn retained_recovery_keyboard_long_mixed_fields_narrow_scaled_readonly() {
    for (size, scale) in [
        (egui::vec2(1188.0, 848.0), 1.0),
        (egui::vec2(800.0, 600.0), 1.25),
    ] {
        let mut h = fixture(true, false);
        h.size = size;
        h.app.personal.settings.appearance.ui_scale = scale;
        h.settle();
        h.tab_to(NORMAL_READ);
        for _ in 0..12 {
            h.key(Key::ArrowDown, Modifiers::NONE);
        }
        refresh(&mut h, false);
        let before = state(&h);
        seek(&mut h, CANCEL);
        eprintln!("RECOVERY_STAGE long existing Cancel visible size={size:?} scale={scale}");
        reading::all_fields(&mut h, true);
        assert_eq!(state(&h), before);
        // Readonly recovery never routes real text/paste into the stale body or original F.
        h.frame(vec![Event::Text("must not replace retained F".into())]);
        h.frame(vec![Event::Paste("must not replace retained B".into())]);
        assert_eq!(state(&h), before);
        enter(&mut h, CANCEL);
        enter(&mut h, KEEP);
        assert_eq!(state(&h), before);
        eprintln!(
            "RECOVERY_STAGE long all exact original glyphs/Up/Escape/Keep complete size={size:?}"
        );
    }
}

#[test]
fn retained_recovery_keyboard_dirty_body_and_missing_source_never_auto_replace() {
    for missing in [false, true] {
        let mut h = fixture(false, !missing);
        refresh(&mut h, missing);
        let before = state(&h);
        let original = fields(&h);
        seek(&mut h, COPY);
        eprintln!("RECOVERY_STAGE guard existing Copy visible missing={missing}");
        reading::all_fields(&mut h, false);
        let b = h.app.manuscript.writing_buffers().pop().unwrap();
        assert!(h
            .app
            .project
            .preview_dialogue_edit(&b, &request(&h))
            .is_err());
        if missing {
            assert!(h.app.project.document(b.path()).is_err());
            let out = h.frame(vec![]);
            if visible_label(&out, RELOAD).is_none() {
                h.click("正文工具");
            }
            h.click(RELOAD);
            assert_eq!(
                state(&h),
                before,
                "unavailable source cannot replace the old buffer"
            );
            assert_eq!(fields(&h), original);
        } else {
            assert!(b.is_changed());
            let out = h.frame(vec![]);
            if visible_label(&out, "正文工具").is_some() {
                h.click("正文工具");
            }
            let out = h.frame(vec![]);
            assert!(
                !labels(&out).contains(RELOAD),
                "dirty B is not offered clean-buffer replacement"
            );
            if egui::Popup::is_any_open(&h.ctx) {
                key(&mut h, Key::Escape, Modifiers::NONE);
            }
            assert_eq!(state(&h), before);
        }
        enter(&mut h, CANCEL);
        enter(&mut h, KEEP);
        assert_eq!(state(&h), before);
        // Discard remains an explicit, second user decision. An unchanged cache
        // with no remaining F is then retired by the existing clean rebase policy.
        if missing {
            assert!(!b.is_changed());
            assert!(h.app.project.document(b.path()).is_err());
            assert_eq!(h.app.manuscript.writing_buffers().len(), 1);
            assert_eq!(buffers(&h), before["buffers"]);
        }
        enter(&mut h, CANCEL);
        enter(&mut h, "确认取消此句输入");
        assert!(fields(&h).is_empty());
        if missing {
            assert!(h.app.project.document(b.path()).is_err());
            assert!(h.app.manuscript.writing_buffers().is_empty());
            let mut expected = before.clone();
            expected["fields"] = json!({});
            expected["buffers"] = json!([]);
            expected["project"]["buffers"] = json!([]);
            assert_eq!(
                state(&h),
                expected,
                "only the explicitly cancelled F and its clean missing-source cache retire"
            );
        } else {
            assert_eq!(buffers(&h), before["buffers"]);
            assert_eq!(h.state(), before["project"]);
        }
        assert_eq!(disk(&h), before["disk"]);
        assert_eq!(
            format!("{:?}", h.app.history_state.current),
            before["node"].as_str().unwrap()
        );
        eprintln!("RECOVERY_STAGE guard Keep/explicit discard complete; dirty B kept or clean missing cache retired; P/D/history unchanged missing={missing}");
    }
}
