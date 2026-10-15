//! 从实际事件草稿、core typed 预览、App 撤销/保存到磁盘重开的闭环。
use super::*;
const COMPLETE: &str = "world room as \"地点\"\r\ntag calm\r\ntag alert\r\nstate mood on world room with calm\r\nlet allowed = true\r\nlet reputation = 3\r\nevent start after ((allowed)) // 原事件注释\r\n  choice \"进入🙂\" if ((allowed)) enable reputation >= 3 disabled \"需要声望\" // 原分支注释\r\n    正文不可改。\r\n    -> END\r\n";
fn complete() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = setup();
    std::fs::create_dir_all(app.project.root.join(".world")).unwrap();
    std::fs::write(
        app.project.root.join(".world/project.json"),
        r#"{"schema_version":1,"language_version":"1.12"}"#,
    )
    .unwrap();
    std::fs::write(&app.active_file, COMPLETE).unwrap();
    app.project = Project::open(&app.project.root).unwrap();
    app.saved_location = true;
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    app.select_event("start");
    (ctx, app)
}
fn expression(app: &WorldeditApp, slot: ConditionSlot) -> String {
    let draft = &app.event_editor.as_ref().unwrap().draft;
    match slot {
        ConditionSlot::EventAfter => draft.after.clone(),
        ConditionSlot::ChoiceDisplay => draft.choices()[0].condition.clone(),
        ConditionSlot::ChoiceEnable => draft.choices()[0].enable_condition.clone(),
    }
}
fn stage(app: &mut WorldeditApp, slot: ConditionSlot, expression: String) {
    let editor = app.event_editor.as_mut().unwrap();
    if slot == ConditionSlot::EventAfter {
        editor.draft.after = expression;
    } else {
        let mut choice = editor.draft.choices().remove(0);
        if slot == ConditionSlot::ChoiceDisplay {
            choice.condition = expression;
        } else {
            choice.enable_condition = expression;
        }
        editor
            .draft
            .write_choice(Some(choice.line), &choice)
            .unwrap();
    }
}
fn apply_event(app: &mut WorldeditApp) {
    let editor = app.event_editor.as_ref().unwrap().clone();
    assert!(
        app.commit("条件已应用", |project| project
            .write_event_at_baseline(
                &editor.path,
                editor.original.as_deref(),
                &editor.draft,
                &editor.baseline
            )),
        "{:?}",
        app.io_error
    );
    app.event_editor = None;
}
#[test]
fn condition_ui_three_slots_save_reopen_only_change_condition_and_app_undo_redo() {
    for (slot, tree, old) in [
        (
            ConditionSlot::ChoiceDisplay,
            ConditionNode::Has {
                state_id: "mood".into(),
                tag_id: "alert".into(),
            },
            "if ((allowed))",
        ),
        (
            ConditionSlot::ChoiceEnable,
            ConditionNode::Compare {
                variable: "reputation".into(),
                op: ConditionComparison::Ge,
                value: ConditionLiteral::Num(8.0),
                literal_left: false,
            },
            "enable reputation >= 3",
        ),
        (
            ConditionSlot::EventAfter,
            ConditionNode::Not {
                child: Box::new(ConditionNode::GlobalBool {
                    variable: "allowed".into(),
                }),
            },
            "after ((allowed))",
        ),
    ] {
        let (_, mut app) = complete();
        let original = expression(&app, slot);
        let mut input = buffer(&app, &original, slot);
        input.tree = Some(tree.clone());
        input.preview(&context(&app), &original).unwrap();
        assert_eq!(app.project.document(&app.active_file).unwrap(), COMPLETE);
        let replacement = input.apply(&context(&app), &original).unwrap();
        let keyword = match slot {
            ConditionSlot::ChoiceDisplay => "if",
            ConditionSlot::ChoiceEnable => "enable",
            ConditionSlot::EventAfter => "after",
        };
        let expected = COMPLETE.replacen(old, &format!("{keyword} {replacement}"), 1);
        stage(&mut app, slot, replacement);
        assert!(app.has_open_authoring_form());
        assert!(!app.project.is_dirty());
        assert_eq!(std::fs::read_to_string(&app.active_file).unwrap(), COMPLETE);
        apply_event(&mut app);
        assert_eq!(app.project.document(&app.active_file).unwrap(), expected);
        assert!(app.project.is_dirty());
        app.undo(false);
        assert_eq!(app.project.document(&app.active_file).unwrap(), COMPLETE);
        app.undo(true);
        assert_eq!(app.project.document(&app.active_file).unwrap(), expected);
        assert!(app.save());
        assert_eq!(std::fs::read_to_string(&app.active_file).unwrap(), expected);
        let root = app.project.root.clone();
        app.project = Project::open(&root).unwrap();
        app.recompile();
        app.select_event("start");
        let reopened = buffer(&app, &expression(&app, slot), slot);
        assert_eq!(reopened.tree, Some(tree));
        assert!(!app.project.is_dirty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn condition_ui_cancel_preserves_draft_bytes_history_and_both_save_states() {
    let (_, mut app) = complete();
    let baseline = app.project.content_baseline();
    let draft = app.event_editor.as_ref().unwrap().draft.clone();
    let history = app.history.len();
    let mut input = buffer(&app, &draft.after, ConditionSlot::EventAfter);
    input.tree = Some(ConditionNode::Bool { value: false });
    input.preview(&context(&app), &draft.after).unwrap();
    app.event_editor
        .as_mut()
        .unwrap()
        .condition_forms
        .entries
        .push((owner("start", None), ConditionSlot::EventAfter, input));
    assert!(app.has_open_authoring_form());
    assert!(app.save());
    assert!(app
        .event_editor
        .as_ref()
        .unwrap()
        .condition_forms
        .has_pending());
    app.event_editor.as_mut().unwrap().condition_forms.clear();
    assert!(app.event_editor.as_ref().unwrap().draft == draft);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.history.len(), history);
    assert_eq!(std::fs::read_to_string(&app.active_file).unwrap(), COMPLETE);
    assert!(!app.has_open_authoring_form());
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn condition_ui_incomplete_enable_reason_pair_stays_pending_across_frames_until_cancelled() {
    let (ctx, mut app) = complete();
    let mut editor = app.event_editor.as_ref().unwrap().clone();
    let original_body = editor.draft.body.clone();
    let mut choice = editor.draft.choices().remove(0);
    choice.disabled_reason.clear();
    let error = editor
        .draft
        .write_choice(Some(choice.line), &choice)
        .unwrap_err();
    assert_eq!(editor.draft.body, original_body);
    editor.condition_forms.choice_cache = Some((original_body.clone(), vec![choice.clone()]));
    editor.condition_forms.choice_error = Some(error);
    for _ in 0..3 {
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(crate::app::choices::choice_cards(
                    ui,
                    &mut editor.draft,
                    &context(&app),
                    &mut editor.condition_forms
                )
                .is_err());
            });
        });
        assert_eq!(editor.draft.body, original_body);
        assert!(editor.condition_forms.has_pending());
        assert_eq!(
            editor.condition_forms.choice_cache.as_ref().unwrap().1[0],
            choice
        );
    }
    app.event_editor = Some(editor);
    assert!(app.save());
    assert!(app
        .event_editor
        .as_ref()
        .unwrap()
        .condition_forms
        .has_pending());
    assert_eq!(std::fs::read_to_string(&app.active_file).unwrap(), COMPLETE);
    app.event_editor.as_mut().unwrap().condition_forms.clear();
    assert!(!app.has_open_authoring_form());
    std::fs::remove_dir_all(&app.project.root).unwrap();
}
