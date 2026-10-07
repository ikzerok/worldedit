use super::*;
use crate::theme::{BodyFamily, Density, StylePreset};
use egui::{Event, Key, Modifiers};
fn key(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    }
}
fn update(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) {
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1100.0, 780.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest());
        },
    );
}
#[test]
fn zoom_shortcuts_share_committed_and_preview_values_and_do_not_steal_ime_keys() {
    let ctx = egui::Context::default();
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    app.personal.settings.reduce_motion = true;
    for event in [
        egui::ImeEvent::Enabled,
        egui::ImeEvent::Preedit("中".into()),
        egui::ImeEvent::Commit("文".into()),
        egui::ImeEvent::Disabled,
    ] {
        app.ime_composing = false;
        app.command_palette.ime = false;
        update(&ctx, &mut app, vec![Event::Ime(event), key(Key::Plus)]);
        assert_eq!(app.personal.settings.ui_scale, 1.0);
    }
    app.ime_composing = true;
    update(&ctx, &mut app, vec![key(Key::Plus)]);
    assert_eq!(app.personal.settings.ui_scale, 1.0);
    app.ime_composing = false;
    app.command_palette.ime = true;
    update(&ctx, &mut app, vec![key(Key::Plus)]);
    assert_eq!(app.personal.settings.ui_scale, 1.0);
    app.command_palette.ime = false;
    update(&ctx, &mut app, vec![key(Key::Plus)]);
    assert_eq!(app.personal.settings.ui_scale, 1.1);
    app.personal.begin_preferences();
    update(&ctx, &mut app, vec![key(Key::Plus)]);
    assert_eq!(app.personal.appearance().ui_scale, 1.2);
    assert_eq!(app.personal.settings.ui_scale, 1.1);
    app.personal
        .preferences_action(preferences::PreferencesAction::Cancel);
    update(&ctx, &mut app, vec![]);
    assert_eq!(app.personal.appearance().ui_scale, 1.1);
    for _ in 0..15 {
        update(&ctx, &mut app, vec![key(Key::Plus)]);
    }
    assert_eq!(app.personal.settings.ui_scale, 2.0);
    update(&ctx, &mut app, vec![key(Key::Num0)]);
    assert_eq!(app.personal.settings.ui_scale, 1.0);
}
#[test]
fn preview_source_font_signature_and_cancel_preserve_source_selection_and_fingerprint() {
    for preview_style in [
        StylePreset::Technical,
        StylePreset::Focus,
        StylePreset::Ledger,
    ] {
        let ctx = egui::Context::default();
        let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        app.personal.settings.reduce_motion = true;
        app.personal.settings.source_wrap = true;
        app.personal.settings.navigation = false;
        app.tab = Tab::Edit;
        let source = format!(
            "event start\n  {}目标{}\n",
            "中文 Latin。".repeat(160),
            "继续写作。".repeat(50)
        );
        app.project
            .set_text(&app.active_file.clone(), source.clone())
            .unwrap();
        let fingerprint = app.project.content_baseline();
        for _ in 0..3 {
            update(&ctx, &mut app, vec![]);
        }
        let start = source.find("目标").unwrap();
        super::super::search::request_selection(
            &ctx,
            app.active_file.clone(),
            source.clone(),
            start..start + "目标".len(),
        );
        for _ in 0..4 {
            update(&ctx, &mut app, vec![]);
        }
        let id = egui::Id::new(("source", &app.active_file));
        let range = egui::TextEdit::load_state(&ctx, id)
            .unwrap()
            .cursor
            .char_range();
        app.personal.begin_preferences();
        app.personal.appearance_draft.as_mut().unwrap().source_size = 28.0;
        app.personal.appearance_draft.as_mut().unwrap().body_family = BodyFamily::Mono;
        app.personal.appearance_draft.as_mut().unwrap().style = preview_style;
        app.personal.appearance_draft.as_mut().unwrap().density = Density::Compact;
        for _ in 0..4 {
            update(&ctx, &mut app, vec![]);
        }
        let view = serde_json::to_value(&app.personal.source_view.as_ref().unwrap().1).unwrap();
        assert_eq!(view["layout"]["body_size"], 28.0);
        assert_eq!(app.personal.settings.source_size, 15.0);
        assert_eq!(
            egui::TextEdit::load_state(&ctx, id)
                .unwrap()
                .cursor
                .char_range(),
            range
        );
        app.personal
            .preferences_action(preferences::PreferencesAction::Cancel);
        for _ in 0..4 {
            update(&ctx, &mut app, vec![]);
        }
        let view = serde_json::to_value(&app.personal.source_view.as_ref().unwrap().1).unwrap();
        assert_eq!(view["layout"]["body_size"], 15.0);
        assert_eq!(
            egui::TextEdit::load_state(&ctx, id)
                .unwrap()
                .cursor
                .char_range(),
            range
        );
        assert_eq!(app.project.document(&app.active_file).unwrap(), source);
        assert_eq!(app.project.content_baseline(), fingerprint);
    }
}
