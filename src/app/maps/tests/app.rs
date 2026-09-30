use super::tests_support::*;
use super::*;
#[test]
fn app_renders_a_registered_map_from_the_core_snapshot_without_dirtying_project() {
    let root = test_workspace("registered-map");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.tab = super::super::Tab::Map;
    let source_before = app.project.sources();
    let dirty_before = app.project.is_dirty();

    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );

    assert_eq!(app.map_canvas.snapshot.layers.len(), 1);
    assert_eq!(app.map_canvas.snapshot.layers[0].placements.len(), 1);
    assert_eq!(app.map_canvas.map_id(), "harbor");
    assert_eq!(app.map_canvas.raster_bytes(), 4);
    app.map_canvas.camera.pan_by(vec2(27.0, -11.0));
    let camera_before_repaint = *app.map_canvas.camera();
    app.map_canvas.set_layer_visible("places", false);
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );
    assert_eq!(*app.map_canvas.camera(), camera_before_repaint);
    assert!(!app.map_canvas.layer_states()[0].1);
    assert_eq!(app.project.sources(), source_before);
    assert_eq!(app.project.is_dirty(), dirty_before);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn map_point_can_create_a_place_and_entrance_in_one_undo_step() {
    let root = test_workspace("place-and-entrance");
    enable_entities(&root);
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );
    app.map_canvas.set_mode(CanvasMode::Edit);
    app.map_canvas.set_tool(CanvasTool::Point);
    app.map_form.create_place_on_next_point = true;
    let history_before = app.history.len();
    let source_before = app.project.sources();
    let screen_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0));
    app.map_canvas
        .set_command_baseline(app.map_command_baseline("harbor"));
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.map_canvas.show(ui));
        },
    );
    let point = app
        .map_canvas
        .camera()
        .normalized_to_screen(pos2(0.7, 0.4), app.map_canvas.viewport());
    click_canvas(&ctx, &mut app.map_canvas, screen_rect, point, 1.0);
    let (intents, baselines) = app.map_canvas.take_edit_batch();
    assert_eq!(intents.len(), 1);
    app.apply_map_intents(intents, baselines);
    assert!(app.map_form.pending_place.is_some());
    assert_eq!(app.project.sources(), source_before);
    assert_eq!(app.history.len(), history_before);
    assert!(app.map_navigation_blocked());

    app.map_form.place_name = "新灯塔".into();
    app.map_form.place_description = "雾港北面的灯塔".into();
    assert!(app.commit_pending_place(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), history_before + 1);
    let snapshot = app.snapshot.as_ref().unwrap();
    let place = snapshot
        .result
        .analysis
        .catalog
        .entities
        .get("entity_1")
        .unwrap();
    assert_eq!(place.display, "新灯塔");
    let entrance = &snapshot.map_index.maps["harbor"].placements["marker_1"];
    assert_eq!(
        entrance.target_ref,
        Some(TargetRef::new("entity", "entity_1"))
    );
    app.undo(false);
    let snapshot = app.snapshot.as_ref().unwrap();
    assert!(!snapshot
        .result
        .analysis
        .catalog
        .entities
        .contains_key("entity_1"));
    assert!(!snapshot.map_index.maps["harbor"]
        .placements
        .contains_key("marker_1"));
    app.undo(true);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities
        .contains_key("entity_1"));
    assert!(app.save(), "{:?}", app.io_error);
    let reopened = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    let reopened_snapshot = reopened.snapshot.as_ref().unwrap();
    assert!(reopened_snapshot
        .result
        .analysis
        .catalog
        .entities
        .contains_key("entity_1"));
    assert_eq!(
        reopened_snapshot.map_index.maps["harbor"].placements["marker_1"].target_ref,
        Some(TargetRef::new("entity", "entity_1"))
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_stale_place_draft_keeps_its_input_without_an_orphan_entity() {
    let root = test_workspace("stale-place-draft");
    enable_entities(&root);
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );
    app.map_canvas.set_mode(CanvasMode::Edit);
    app.map_form.create_place_on_next_point = true;
    app.apply_map_intents(
        vec![EditIntent::Create(MapGeometry::Point(
            NormalizedPoint::new(0.7, 0.4),
        ))],
        vec![app.map_command_baseline("harbor")],
    );
    app.map_form.place_name = "待确认地点".into();
    app.project
        .set_text(
            &root.join("world.wl"),
            "world harbor as \"雾港\"\n  description \"另一版本\"\n".into(),
        )
        .unwrap();
    assert!(!app.commit_pending_place());
    assert_eq!(app.map_form.place_name, "待确认地点");
    assert!(app.map_form.pending_place.is_some());
    assert!(!app
        .project
        .compile()
        .analysis
        .catalog
        .entities
        .contains_key("entity_1"));
    assert_eq!(app.history.len(), 0);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn browse_mode_clicks_and_commands_do_not_write_or_create_history() {
    let root = test_workspace("browse-read-only");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.tab = super::super::Tab::Map;
    let frame = || RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
        ..Default::default()
    };
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    assert!(!app.map_canvas.is_edit_mode());
    let source_before = app.project.sources();
    let dirty_before = app.project.is_dirty();
    let history_before = app.history.len();
    let revision_before = app.map_revision;

    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
            events: vec![
                Event::PointerMoved(pos2(1000.0, 150.0)),
                Event::PointerButton {
                    pos: pos2(1000.0, 150.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));

    let applied = app.apply_map_command(
        "harbor",
        worldline_core::presentation_commands::Command::SetLayer {
            map_id: "harbor".into(),
            layer_id: "places".into(),
            title: None,
            visible_default: None,
            locked: Some(true),
            layer_order: None,
        },
        "不应写入",
    );
    assert!(!applied);
    assert_eq!(app.project.sources(), source_before);
    assert_eq!(app.project.is_dirty(), dirty_before);
    assert_eq!(app.history.len(), history_before);
    assert_eq!(app.map_revision, revision_before);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_conflicted_drag_keeps_its_preview_for_retry_or_cancel() {
    let root = test_workspace("map-conflict-preview");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.tab = super::super::Tab::Map;
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );
    app.map_canvas.set_mode(CanvasMode::Edit);
    let source_before = app.project.sources();
    let pending = EditIntent::Move {
        placement: "lighthouse".into(),
        geometry: MapGeometry::Point(NormalizedPoint::new(0.7, 0.8)),
    };
    app.apply_map_intents(
        vec![pending],
        vec![Some(MapCommandBaseline {
            revision: worldline_core::presentation_commands::Revision::default(),
            expected_documents: BTreeMap::new(),
        })],
    );
    assert!(app.map_failed_command.is_some());
    assert_eq!(app.project.sources(), source_before);
    assert!(app.map_canvas.draft.is_some());
    assert_eq!(
        app.map_canvas
            .selected_placement()
            .map(|placement| placement.geometry),
        Some(MapGeometry::Point(NormalizedPoint::new(0.7, 0.8)))
    );
    app.map_canvas.reset_local_preview();
    app.map_failed_command = None;
    assert!(app.map_canvas.draft.is_none());
    assert_eq!(app.project.sources(), source_before);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stale_preview_retries_against_current_revision_once() {
    let root = test_workspace("map-stale-retry");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.tab = super::super::Tab::Map;
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );
    app.map_canvas.set_mode(CanvasMode::Edit);
    let pending = EditIntent::Move {
        placement: "lighthouse".into(),
        geometry: MapGeometry::Point(NormalizedPoint::new(0.7, 0.8)),
    };
    let stale = MapCommandBaseline {
        revision: worldline_core::presentation_commands::Revision::default(),
        expected_documents: BTreeMap::new(),
    };
    let map_path = worldline_core::presentation_commands::map_document_path(&app.project, "harbor")
        .expect("map path");
    let map_before = app
        .project
        .authoring_document(&map_path)
        .expect("map document")
        .bytes()
        .to_vec();
    app.apply_map_intents(vec![pending], vec![Some(stale)]);
    assert!(app.map_failed_command.is_some());
    assert_eq!(app.history.len(), 0);
    let revision_after_failure = app.map_revision;
    app.recompile();
    assert_ne!(app.map_revision, revision_after_failure);

    app.retry_failed_map_command();

    assert!(app.map_failed_command.is_none());
    assert_eq!(app.history.len(), 1);
    assert_ne!(
        app.project
            .authoring_document(&map_path)
            .expect("map document")
            .bytes(),
        map_before.as_slice()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn retrying_after_switching_maps_keeps_failed_preview_and_project_unchanged() {
    let root = test_workspace("map-stale-retry-switch");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.tab = super::super::Tab::Map;
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );
    app.map_canvas.set_mode(CanvasMode::Edit);
    app.apply_map_intents(
        vec![EditIntent::Move {
            placement: "lighthouse".into(),
            geometry: MapGeometry::Point(NormalizedPoint::new(0.7, 0.8)),
        }],
        vec![Some(MapCommandBaseline {
            revision: worldline_core::presentation_commands::Revision::default(),
            expected_documents: BTreeMap::new(),
        })],
    );
    let source_before = app.project.sources();
    let history_before = app.history.len();
    app.map_selection = Some("another-map".into());

    app.retry_failed_map_command();

    assert_eq!(app.project.sources(), source_before);
    assert_eq!(app.history.len(), history_before);
    assert!(matches!(
        app.map_failed_command.as_ref(),
        Some(PendingMapCommand { map_id, .. }) if map_id == "harbor"
    ));
    assert!(app
        .io_error
        .as_deref()
        .is_some_and(|message| message.contains("返回该地图")));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn create_without_an_editable_layer_keeps_draft_for_retry() {
    let root = test_workspace("map-no-editable-layer");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.tab = super::super::Tab::Map;
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );
    app.map_canvas.set_mode(CanvasMode::Edit);
    for layer in &mut app.map_canvas.snapshot.layers {
        layer.locked = true;
    }
    let geometry = MapGeometry::Point(NormalizedPoint::new(0.6, 0.6));
    app.map_canvas.draft = Some(geometry.clone());
    app.map_canvas.finish_draft();
    let (intents, baselines) = app.map_canvas.take_edit_batch();
    app.apply_map_intents(intents, baselines);

    assert_eq!(app.map_canvas.draft, Some(geometry));
    assert!(matches!(
        app.map_failed_command.as_ref(),
        Some(PendingMapCommand {
            map_id,
            intent: EditIntent::Create(_),
            ..
        }) if map_id == "harbor"
    ));
    assert_eq!(app.io_error.as_deref(), Some("当前没有可编辑的未锁定图层"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn session_visibility_override_does_not_mask_persisted_default_after_undo() {
    let mut canvas = point_canvas();
    canvas.set_layer_visible("places", false);
    assert_eq!(canvas.layer_default_visibility("places"), Some(true));

    let mut persisted_hidden = canvas.core_snapshot.clone();
    persisted_hidden.layers[0].visible = false;
    canvas.set_snapshot(1, persisted_hidden);
    assert_eq!(canvas.layer_default_visibility("places"), Some(false));
    assert!(!canvas.layer_states()[0].1);

    // Undo restores the persisted default. It must also clear the browse
    // override so the next refresh cannot hide the layer again.
    canvas.set_layer_default_visible("places", true);
    assert_eq!(canvas.layer_default_visibility("places"), Some(true));
    assert!(canvas.layer_states()[0].1);
    let mut persisted_visible = canvas.core_snapshot.clone();
    persisted_visible.layers[0].visible = true;
    canvas.set_snapshot(2, persisted_visible);
    assert!(canvas.layer_states()[0].1);
}

#[test]
fn app_clears_the_previous_map_when_registration_disappears() {
    let root = test_workspace("map-removed");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.tab = super::super::Tab::Map;
    let frame = || RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
        ..Default::default()
    };
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    assert_eq!(app.map_canvas.map_id(), "harbor");

    std::fs::write(
        root.join(".world/project.json"),
        r#"{
  "schema_version": 1,
  "language_version": "1.9",
  "entry": "world.wl",
  "required_features": ["presentation.maps.v1"],
  "maps": {},
  "graph_views": {},
  "extensions": {}
}"#,
    )
    .expect("updated manifest");
    app.project.refresh().expect("refresh test workspace");
    app.recompile();
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));

    assert!(app.map_canvas.map_id().is_empty());
    assert!(app.map_canvas.snapshot.layers.is_empty());
    assert!(app.map_canvas.snapshot.raster_layers.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn shared_reference_navigation_keeps_project_and_reading_data_unchanged() {
    let root = test_workspace("shared-reference-navigation");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx);
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    let before = app.project.sources();
    let version = app.version;
    app.open_reading(worldline_core::catalog::TargetRef {
        kind: "world".into(),
        id: "harbor".into(),
    });
    app.locate_reference("harbor", "lighthouse");
    assert_eq!(app.tab, super::super::Tab::Map);
    assert_eq!(app.map_selection.as_deref(), Some("harbor"));
    let request = app.map_locate_request.as_ref().unwrap();
    assert_eq!(request.placement_id, "lighthouse");
    assert_eq!(request.layer_id, "places");
    assert!(app.reading_target.is_none());
    assert_eq!(app.project.sources(), before);
    assert_eq!(app.version, version);
    assert!(!app.project.is_dirty());
    assert!(app.history.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn app_surfaces_a_missing_raster_asset_without_writing_the_project() {
    let root = test_workspace("missing-raster");
    std::fs::remove_file(root.join("assets/harbor.png")).expect("remove test raster");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.tab = super::super::Tab::Map;
    let source_before = app.project.sources();
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );

    let states = app.map_canvas.raster_states();
    assert_eq!(states.len(), 1);
    assert!(!states[0].1);
    assert!(states[0]
        .2
        .as_deref()
        .is_some_and(|message| message.contains("素材")));
    assert_eq!(app.project.sources(), source_before);
    assert!(!app.project.is_dirty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn raster_budget_eviction_does_not_retry_reads_on_each_repaint() {
    let root = test_workspace("raster-budget");
    let second = root.join("assets/second.png");
    std::fs::write(&second, TEST_PNG).expect("second test raster");
    let mut canvas = MapCanvas::new(MapRenderSnapshot {
        map_id: "budget".into(),
        title: "预算测试".into(),
        extent: vec2(100.0, 100.0),
        canvas: core_canvas(100.0 as u32, 100.0 as u32),
        measurement: None,
        raster_layers: vec![
            RasterPlacement {
                asset_key: "first".into(),
                asset_path: Some(root.join("assets/harbor.png")),
                asset_available: true,
                rect: NormalizedRect {
                    min: NormalizedPoint::new(0.0, 0.0),
                    max: NormalizedPoint::new(0.5, 1.0),
                },
            },
            RasterPlacement {
                asset_key: "second".into(),
                asset_path: Some(second),
                asset_available: true,
                rect: NormalizedRect {
                    min: NormalizedPoint::new(0.5, 0.0),
                    max: NormalizedPoint::new(1.0, 1.0),
                },
            },
        ],
        layers: Vec::new(),
    });
    canvas.set_raster_budget(4);
    let ctx = egui::Context::default();
    canvas.prepare_rasters(&ctx, &root);
    assert_eq!(canvas.raster_attempt_count(), 2);
    assert_eq!(canvas.raster_bytes(), 4);
    canvas.prepare_rasters(&ctx, &root);
    assert_eq!(canvas.raster_attempt_count(), 2);
    assert_eq!(canvas.raster_bytes(), 4);
    let _ = std::fs::remove_dir_all(root);
}
