use super::*;
use egui::{pos2, vec2, Event, RawInput, Rect};
use std::sync::atomic::{AtomicUsize, Ordering};

mod performance;

fn fixture(objects: usize) -> (egui::Context, WorldeditApp) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = std::env::temp_dir().join(format!(
        "schema-snapshot-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).expect("测试工作区必须独占新建目录");
    app.project = Project::new(&root);
    let path = app.project.entry.clone();
    app.project.documents.retain(|file, _| file == &path);
    app.project.create_authoring_document(&root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.13","required_features":[],"maps":{},"graph_views":{}}"#.to_vec()).unwrap();
    let mut source = String::from(
        "schema city for entity entity_type place\n  field population_id population number required\nschema person for character\n  field name_id name text required\nbind character object_0 to person\ncharacter object_0\n  property name = \"\"\n",
    );
    for id in 0..objects {
        source.push_str(&format!("bind entity object_{id} to city\nentity object_{id} kind place\n  property population = 0\n"));
    }
    source.push_str("event start\n  正文。\n  -> END\n");
    app.project.set_text(&path, source).unwrap();
    app.active_file = path;
    app.recompile();
    app.open_schema_editor(&ctx);
    (ctx, app)
}

fn frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 900.0))),
            events,
            ..Default::default()
        },
        |ctx| app.schema_editor_window(ctx),
    )
}

fn text_position(shape: &egui::Shape, needle: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text.starts_with(needle) => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| text_position(shape, needle)),
        _ => None,
    }
}

fn toggle_index(ctx: &egui::Context, app: &mut WorldeditApp) {
    let output = frame(ctx, app, vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position(&shape.shape, "当前约束索引"))
        .expect("索引折叠标题必须可达");
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

#[test]
fn schema_idle_scroll_collapse_and_appearance_frames_never_recompile() {
    let (ctx, mut app) = fixture(3);
    let before = app.semantic_compile_count;
    let index = app.current_schema_index().unwrap();
    for _ in 0..120 {
        frame(&ctx, &mut app, vec![]);
    }
    toggle_index(&ctx, &mut app);
    for round in 0..120 {
        frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos2(500.0, 450.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, if round % 2 == 0 { -20.0 } else { 20.0 }),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    toggle_index(&ctx, &mut app);
    for _ in 0..120 {
        frame(&ctx, &mut app, vec![]);
    }
    for style in [
        crate::theme::StylePreset::Studio,
        crate::theme::StylePreset::Manuscript,
        crate::theme::StylePreset::Technical,
        crate::theme::StylePreset::Focus,
        crate::theme::StylePreset::Ledger,
    ] {
        for palette in crate::theme::PaletteId::ALL {
            for mode in [
                crate::theme::ThemeMode::Light,
                crate::theme::ThemeMode::Dark,
            ] {
                app.personal.settings.appearance.style = style;
                app.personal.settings.appearance.palette = palette;
                app.personal.settings.appearance.theme = mode;
                let _theme = crate::theme::configure_appearance(&ctx, app.personal.appearance());
                frame(&ctx, &mut app, vec![]);
            }
        }
    }
    assert_eq!(app.semantic_compile_count, before);
    assert!(Arc::ptr_eq(&index, &app.current_schema_index().unwrap()));
    assert!(!include_str!("../schema_ui.rs").contains(".schema_index()"));
    assert!(!include_str!("../schema_ui.rs").contains(".compile("));
}

#[test]
fn complete_target_identity_and_warm_switches_reuse_one_snapshot() {
    let (_, app) = fixture(2);
    let character = TargetRef::new("character", "object_0");
    let entity = TargetRef::new("entity", "object_0");
    let other = TargetRef::new("entity", "object_1");
    let original = app.current_schema_authoring_shared(&entity).unwrap();
    let before = app.semantic_compile_count;
    for _ in 0..120 {
        for target in [&character, &entity, &other] {
            let projection = app.current_schema_authoring_shared(target).unwrap();
            assert_eq!(&projection.target, target);
        }
    }
    let snapshot = &app.snapshot.as_ref().unwrap().schema;
    assert_eq!(snapshot.projection_builds.get(), 3);
    assert!(Arc::ptr_eq(
        &original,
        &app.current_schema_authoring_shared(&entity).unwrap()
    ));
    assert_eq!(app.semantic_compile_count, before);
}

#[test]
fn stale_baseline_and_workspace_cannot_republish_cached_projection() {
    let (_, mut app) = fixture(2);
    let target = TargetRef::new("entity", "object_0");
    app.current_schema_authoring(&target).unwrap();
    let old = app.snapshot.take().unwrap();
    let original = app.project.document(&app.active_file).unwrap().to_owned();
    app.project
        .set_text(
            &app.active_file,
            original.replace("population = 0", "population = 1"),
        )
        .unwrap();
    assert!(old.schema.index(&app.project).unwrap_err().contains("过期"));
    assert!(old
        .schema
        .projection(&app.project, &old.result, &target)
        .unwrap_err()
        .contains("过期"));
    app.snapshot = Some(old);
    app.refresh_presentation_after_map_command();
    assert!(app.current_schema_authoring(&target).is_err());
    let old = app.snapshot.take().unwrap();
    app.recompile();
    let fresh = app.current_schema_authoring(&target).unwrap();
    assert_eq!(fresh.baseline, app.project.content_baseline());
    assert!(old
        .schema
        .projection(&app.project, &old.result, &target)
        .is_err());
    let mut other_workspace = app.project.clone();
    other_workspace.root = other_workspace.root.join("different-workspace");
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .schema
        .index(&other_workspace)
        .is_err());
}

#[test]
fn cache_is_bounded_and_read_errors_do_not_repeat_projection_work() {
    let (_, app) = fixture(MAX_CACHED_PROJECTIONS + 2);
    for index in 0..MAX_CACHED_PROJECTIONS + 2 {
        app.current_schema_authoring(&TargetRef::new("entity", &format!("object_{index}")))
            .unwrap();
    }
    let snapshot = &app.snapshot.as_ref().unwrap().schema;
    assert_eq!(snapshot.projections.borrow().len(), MAX_CACHED_PROJECTIONS);
    let missing = TargetRef::new("entity", "missing");
    for _ in 0..120 {
        assert!(app.current_schema_authoring(&missing).is_err());
    }
    assert_eq!(
        snapshot.projection_builds.get(),
        MAX_CACHED_PROJECTIONS as u64 + 3
    );
}

#[test]
fn presentation_rebase_reuses_index_but_rebuilds_transaction_baseline() {
    let (_, mut app) = fixture(1);
    let manifest = app.project.root.join(".world/project.json");
    app.project
        .set_authoring_document(
            &manifest,
            br#"{"schema_version":1,"language_version":"1.13","required_features":["presentation.maps.v1"],"maps":{"atlas":".world/maps/atlas.json"},"graph_views":{}}"#.to_vec(),
        )
        .unwrap();
    let map_path = app.project.root.join(".world/maps/atlas.json");
    let mut map = serde_json::json!({
        "schema_version": 1, "id": "atlas", "title": "初始地图",
        "raster_layers": [], "canvas": {"width": 1000, "height": 800, "unit": "normalized"},
        "layer_order": [], "layers": {}, "placements": {}, "extensions": {}
    });
    app.project
        .create_authoring_document(&map_path, serde_json::to_vec(&map).unwrap())
        .unwrap();
    app.recompile();
    let target = TargetRef::new("entity", "object_0");
    let before = app.current_schema_authoring(&target).unwrap();
    let index = app.current_schema_index().unwrap();
    let compiles = app.semantic_compile_count;
    map["title"] = "修改地图标题".into();
    app.project
        .set_authoring_document(&map_path, serde_json::to_vec(&map).unwrap())
        .unwrap();
    app.refresh_presentation_after_map_command();
    let after = app.current_schema_authoring(&target).unwrap();
    assert_ne!(before.baseline, after.baseline);
    assert_eq!(after.baseline, app.project.content_baseline());
    assert!(Arc::ptr_eq(&index, &app.current_schema_index().unwrap()));
    assert_eq!(app.semantic_compile_count, compiles);
    assert_eq!(
        app.snapshot.as_ref().unwrap().map_index.maps["atlas"].title,
        "修改地图标题"
    );
}

#[test]
fn snapshot_refresh_keeps_text_focus_and_cursor() {
    let (ctx, mut app) = fixture(1);
    frame(&ctx, &mut app, vec![]);
    let id = egui::Id::new("schema-source-editor");
    let mut state = egui::TextEdit::load_state(&ctx, id).unwrap();
    let cursor = egui::text::CCursorRange::one(egui::text::CCursor::new(7));
    state.cursor.set_char_range(Some(cursor));
    egui::TextEdit::store_state(&ctx, id, state);
    ctx.memory_mut(|memory| memory.request_focus(id));
    let source = app.project.document(&app.active_file).unwrap().to_owned();
    app.project
        .set_text(&app.active_file, source + "// 外改诊断刷新\n")
        .unwrap();
    app.recompile();
    frame(&ctx, &mut app, vec![]);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    assert_eq!(
        egui::TextEdit::load_state(&ctx, id)
            .unwrap()
            .cursor
            .char_range(),
        Some(cursor)
    );
}
