use super::*;

#[test]
fn focus_navigation_command_toggles_drawer_without_changing_saved_navigation_preference() {
    for navigation in [false, true] {
        let ctx = egui::Context::default();
        let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        app.personal.settings.appearance.style = crate::theme::StylePreset::Focus;
        app.personal.settings.navigation = navigation;
        let baseline = app.project.content_baseline();
        app.execute_palette_action(&ctx, Action::Navigation);
        assert!(app.navigation_drawer_open(&ctx));
        assert_eq!(app.personal.settings.navigation, navigation);
        app.execute_palette_action(&ctx, Action::Navigation);
        assert!(!app.navigation_drawer_open(&ctx));
        assert_eq!(app.personal.settings.navigation, navigation);
        assert_eq!(app.project.content_baseline(), baseline);
    }
}

#[test]
fn ordinary_navigation_command_keeps_its_existing_preference_toggle() {
    let ctx = egui::Context::default();
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    app.personal.settings.appearance.style = crate::theme::StylePreset::Studio;
    let baseline = app.project.content_baseline();
    let navigation = app.personal.settings.navigation;
    app.execute_palette_action(&ctx, Action::Navigation);
    assert_eq!(app.personal.settings.navigation, !navigation);
    assert!(!app.navigation_drawer_open(&ctx));
    app.execute_palette_action(&ctx, Action::Navigation);
    assert_eq!(app.personal.settings.navigation, navigation);
    assert_eq!(app.project.content_baseline(), baseline);
}
