use super::*;
use crate::theme::{AppearancePreferences, PaletteId, StylePreset, ThemeMode};
use eframe::Storage as _;
use preferences::PreferencesAction;
#[derive(Default)]
struct Storage(BTreeMap<String, String>);
impl eframe::Storage for Storage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }
    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.into(), value);
    }
    fn flush(&mut self) {}
}
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
    assert_eq!(restored.settings.source_size, 20.0);
    assert!(!json.contains("source_view"));
}
#[test]
fn personal_settings_are_bounded_versioned_and_content_free() {
    let state = PersonalState::decode(
        r#"{"schema_version":1,"settings":{"body_size":999,"line_spacing":0}}"#,
    )
    .unwrap();
    assert_eq!(state.settings.body_size, 36.0);
    assert_eq!(state.settings.line_spacing, 1.0);
    assert_eq!(state.schema_version, 2);
    assert!(PersonalState::decode(r#"{"schema_version":2}"#).is_some());
    assert!(PersonalState::decode("broken").is_none());
    let json = serde_json::to_string(&state).unwrap();
    assert!(!json.contains("history"));
    assert!(!json.contains("draft"));
}
#[test]
fn legacy_migration_preserves_author_identity_layout_and_explicit_mode_idempotently() {
    for mode in ["Light", "Dark", "System"] {
        let raw = format!(
            r#"{{"schema_version":1,"settings":{{"theme":"{mode}","body_size":22,"line_spacing":1.8,"reading_width":920,"navigation":false,"source_wrap":true,"focus":true,"navigation_width":244,"reference_width":378}},"last_project":"/novel/main.wl","workspaces":{{"/novel":{{"location":{{"file":"/novel/章节.wl","cursor":13,"source_secondary":4,"source_scroll":[2,88],"source_baseline":"original","target":{{"kind":"entity","id":"城"}}}},"references":[{{"kind":"entity","id":"城"}}]}}}}}}"#
        );
        let state = PersonalState::decode(&raw).unwrap();
        assert_eq!(format!("{:?}", state.settings.theme), mode);
        assert_eq!(state.settings.palette, PaletteId::Mist);
        assert_eq!(state.settings.style, StylePreset::Studio);
        assert_eq!(state.settings.body_size, 22.0);
        assert_eq!(state.settings.line_spacing, 1.8);
        assert_eq!(state.settings.reading_width, 920.0);
        assert!(!state.settings.navigation && state.settings.source_wrap && state.settings.focus);
        assert_eq!(state.settings.navigation_width, 244.0);
        assert_eq!(state.settings.reference_width, 378.0);
        assert_eq!(state.last_project, Some(PathBuf::from("/novel/main.wl")));
        let view = &state.workspaces[&PathBuf::from("/novel")];
        assert_eq!(view.location.cursor, Some(13));
        assert_eq!(view.location.source_scroll, [2.0, 88.0]);
        assert_eq!(view.references, vec![TargetRef::new("entity", "城")]);
        let once = serde_json::to_string(&state).unwrap();
        let twice = serde_json::to_string(&PersonalState::decode(&once).unwrap()).unwrap();
        assert_eq!(once, twice);
    }
}
#[test]
fn new_defaults_are_distinct_from_legacy_implicit_defaults() {
    let new = PersonalState::default();
    assert_eq!(new.settings.theme, ThemeMode::System);
    assert_eq!(
        (new.settings.body_size, new.settings.source_size),
        (17.0, 15.0)
    );
    assert_eq!(
        (new.settings.line_spacing, new.settings.reading_width),
        (1.65, 760.0)
    );
    let old = PersonalState::decode(r#"{"schema_version":1}"#).unwrap();
    assert_eq!(old.settings.theme, ThemeMode::Dark);
    assert_eq!(
        (
            old.settings.body_size,
            old.settings.line_spacing,
            old.settings.reading_width
        ),
        (16.0, 1.45, 840.0)
    );
}
#[test]
fn broken_appearance_fields_recover_locally_without_losing_neighbor_fields_or_author_data() {
    let state = PersonalState::decode(r#"{"schema_version":2,"last_project":"/keep/main.wl","settings":{"navigation":false,"reference_width":400,"appearance":{"mode":"Light","palette":"does-not-exist","style":"Technical","body_size":"NaN","source_size":23,"line_spacing":null,"reading_width":1100,"ui_scale":99}}}"#).unwrap();
    assert_eq!(state.settings.theme, ThemeMode::Light);
    assert_eq!(state.settings.palette, PaletteId::Mist);
    assert_eq!(state.settings.style, StylePreset::Technical);
    assert_eq!(state.settings.body_size, 17.0);
    assert_eq!(state.settings.source_size, 23.0);
    assert_eq!(state.settings.reading_width, 1100.0);
    assert_eq!(state.settings.ui_scale, 2.0);
    assert!(!state.settings.navigation);
    assert_eq!(state.settings.reference_width, 400.0);
    assert_eq!(state.last_project, Some(PathBuf::from("/keep/main.wl")));
    let mut p = AppearancePreferences {
        body_size: f32::NAN,
        source_size: f32::INFINITY,
        line_spacing: f32::NEG_INFINITY,
        ui_scale: f32::NAN,
        ..Default::default()
    };
    p.normalize();
    assert_eq!(p, AppearancePreferences::default());
}
#[test]
fn future_and_unreadable_storage_are_preserved_byte_for_byte_without_automatic_overwrite() {
    for raw in ["{broken", r#"{"schema_version":"unknown"}"#, " { \"schema_version\": 91, \"future\": [1,2,3], \"settings\": {\"appearance\":{\"mode\":\"Dark\"}} } "] {
        let mut storage = Storage::default();
        storage.set_string(KEY, raw.into());
        let mut state = PersonalState::restore(Some(&storage));
        assert_eq!(state.protected_storage.as_deref(), Some(raw));
        assert!(state.storage_notice.is_some());
        state.settings.body_size = 29.0;
        state.save(&mut storage);
        assert_eq!(storage.get_string(KEY).as_deref(), Some(raw));
    }
}
#[test]
fn preview_cancel_escape_and_close_do_not_commit_or_persist_drafts() {
    for action in [
        PreferencesAction::Cancel,
        PreferencesAction::Escape,
        PreferencesAction::WindowClose,
    ] {
        let mut state = PersonalState::default();
        let committed = state.settings.appearance;
        state.begin_preferences();
        state.appearance_draft.as_mut().unwrap().palette = PaletteId::Plum;
        assert_eq!(state.appearance().palette, PaletteId::Plum);
        assert_eq!(state.settings.appearance, committed);
        let mut storage = Storage::default();
        state.save(&mut storage);
        let restored = PersonalState::restore(Some(&storage));
        assert_eq!(restored.settings.appearance, committed);
        state.preferences_action(action);
        assert_eq!(*state.appearance(), committed);
        assert!(state.appearance_draft.is_none());
    }
}
#[test]
fn apply_then_edit_cancel_and_reset_preserve_latest_commit_and_nonappearance_state() {
    let mut state = PersonalState::default();
    state.settings.navigation = false;
    state.settings.focus = true;
    state.settings.source_wrap = true;
    state.history.push(Location {
        cursor: Some(9),
        ..Default::default()
    });
    state.workspaces.insert(
        PathBuf::from("/keep"),
        WorkspaceView {
            references: vec![TargetRef::new("entity", "kept")],
            ..Default::default()
        },
    );
    state.begin_preferences();
    state.appearance_draft.as_mut().unwrap().palette = PaletteId::Copper;
    state.preferences_action(PreferencesAction::Apply);
    state.appearance_draft.as_mut().unwrap().style = StylePreset::Technical;
    state.preferences_action(PreferencesAction::Cancel);
    assert_eq!(state.settings.palette, PaletteId::Copper);
    assert_eq!(state.settings.style, StylePreset::Studio);
    state.begin_preferences();
    state.reset_appearance_preview();
    assert_eq!(*state.appearance(), AppearancePreferences::default());
    state.preferences_action(PreferencesAction::Cancel);
    assert_eq!(state.settings.palette, PaletteId::Copper);
    assert!(!state.settings.navigation && state.settings.focus && state.settings.source_wrap);
    assert_eq!(state.history[0].cursor, Some(9));
    assert_eq!(
        state.workspaces[&PathBuf::from("/keep")].references[0].id,
        "kept"
    );
    for _ in 0..20 {
        state.begin_preferences();
        assert_eq!(*state.appearance(), state.settings.appearance);
        state.appearance_draft.as_mut().unwrap().body_size = 30.0;
        state.preferences_action(PreferencesAction::WindowClose);
    }
    state.begin_preferences();
    state.appearance_draft.as_mut().unwrap().style = StylePreset::Technical;
    state.preferences_action(PreferencesAction::Accept);
    let mut storage = Storage::default();
    state.save(&mut storage);
    assert_eq!(
        PersonalState::restore(Some(&storage)).settings.style,
        StylePreset::Technical
    );
}
