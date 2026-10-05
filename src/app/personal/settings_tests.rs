use super::*;
#[test]
fn source_wrap_preference_defaults_off_and_survives_device_restore() {
    let old = PersonalState::decode(r#"{"schema_version":1,"settings":{"body_size":20}}"#).unwrap();
    assert!(!old.settings.source_wrap);
    let mut state = old;
    state.settings.source_wrap = true;
    let json = serde_json::to_string(&state).unwrap();
    let restored = PersonalState::decode(&json).unwrap();
    assert!(restored.settings.source_wrap);
    assert_eq!(restored.settings.body_size, 20.0);
    assert!(!json.contains("source_view"));
}

#[test]
fn personal_settings_are_bounded_versioned_and_content_free() {
    let state = PersonalState::decode(
        r#"{"schema_version":1,"settings":{"body_size":999,"line_spacing":0}}"#,
    )
    .unwrap();
    assert_eq!(state.settings.body_size, 28.0);
    assert_eq!(state.settings.line_spacing, 1.0);
    assert!(PersonalState::decode(r#"{"schema_version":2}"#).is_none());
    assert!(PersonalState::decode("broken").is_none());
    let json = serde_json::to_string(&state).unwrap();
    assert!(!json.contains("history"));
    assert!(!json.contains("draft"));
}
