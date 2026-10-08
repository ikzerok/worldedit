use super::*;
use worldline_core::ast::PropertyValue;

#[test]
fn pristine_manager_and_trial_values_are_not_unsubmitted_authoring() {
    let mut state = ManagerState::default();
    assert!(!state.has_unsubmitted_work());
    state
        .trial_values
        .push(("example".into(), PropertyValue::Str("只试填".into())));
    assert!(!state.has_unsubmitted_work());
    assert!(state.request(Action::New).is_some());
}
#[test]
fn untouched_new_template_is_protected_from_every_replacement_action() {
    for action in [
        Action::New,
        Action::Copy("template_place".into()),
        Action::Select("project:other".into()),
        Action::Reload("project:here".into()),
    ] {
        let mut state = ManagerState {
            editor: "generated draft".into(),
            baseline_editor: "generated draft".into(),
            is_new: true,
            ..Default::default()
        };
        assert!(state.has_unsubmitted_work());
        assert!(state.request(action).is_none());
        assert!(state.pending.is_some());
        assert_eq!(state.editor, "generated draft");
        state.pending = None;
        assert_eq!(state.editor, "generated draft", "取消不能丢新草稿");
        state.discard();
        assert!(!state.has_unsubmitted_work());
    }
}
#[test]
fn invalid_json_and_pending_property_input_are_protected_independent_of_selection() {
    let mut state = ManagerState {
        editor: "{\n broken 原文".into(),
        ..Default::default()
    };
    assert!(state.has_unsubmitted_work());
    assert!(state.request(Action::New).is_none());
    assert_eq!(state.editor, "{\n broken 原文");
    state.baseline_editor = state.editor.clone();
    assert!(!state.has_unsubmitted_work());
    let field = ProjectTemplateField {
        id: "field_a".into(),
        key: Some("a".into()),
        label: "标题".into(),
        field_type: "number".into(),
        required: false,
        choices: vec![],
        target: None,
        target_entity_type: None,
        default: None,
        fields: vec![],
    };
    let mut input = FieldInput::new(&field);
    input.has_default = true;
    input.default_text = "无效数字".into();
    assert!(input.value().is_err());
    state.properties = Some(input);
    assert!(state.has_unsubmitted_work());
    assert!(state.request(Action::New).is_none());
    assert_eq!(state.properties.as_ref().unwrap().default_text, "无效数字");
}
#[test]
fn ime_composition_blocks_destructive_navigation_without_clearing_input() {
    let mut state = ManagerState {
        editor: "中文组合".into(),
        ime_composing: true,
        ..Default::default()
    };
    assert!(state.request(Action::New).is_none());
    assert!(state.pending.is_none());
    assert_eq!(state.editor, "中文组合");
}
#[test]
fn explicit_default_zero_false_and_empty_text_remain_distinct_from_absence() {
    for (kind, text, expected) in [
        ("number", "0", serde_json::json!(0.0)),
        ("boolean", "", serde_json::json!(false)),
        ("text", "", serde_json::json!("")),
    ] {
        let field = ProjectTemplateField {
            id: "f".into(),
            key: Some("f".into()),
            label: "F".into(),
            field_type: kind.into(),
            required: false,
            choices: vec![],
            target: None,
            target_entity_type: None,
            default: None,
            fields: vec![],
        };
        let mut input = FieldInput::new(&field);
        assert!(input.value().unwrap().default.is_none());
        input.has_default = true;
        input.default_text = text.into();
        assert_eq!(input.value().unwrap().default, Some(expected));
    }
}

#[test]
fn invalid_json_mode_changes_cancel_and_local_restore_keep_identity_reservations() {
    let mut state = ManagerState {
        editor: "valid previous document".into(),
        reserved_field_ids: vec!["field_1".into(), "group_1".into()],
        reserved_keys: vec!["property_1".into()],
        ..Default::default()
    };
    state.draft_history.push(state.editor.clone());
    state.editor = "{ invalid 原文".into();
    state.invalidate();
    state.mode = Mode::Json;
    state.mode = Mode::Design;
    assert!(state.request(Action::New).is_none());
    state.pending = None;
    let resumed: ProjectTemplateDraft =
        serde_json::from_slice(&serde_json::to_vec(&state.draft()).unwrap()).unwrap();
    assert_eq!(resumed.source_bytes, b"{ invalid \xe5\x8e\x9f\xe6\x96\x87");
    assert_eq!(resumed.reserved_field_ids, ["field_1", "group_1"]);
    assert_eq!(resumed.reserved_keys, ["property_1"]);
    state.editor = state.draft_history.pop().unwrap();
    state.invalidate();
    assert_eq!(state.draft().reserved_field_ids, resumed.reserved_field_ids);
    state.discard();
    assert!(state.draft().reserved_field_ids.is_empty());
}

#[test]
fn impact_paging_has_exact_bounds_and_invalidated_preview_resets_position() {
    for (total, requested, expected_page, expected) in [
        (0, 8, 0, 0..0),
        (1, 9, 0, 0..1),
        (50, 8, 0, 0..50),
        (51, 0, 0, 0..50),
        (51, 9, 1, 50..51),
        (103, 1, 1, 50..100),
        (103, 2, 2, 100..103),
    ] {
        let mut page = requested;
        assert_eq!(impact::page_range(total, &mut page), expected);
        assert_eq!(page, expected_page);
    }
    let mut state = ManagerState {
        impact_page: 7,
        ..Default::default()
    };
    state.invalidate();
    assert_eq!(state.impact_page, 0);
}
