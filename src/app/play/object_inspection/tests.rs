//! 从产品入口查询真实 Story；不把合成 egui 输出当物理 GUI/IME 证据。
#[path = "keyboard_tests.rs"]
mod keyboard_tests;
#[path = "native_ime_tests.rs"]
mod native_ime_tests;
use super::*;
use crate::app::Tab;
use worldline_core::project::Project;
use worldline_runtime::{InspectionCellStatus, ReplayTrace, Value};
const SOURCE: &str = "world room as \"同名\"\ncharacter hero as \"同名\"\ncharacter other as \"同名\"\nentity empty kind place as \"无状态地点\"\ntag calm\ntag alert\nstate mood on character hero with calm as \"情绪\"\nstate access on character hero with [] as \"通行\"\nstate other_mood on character other with calm\nlet hero_score = 9\nevent start with hero\n  become mood with alert\n  choice \"继续\"\n    become mood with calm\n    choice \"结束\"\n      -> END\n";
fn setup() -> (egui::Context, WorldeditApp) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let ctx = egui::Context::default();
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = std::env::temp_dir().join(format!(
        "object-inspection-ui-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    app.project = Project::new(&root);
    app.active_file = app.project.entry.clone();
    app.project
        .documents
        .retain(|path, _| path == &app.active_file);
    app.project
        .set_text(&app.active_file.clone(), SOURCE.into())
        .unwrap();
    app.project
        .create_authoring_document(
            &root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.12","required_features":[]}"#.to_vec(),
        )
        .unwrap();
    app.project.save().unwrap();
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.snapshot.as_ref().unwrap().result.diagnostics
    );
    (ctx, app)
}
fn start(app: &mut WorldeditApp) {
    app.start_play();
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .continue_story()
        .unwrap();
    app.tab = Tab::Catalog;
}
fn unchanged(app: &WorldeditApp) -> (String, ReplayTrace, String) {
    let story = app.play.as_ref().unwrap().story.as_ref().unwrap();
    (
        story.save().unwrap(),
        story.replay_trace(),
        app.project.content_baseline(),
    )
}
fn select(ctx: &egui::Context, app: &mut WorldeditApp, kind: &str, id: &str) {
    app.open_object_inspection(ctx, TargetRef::new(kind, id));
    app.select_object_run(RunKind::Ordinary);
    app.refresh_object_inspection(ctx);
}

#[test]
fn object_workspace_reset_discards_old_run_page_source_and_identity() {
    let (ctx, mut app) = setup();
    start(&mut app);
    select(&ctx, &mut app, "character", "hero");
    assert!(app.replay_debugger.object_inspection.page.is_some());
    app.reset_views();
    let state = &app.replay_debugger.object_inspection;
    assert!(!state.open);
    assert!(state.target.is_none());
    assert!(state.stamp.is_none());
    assert!(state.page.is_none());
    assert!(state.origin.is_none());
    assert!(app.play.is_none());
    assert!(app.object_rehearsal_snapshot().is_none());
}
#[test]
fn object_entry_never_starts_run_and_exact_owner_uses_runtime_catalog() {
    let (ctx, mut app) = setup();
    app.open_object_inspection(&ctx, TargetRef::new("character", "hero"));
    app.refresh_object_inspection(&ctx);
    assert!(app.play.is_none());
    assert!(app.replay_debugger.object_inspection.run.is_none());
    start(&mut app);
    let before = unchanged(&app);
    select(&ctx, &mut app, "character", "hero");
    let page = app.replay_debugger.object_inspection.page.as_ref().unwrap();
    assert_eq!(
        page.owner.as_ref().unwrap().target,
        TargetRef::new("character", "hero")
    );
    assert_eq!(page.total_items, 2);
    assert!(!page
        .items
        .iter()
        .any(|item| item.key.name == "other_mood" || item.key.name == "hero_score"));
    let item = page
        .items
        .iter()
        .find(|item| item.key.name == "mood")
        .unwrap();
    assert_eq!(item.first.value, Some(Value::TagSet(vec!["alert".into()])));
    assert_eq!(
        item.state_definition.as_ref().unwrap().initial.value,
        Some(Value::TagSet(vec!["calm".into()]))
    );
    assert_eq!(item.previous.status, InspectionCellStatus::Unrecorded);
    select(&ctx, &mut app, "entity", "empty");
    assert_eq!(
        app.replay_debugger
            .object_inspection
            .page
            .as_ref()
            .unwrap()
            .total_items,
        0
    );
    select(&ctx, &mut app, "world", "hero");
    assert!(app
        .replay_debugger
        .object_inspection
        .error
        .as_ref()
        .unwrap()
        .contains("INSPECTION_OWNER_NOT_FOUND"));
    assert_eq!(unchanged(&app), before);
}
#[test]
fn object_state_source_back_retains_owner_run_filter_selection_scroll_and_story() {
    let (ctx, mut app) = setup();
    start(&mut app);
    app.open_reading(TargetRef::new("character", "hero"));
    select(&ctx, &mut app, "character", "hero");
    app.replay_debugger.object_inspection.query.text = "mood".into();
    app.refresh_object_inspection(&ctx);
    let page = app.replay_debugger.object_inspection.page.as_ref().unwrap();
    let item = &page.items[0];
    let request = super::super::inspection::SourceRequest {
        stamp: page.stamp,
        key: item.key.clone(),
        source: item.source.clone().unwrap(),
    };
    app.replay_debugger.object_inspection.selected = Some(item.key.clone());
    app.replay_debugger.object_inspection.scroll = 42.0;
    let before = unchanged(&app);
    app.jump_to_object_inspection_source(&ctx, &request);
    assert!(
        !app.replay_debugger.object_inspection.open,
        "{:?}",
        app.replay_debugger.object_inspection.error
    );
    app.author_back(&ctx);
    let state = &app.replay_debugger.object_inspection;
    assert!(state.open);
    assert_eq!(state.query.text, "mood");
    assert_eq!(state.selected, Some(request.key));
    assert_eq!(state.scroll, 42.0);
    assert_eq!(state.stamp, Some(request.stamp));
    assert_eq!(unchanged(&app), before);
}
#[test]
fn old_object_page_never_rebinds_after_advancing_restarting_or_editing_source() {
    let (ctx, mut app) = setup();
    start(&mut app);
    select(&ctx, &mut app, "character", "hero");
    let old_page = app.replay_debugger.object_inspection.page.clone();
    let old_stamp = app.replay_debugger.object_inspection.stamp;
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .choose(0)
        .unwrap();
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .continue_story()
        .unwrap();
    app.refresh_object_inspection(&ctx);
    assert!(app.replay_debugger.object_inspection.run_changed);
    assert_eq!(app.replay_debugger.object_inspection.page, old_page);
    assert_eq!(app.replay_debugger.object_inspection.stamp, old_stamp);
    app.select_object_run(RunKind::Ordinary);
    app.refresh_object_inspection(&ctx);
    let page = app.replay_debugger.object_inspection.page.clone();
    app.project
        .set_text(&app.active_file.clone(), SOURCE.replace("情绪", "新稿名字"))
        .unwrap();
    app.recompile();
    app.refresh_object_inspection(&ctx);
    assert_eq!(app.replay_debugger.object_inspection.page, page);
    assert!(app
        .replay_debugger
        .object_inspection
        .source_reason
        .is_some());
    app.start_play();
    app.refresh_object_inspection(&ctx);
    assert!(app.replay_debugger.object_inspection.run_changed);
}
#[test]
fn owner_filter_pagination_and_read_only_window_use_frozen_stamp() {
    let (ctx, mut app) = setup();
    start(&mut app);
    select(&ctx, &mut app, "character", "hero");
    app.replay_debugger.object_inspection.query.limit = 1;
    app.refresh_object_inspection(&ctx);
    let first = app.replay_debugger.object_inspection.page.clone().unwrap();
    app.replay_debugger.object_inspection.query.offset = first.next_offset.unwrap();
    app.refresh_object_inspection(&ctx);
    let second = app.replay_debugger.object_inspection.page.clone().unwrap();
    assert_eq!(first.stamp, second.stamp);
    assert_ne!(first.items[0].key, second.items[0].key);
    let before = unchanged(&app);
    for size in [
        egui::vec2(1280.0, 760.0),
        egui::vec2(760.0, 540.0),
        egui::vec2(400.0, 300.0),
    ] {
        for _ in 0..3 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                    ..Default::default()
                },
                |ctx| app.object_inspection_window(ctx),
            );
        }
        assert_eq!(unchanged(&app), before);
        assert_eq!(
            app.replay_debugger
                .object_inspection
                .page
                .as_ref()
                .unwrap()
                .stamp,
            first.stamp
        );
    }
}

#[test]
fn object_window_escape_respects_top_layer_and_ime_keeps_underlying_author_input() {
    let (ctx, mut app) = setup();
    app.select_event("start");
    app.reading_target = Some(TargetRef::new("character", "hero"));
    app.open_object_inspection(&ctx, TargetRef::new("character", "hero"));
    let draft = app.event_editor.as_ref().unwrap().draft.clone();
    let escape = egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    app.command_palette.ime = true;
    let _ = ctx.run(
        egui::RawInput {
            events: vec![escape.clone()],
            ..Default::default()
        },
        |ctx| app.edit_shortcuts(ctx),
    );
    assert!(app.replay_debugger.object_inspection.open);
    app.command_palette.ime = false;
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: false,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ctx| app.edit_shortcuts(ctx),
    );
    let _ = ctx.run(
        egui::RawInput {
            events: vec![escape],
            ..Default::default()
        },
        |ctx| app.edit_shortcuts(ctx),
    );
    assert!(!app.replay_debugger.object_inspection.open);
    assert!(app.reading_target.is_some());
    assert!(app.event_editor.as_ref().unwrap().draft == draft);
}
