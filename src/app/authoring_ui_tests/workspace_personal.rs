use super::*;
use std::collections::BTreeMap;
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
fn view(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
    let mut frame = eframe::Frame::_new_kittest();
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1040.0, 660.0))),
            events,
            ..Default::default()
        },
        |ctx| eframe::App::update(app, ctx, &mut frame),
    )
}
#[test]
fn v080_world_read_only_visit_does_not_block_close_or_write() {
    let (ctx, mut app) = app();
    app.project.mark_saved();
    let before = app.project.content_baseline();
    app.tab = Tab::World;
    let _ = view(&ctx, &mut app, vec![]);
    assert!(app.world_editor.is_some());
    assert!(!app.has_open_authoring_form());
    app.request_action(Pending::Close, &ctx);
    assert!(app.allow_close);
    assert!(app.draft_action.is_none());
    assert_eq!(app.project.content_baseline(), before);
    assert!(app.history.is_empty());
}
#[test]
fn v080_actual_world_input_is_protected_and_cancel_preserves_it() {
    let (ctx, mut app) = app();
    app.project.mark_saved();
    app.tab = Tab::World;
    let _ = view(&ctx, &mut app, vec![]);
    app.world_editor.as_mut().unwrap().description = "不能丢失的中文草稿".into();
    let before = app.project.content_baseline();
    app.request_action(Pending::Close, &ctx);
    assert!(!app.allow_close);
    assert!(app.draft_action.is_some());
    for _ in 0..4 {
        let _ = view(&ctx, &mut app, vec![]);
    }
    let output = view(&ctx, &mut app, vec![]);
    let mut text = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut text);
    }
    assert!(text.contains("还有未应用的创作输入"), "{text}");
    assert!(text.contains("世界观"));
    app.draft_action = None;
    assert_eq!(
        app.world_editor.as_ref().unwrap().description,
        "不能丢失的中文草稿"
    );
    assert_eq!(app.project.content_baseline(), before);
}
#[test]
fn v080_clean_entity_form_is_not_a_dirty_draft() {
    let (ctx, mut app) = app();
    app.project.mark_saved();
    app.edit_entity(Some("a"));
    assert!(!app.has_open_authoring_form());
    app.entity_editor.as_mut().unwrap().draft.description = "输入".into();
    assert!(app.has_open_authoring_form());
    app.request_action(Pending::Close, &ctx);
    assert!(app.draft_action.is_some());
    app.discard_authoring_drafts();
    assert!(!app.has_open_authoring_form());
    assert!(!app.project.is_dirty());
}
#[test]
fn v080_existing_event_and_character_forms_compare_real_fields() {
    let (_, mut app) = app();
    let entry = app.project.entry.clone();
    app.project
        .set_text(
            &entry,
            "world bay\ncharacter lin as \"林\"\nevent arrival with lin\n  到达。\n  -> END\n"
                .into(),
        )
        .unwrap();
    app.recompile();
    app.select_event("arrival");
    assert!(!app.has_open_authoring_form());
    app.event_editor.as_mut().unwrap().draft.summary = "新摘要".into();
    assert!(app.has_open_authoring_form());
    app.event_editor = None;
    app.select_character("lin");
    assert!(!app.has_open_authoring_form());
    app.character_editor.as_mut().unwrap().draft.display = "林岑".into();
    assert!(app.has_open_authoring_form());
}
#[test]
fn v080_preferences_and_references_roundtrip_without_project_mutation() {
    let (ctx, mut app) = app();
    app.saved_location = true;
    let baseline = app.project.content_baseline();
    app.personal.settings.focus = true;
    app.personal.settings.body_size = 22.0;
    app.personal.settings.theme = crate::theme::ThemeMode::Light;
    app.tab = Tab::Catalog;
    app.catalog_target = Some(TargetRef::new("entity", "a"));
    app.reading_panels.pin(TargetRef::new("entity", "a"));
    app.reading_panels.pin(TargetRef::new("entity", "b"));
    app.capture_personal_view(&ctx);
    let mut storage = Storage::default();
    app.personal.save(&mut storage);
    app.personal = crate::app::personal::PersonalState::restore(Some(&storage));
    app.reading_panels.clear();
    app.tab = Tab::World;
    app.restore_personal_view(&ctx);
    assert_eq!(app.tab, Tab::Catalog);
    assert_eq!(app.reading_panels.ids().len(), 2);
    assert_eq!(app.personal.settings.body_size, 22.0);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    assert!(!storage.0.values().any(|text| text.contains("description")));
}
#[test]
fn v080_reference_dock_leaves_main_area_and_keeps_identity_at_minimum_width() {
    let (ctx, mut app) = app();
    app.personal.settings.navigation = false;
    app.reading_panels.pin(TargetRef::new("entity", "a"));
    app.reading_panels.pin(TargetRef::new("entity", "b"));
    let before = app.project.content_baseline();
    let mut central = 0.0;
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1040.0, 660.0))),
            ..Default::default()
        },
        |ctx| {
            app.docked_reading(ctx);
            central = ctx.available_rect().width();
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label("正文");
            });
        },
    );
    assert!(central >= 580.0, "{central}");
    assert_eq!(app.reading_panels.ids().len(), 2);
    assert_eq!(app.project.content_baseline(), before);
}
#[test]
fn v080_commands_do_not_consume_shortcuts_during_ime_composition() {
    let (ctx, mut app) = app();
    let _ = view(
        &ctx,
        &mut app,
        vec![
            Event::Ime(egui::ImeEvent::Preedit("中".into())),
            Event::Key {
                key: egui::Key::P,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
        ],
    );
    assert!(!app.command_palette.open);
    assert!(app.command_palette.ime);
    let _ = view(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Disabled)]);
    assert!(!app.command_palette.ime);
}
#[test]
fn v080_navigation_back_restores_complete_source_position() {
    let (ctx, mut app) = app();
    let original = app.active_file.clone();
    app.tab = Tab::Catalog;
    app.catalog_target = Some(TargetRef::new("entity", "a"));
    app.jump_to_file(&original.to_string_lossy(), 2, 1);
    assert_eq!(app.tab, Tab::Edit);
    app.author_back(&ctx);
    assert_eq!(app.tab, Tab::Catalog);
    assert_eq!(app.catalog_target, Some(TargetRef::new("entity", "a")));
}
#[test]
fn v080_light_dark_palettes_have_readable_text_and_restore() {
    let ctx = egui::Context::default();
    for mode in [
        crate::theme::ThemeMode::Dark,
        crate::theme::ThemeMode::Light,
    ] {
        let _theme = crate::theme::configure(&ctx, mode);
        fn luminance(c: egui::Color32) -> f32 {
            let v = |x: u8| {
                let n = x as f32 / 255.0;
                if n <= 0.04045 {
                    n / 12.92
                } else {
                    ((n + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * v(c.r()) + 0.7152 * v(c.g()) + 0.0722 * v(c.b())
        }
        let error_luminance = luminance(crate::theme::ERROR());
        let error_background = luminance(crate::theme::error_background());
        assert!(
            (error_luminance.max(error_background) + 0.05)
                / (error_luminance.min(error_background) + 0.05)
                >= 4.5
        );
        for background in [
            crate::theme::BG(),
            crate::theme::PANEL(),
            crate::theme::CARD(),
        ] {
            let b = luminance(background);
            for foreground in [crate::theme::TEXT(), crate::theme::MUTED()] {
                let a = luminance(foreground);
                assert!(
                    (a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5,
                    "{mode:?}: {foreground:?} / {background:?}"
                );
            }
        }
    }
    let _theme = crate::theme::configure(&ctx, crate::theme::ThemeMode::Dark);
}

#[test]
fn v080_ime_commit_and_enter_in_same_frame_cannot_execute_palette_action() {
    let (ctx, mut app) = app();
    app.open_commands(&ctx, true);
    app.command_palette.query = "世界观".into();
    let original = app.tab;
    let _ = view(
        &ctx,
        &mut app,
        vec![
            Event::Ime(egui::ImeEvent::Commit("观".into())),
            Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert!(app.command_palette.open);
    assert_eq!(app.tab, original);
    assert!(app.command_palette.ime_frame);
    let _ = view(&ctx, &mut app, vec![]);
    assert!(!app.command_palette.ime_frame);
}

#[test]
fn v080_system_theme_follows_reported_os_theme_without_project_edits() {
    let ctx = egui::Context::default();
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let _ = ctx.run(
            RawInput {
                system_theme: Some(theme),
                ..Default::default()
            },
            |ctx| {
                let _theme = crate::theme::configure(ctx, crate::theme::ThemeMode::System);
                assert_eq!(crate::theme::is_light(), theme == egui::Theme::Light);
            },
        );
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn v080_startup_recent_uses_one_profile_and_rejects_missing_project_without_creating() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    app.saved_location = true;
    app.capture_personal_view(&ctx);
    let mut storage = Storage::default();
    app.personal.save(&mut storage);
    let path = crate::app::startup::recent_project(Some(&storage)).unwrap();
    assert_eq!(path, app.project.entry);
    std::fs::remove_dir_all(&app.project.root).unwrap();
    assert!(crate::app::startup::recent_project(Some(&storage)).is_none());
    assert!(!app.project.root.exists());
    assert!(crate::app::startup::recent_project(Some(&Storage::default())).is_none());
}

#[test]
fn v080_navigation_to_another_form_preserves_unapplied_entity_input() {
    let (_, mut app) = app();
    app.edit_entity(Some("a"));
    app.entity_editor.as_mut().unwrap().draft.description = "原对象的中文草稿".into();
    app.edit_entity(Some("b"));
    let form = app.entity_editor.as_ref().unwrap();
    assert_eq!(form.original.as_deref(), Some("a"));
    assert_eq!(form.draft.description, "原对象的中文草稿");
}

#[test]
fn v080_new_event_and_character_defaults_are_clean_but_edits_are_protected() {
    let (ctx, mut app) = app();
    app.project.mark_saved();
    let baseline = app.project.content_baseline();
    app.new_event(None);
    assert!(!app.has_open_authoring_form());
    app.event_editor.as_mut().unwrap().draft.body = "真实输入".into();
    assert!(app.has_open_authoring_form());
    app.request_action(Pending::Close, &ctx);
    assert!(app.draft_action.is_some());
    assert!(!app.allow_close);
    app.discard_authoring_drafts();
    app.draft_action = None;
    app.new_character();
    assert!(!app.has_open_authoring_form());
    app.character_editor.as_mut().unwrap().draft.display = "新输入的名字".into();
    assert!(app.has_open_authoring_form());
    app.discard_authoring_drafts();
    app.new_character();
    assert!(!app.has_open_authoring_form());
    app.request_action(Pending::Close, &ctx);
    assert!(app.allow_close);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}
#[test]
fn v080_new_entity_and_relation_defaults_do_not_claim_saved_work() {
    let (_, mut app) = app();
    app.edit_entity(None);
    assert!(!app.has_open_authoring_form());
    app.entity_editor.as_mut().unwrap().draft.description = "已写正文".into();
    assert!(app.has_open_authoring_form());
    app.discard_authoring_drafts();
    app.edit_relation(None, None);
    assert!(!app.has_open_authoring_form());
    app.relation_editor.as_mut().unwrap().draft.description = "已写关系".into();
    assert!(app.has_open_authoring_form());
    app.discard_authoring_drafts();
    app.edit_relation_type(None);
    assert!(!app.has_open_authoring_form());
    app.relation_type_editor.as_mut().unwrap().draft.display = "新类型".into();
    assert!(app.has_open_authoring_form());
}

#[test]
fn v080_nested_form_render_does_not_rebase_a_temporarily_taken_draft() {
    let (_, mut app) = app();
    app.new_event(None);
    app.event_editor.as_mut().unwrap().draft.body = "已修改正文".into();
    let draft = app.event_editor.take();
    app.capture_new_draft_baselines();
    app.event_editor = draft;
    assert!(app.has_open_authoring_form());
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn v080_explicit_invalid_startup_does_not_fall_back_or_open_recent_project() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    app.saved_location = true;
    app.capture_personal_view(&ctx);
    let mut storage = Storage::default();
    app.personal.save(&mut storage);
    let missing = app.project.root.join("missing.wl");
    assert_eq!(
        crate::app::startup::preferred_project(Some(&storage), Some(missing.clone())),
        Some(missing)
    );
    let mut creation = eframe::CreationContext::_new_kittest(ctx.clone());
    creation.storage = Some(&storage);
    let cancelled = WorldeditApp::new(&creation, None);
    assert_ne!(cancelled.project.root, app.project.root);
    assert!(!cancelled.saved_location);
}

#[test]
fn v080_personal_reopen_restores_exact_entity_identity_as_clean_form() {
    let (ctx, mut app) = app();
    app.saved_location = true;
    let before = app.project.content_baseline();
    app.edit_entity(Some("b"));
    assert_eq!(app.entity_editor.as_ref().unwrap().draft.display, "同名");
    app.capture_personal_view(&ctx);
    let mut storage = Storage::default();
    app.personal.save(&mut storage);
    app.entity_editor = None;
    app.personal = crate::app::personal::PersonalState::restore(Some(&storage));
    app.restore_personal_view(&ctx);
    assert_eq!(
        app.entity_editor.as_ref().unwrap().original.as_deref(),
        Some("b")
    );
    assert!(!app.has_open_authoring_form());
    assert_eq!(app.project.content_baseline(), before);
    assert!(app.history.is_empty());
}
#[test]
fn v080_personal_reopen_restores_character_identity_without_source_changes() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    app.project
        .set_text(
            &entry,
            "character a as \"同名\"\ncharacter b as \"同名\"\nevent start\n  -> END\n".into(),
        )
        .unwrap();
    app.recompile();
    app.saved_location = true;
    app.tab = Tab::Characters;
    app.select_character("b");
    let before = app.project.content_baseline();
    app.capture_personal_view(&ctx);
    app.character_editor = None;
    app.restore_personal_view(&ctx);
    assert_eq!(
        app.character_editor.as_ref().unwrap().original.as_deref(),
        Some("b")
    );
    assert!(!app.has_open_authoring_form());
    assert_eq!(app.project.content_baseline(), before);
}

fn click_personal_view(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for _ in 0..3 {
        let _ = view(ctx, app, vec![]);
    }
    let output = view(ctx, app, vec![]);
    let point = visible_text_position(&output, label).unwrap_or_else(|| {
        let mut text = String::new();
        for shape in &output.shapes {
            collect_text(&shape.shape, &mut text);
        }
        panic!("没有可见控件 {label}: {text}");
    });
    for pressed in [true, false] {
        let _ = view(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}
#[test]
fn v080_full_catalog_with_reference_at_1040_uses_transient_navigation_and_index() {
    let (ctx, mut app) = app();
    app.tab = Tab::Catalog;
    app.catalog_filter.clear();
    app.personal.settings.navigation = true;
    app.personal.settings.reference_width = 440.0;
    for _ in 0..3 {
        let _ = view(&ctx, &mut app, vec![]);
    }
    let before = app.project.content_baseline();
    let navigation_width = app.personal.settings.navigation_width;
    app.reading_panels.pin(TargetRef::new("entity", "a"));
    for _ in 0..3 {
        let _ = view(&ctx, &mut app, vec![]);
    }
    let output = view(&ctx, &mut app, vec![]);
    let mut text = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut text);
    }
    assert!(
        text.contains("固定参考") && text.contains("资料索引（窄窗）"),
        "{text}"
    );
    assert!(
        !text.contains("世界资料索引") && !text.contains("写作与阅读"),
        "{text}"
    );
    let width = output
        .shapes
        .iter()
        .filter(|shape| text_position_contains(&shape.shape, "资料与状态").is_some())
        .map(|shape| shape.clip_rect.width())
        .fold(0.0, f32::max);
    assert!(width >= 480.0, "主编辑区只有 {width} 像素");
    assert!(app.personal.settings.navigation);
    assert_eq!(app.personal.settings.navigation_width, navigation_width);
    assert!(
        (app.personal.settings.reference_width - 440.0).abs() <= 1.0,
        "reference width={}",
        app.personal.settings.reference_width
    );
    click_personal_view(&ctx, &mut app, "资料索引（窄窗）");
    assert!(app.personal.catalog_drawer_open);
    let _ = view(
        &ctx,
        &mut app,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(!app.personal.catalog_drawer_open);
    click_personal_view(&ctx, &mut app, "资料索引（窄窗）");
    assert!(app.personal.catalog_drawer_open);
    let chosen = TargetRef::new("entity", "b");
    let caption = crate::app::object_picker::candidate_caption(
        app.snapshot
            .as_ref()
            .unwrap()
            .result
            .analysis
            .catalog
            .object(&chosen)
            .unwrap(),
        Some(&app.project.root),
    );
    assert!(
        caption.contains("同名 · 实体:b\n"),
        "完整同名对象身份必须可见：{caption}"
    );
    click_personal_view(&ctx, &mut app, &caption);
    assert!(!app.personal.catalog_drawer_open);
    assert_eq!(app.catalog_target, Some(TargetRef::new("entity", "b")));
    click_personal_view(&ctx, &mut app, "关闭参考");
    for _ in 0..3 {
        let _ = view(&ctx, &mut app, vec![]);
    }
    let output = view(&ctx, &mut app, vec![]);
    let mut text = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut text);
    }
    assert!(
        text.contains("世界资料索引") && text.contains("写作与阅读"),
        "{text}"
    );
    assert!(app.personal.settings.navigation);
    assert_eq!(app.personal.settings.navigation_width, navigation_width);
    assert_eq!(app.project.content_baseline(), before);
    assert!(app.history.is_empty());
}
