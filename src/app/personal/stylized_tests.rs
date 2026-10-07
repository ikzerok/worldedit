use super::*;
use crate::theme::{self, PaletteId, ThemeMode};
use preferences::PreferencesAction;

#[test]
fn every_new_palette_roundtrips_requested_mode_without_serializing_effective_mode() {
    for palette in [
        PaletteId::Terminal,
        PaletteId::Neon,
        PaletteId::Vellum,
        PaletteId::Sakura,
        PaletteId::Monochrome,
    ] {
        for requested in [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark] {
            let mut state = PersonalState::default();
            state.settings.palette = palette;
            state.settings.theme = requested;
            state.settings.reference_width = 378.0;
            state.settings.focus = true;
            state.last_project = Some(PathBuf::from("/kept/main.wl"));
            state.workspaces.insert(
                PathBuf::from("/kept"),
                WorkspaceView {
                    references: vec![TargetRef::new("entity", "原对象")],
                    location: Location {
                        cursor: Some(27),
                        ..Default::default()
                    },
                },
            );
            let json = serde_json::to_string(&state).unwrap();
            assert!(!json.contains("effective_mode"));
            let restored = PersonalState::decode(&json).unwrap();
            assert_eq!(restored.settings.appearance, state.settings.appearance);
            assert_eq!(restored.settings.theme, requested);
            assert_eq!(restored.settings.reference_width, 378.0);
            assert!(restored.settings.focus);
            assert_eq!(restored.last_project, state.last_project);
            assert_eq!(
                restored.workspaces[&PathBuf::from("/kept")].references[0].id,
                "原对象"
            );
            assert_eq!(
                restored.workspaces[&PathBuf::from("/kept")].location.cursor,
                Some(27)
            );
            assert_eq!(serde_json::to_string(&restored).unwrap(), json);
            assert_eq!(
                theme::resolve(restored.appearance(), None).effective_mode,
                palette.effective_mode(requested, None)
            );
        }
    }
}

#[test]
fn fixed_palette_preview_apply_cancel_and_reopen_preserve_mode_intent_and_latest_commit() {
    for palette in [PaletteId::Terminal, PaletteId::Neon, PaletteId::Vellum] {
        for requested in [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark] {
            for cancel in [
                PreferencesAction::Cancel,
                PreferencesAction::Escape,
                PreferencesAction::WindowClose,
            ] {
                let mut state = PersonalState::default();
                state.settings.theme = requested;
                let original = state.settings.appearance;
                state.begin_preferences();
                state.appearance_draft.as_mut().unwrap().palette = palette;
                assert_eq!(state.appearance().theme, requested);
                assert_eq!(
                    theme::resolve(state.appearance(), None).effective_mode,
                    palette.effective_mode(requested, None)
                );
                let during = serde_json::to_string(&state).unwrap();
                assert_eq!(
                    PersonalState::decode(&during).unwrap().settings.appearance,
                    original
                );
                state.preferences_action(cancel);
                assert_eq!(*state.appearance(), original);
                state.begin_preferences();
                state.appearance_draft.as_mut().unwrap().palette = palette;
                state.preferences_action(PreferencesAction::Apply);
                let committed = state.settings.appearance;
                assert_eq!(committed.theme, requested);
                state.appearance_draft.as_mut().unwrap().palette = PaletteId::Sakura;
                state.appearance_draft.as_mut().unwrap().theme = ThemeMode::Dark;
                state.preferences_action(cancel);
                assert_eq!(*state.appearance(), committed);
                assert!(state.appearance_draft.is_none());
                state.begin_preferences();
                assert_eq!(*state.appearance(), committed);
                state.appearance_draft.as_mut().unwrap().palette = PaletteId::Mist;
                state.preferences_action(PreferencesAction::Accept);
                assert_eq!(state.settings.theme, requested);
                assert_eq!(
                    theme::resolve(state.appearance(), Some(egui::Theme::Dark)).effective_mode,
                    PaletteId::Mist.effective_mode(requested, Some(egui::Theme::Dark))
                );
                assert!(!state.preferences_open);
            }
        }
    }
}

#[test]
fn new_palette_survives_local_repair_and_future_storage_stays_protected() {
    for palette in [
        PaletteId::Terminal,
        PaletteId::Neon,
        PaletteId::Vellum,
        PaletteId::Sakura,
        PaletteId::Monochrome,
    ] {
        let id = serde_json::to_string(&palette).unwrap();
        let raw = format!(
            r#"{{"schema_version":2,"settings":{{"appearance":{{"palette":{id},"mode":"Light","body_size":"bad","source_size":23,"accent":"unknown"}}}},"last_project":"/keep/main.wl"}}"#
        );
        let repaired = PersonalState::decode(&raw).unwrap();
        assert_eq!(repaired.settings.palette, palette);
        assert_eq!(repaired.settings.theme, ThemeMode::Light);
        assert_eq!(repaired.settings.body_size, 17.0);
        assert_eq!(repaired.settings.source_size, 23.0);
        assert_eq!(repaired.settings.accent, theme::AccentChoice::Palette);
        assert_eq!(repaired.last_project, Some(PathBuf::from("/keep/main.wl")));
        let future = raw.replace("\"schema_version\":2", "\"schema_version\":93");
        let protected = PersonalState::decode(&future).unwrap();
        assert_eq!(
            protected.protected_storage.as_deref(),
            Some(future.as_str())
        );
        assert_eq!(protected.settings.palette, palette);
        assert_eq!(protected.settings.theme, ThemeMode::Light);
        assert!(protected.storage_warning().is_some());
    }
}

#[test]
fn enum_objects_and_arrays_recover_locally_like_the_loading_shell() {
    for appearance in [
        r#"{"mode":{"Dark":null},"palette":{"Terminal":null},"style":{"Ledger":null},"density":{"Compact":null},"accent":{"Copper":null},"body_family":{"Mono":null},"source_size":23,"high_contrast":true}"#,
        r#"{"mode":["Dark"],"palette":["Terminal"],"style":["Ledger"],"density":["Compact"],"accent":["Copper"],"body_family":["Mono"],"source_size":23,"high_contrast":true}"#,
    ] {
        let raw = format!(
            r#"{{"schema_version":2,"settings":{{"navigation":false,"reference_width":379,"appearance":{appearance}}},"last_project":"/keep/main.wl"}}"#
        );
        let repaired = PersonalState::decode(&raw).unwrap();
        let defaults = theme::AppearancePreferences::default();
        assert_eq!(repaired.settings.theme, defaults.theme);
        assert_eq!(repaired.settings.palette, defaults.palette);
        assert_eq!(repaired.settings.style, defaults.style);
        assert_eq!(repaired.settings.density, defaults.density);
        assert_eq!(repaired.settings.accent, defaults.accent);
        assert_eq!(repaired.settings.body_family, defaults.body_family);
        assert_eq!(repaired.settings.source_size, 23.0);
        assert!(repaired.settings.high_contrast);
        assert!(!repaired.settings.navigation);
        assert_eq!(repaired.settings.reference_width, 379.0);
        assert_eq!(repaired.last_project, Some(PathBuf::from("/keep/main.wl")));
    }
    let legacy = PersonalState::decode(
        r#"{"schema_version":1,"settings":{"theme":{"Light":null},"body_size":21}}"#,
    )
    .unwrap();
    assert_eq!(legacy.settings.theme, ThemeMode::Dark);
    assert_eq!(legacy.settings.body_size, 21.0);
    let alias = PersonalState::decode(r#"{"schema_version":2,"settings":{"appearance":{"mode":{"Dark":null},"theme":"Light","palette":"Terminal"}}}"#).unwrap();
    assert_eq!(alias.settings.theme, ThemeMode::Light);
    assert_eq!(alias.settings.palette, PaletteId::Terminal);
    assert_eq!(
        theme::resolve(alias.appearance(), None).effective_mode,
        ThemeMode::Dark
    );
}
