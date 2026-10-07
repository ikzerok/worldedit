use super::*;
use crate::app::WorldeditApp;
use worldline_core::presentation_commands::Command;
use worldline_core::vector_scene::{SceneGeometry, SceneNode, SceneOp};

fn fixture(name: &str) -> (PathBuf, egui::Context, WorldeditApp) {
    let root = tests_support::test_workspace(name);
    tests_support::enable_entities(&root);
    let path = root.join("world.wl");
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        format!("{source}\nentity first kind place\nentity second kind place\n"),
    )
    .unwrap();
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, Some(path));
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 900.0))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );
    app.map_canvas.set_mode(CanvasMode::Edit);
    (root, ctx, app)
}

fn point() -> MapGeometry {
    MapGeometry::Point(NormalizedPoint::new(0.4, 0.5))
}

fn create(app: &mut WorldeditApp) {
    let baseline = app.map_command_baseline("harbor");
    app.apply_map_intents(vec![EditIntent::Create(point())], vec![baseline]);
}

#[test]
fn changed_search_blocks_creation_save_and_retry_until_cancel_then_cannot_resurrect() {
    let (root, _ctx, mut app) = fixture("binding-cancel-create");
    app.select_marker_binding(TargetRef::new("entity", "first"));
    app.map_form.target_query = "second".into();
    let baseline = app.project.content_baseline();
    create(&mut app);
    assert!(app.map_form.target.is_none());
    assert_eq!(
        app.map_form.binding.pending(),
        Some(&TargetRef::new("entity", "first"))
    );
    assert!(app.map_failed_command.is_some());
    assert!(app.map_canvas.draft.is_some());
    assert!(!app.apply_map_command(
        "harbor",
        Command::UpdatePlacement {
            map_id: "harbor".into(),
            placement_id: "lighthouse".into(),
            geometry: None,
            target_ref: Some(None),
            annotation: Some("unsaved".into()),
            role: None,
            label_override: None,
            layer_id: None,
        },
        "must not apply"
    ));
    app.retry_failed_map_command();
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    app.map_canvas
        .edit_intents
        .push(EditIntent::Create(point()));
    app.map_canvas
        .intent_baselines
        .push(app.map_command_baseline("harbor"));
    app.cancel_map_form();
    assert!(!app.map_form.has_uncommitted_work());
    assert!(app.map_failed_command.is_none());
    assert!(app.map_canvas.draft.is_none());
    assert!(app.map_canvas.edit_intents.is_empty());
    assert!(app.map_canvas.intent_baselines.is_empty());
    assert!(!app.map_navigation_blocked());
    app.retry_failed_map_command();
    let (intents, baselines) = app.map_canvas.take_edit_batch();
    app.apply_map_intents(intents, baselines);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_binding_requires_reselect_or_explicit_no_reference_before_retry() {
    for omit in [false, true] {
        let (root, _ctx, mut app) = fixture(if omit {
            "binding-omit"
        } else {
            "binding-reselect"
        });
        app.select_marker_binding(TargetRef::new("entity", "first"));
        app.recompile();
        assert!(app.map_form.target.is_none());
        create(&mut app);
        assert!(app.map_failed_command.is_some());
        assert!(app.history.is_empty());
        if omit {
            app.omit_marker_binding();
        } else {
            app.select_marker_binding(TargetRef::new("entity", "second"));
        }
        app.retry_failed_map_command();
        assert!(app.map_failed_command.is_none(), "{:?}", app.io_error);
        assert_eq!(app.history.len(), 1);
        let map = &app.snapshot.as_ref().unwrap().map_index.maps["harbor"];
        let created = &map.placements["marker_1"];
        assert_eq!(
            created.target_ref,
            (!omit).then(|| TargetRef::new("entity", "second"))
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn loaded_author_binding_survives_source_change_and_missing_identity_is_not_unbound() {
    let (root, _ctx, mut app) = fixture("binding-loaded-author");
    app.map_form.target = Some(TargetRef::new("entity", "first"));
    app.map_form.editing_placement = Some("lighthouse".into());
    app.recompile();
    assert_eq!(app.map_form.target, Some(TargetRef::new("entity", "first")));
    assert!(app.map_form.binding.pending().is_none());
    let entry = app.project.entry.clone();
    let source = app.project.sources()[&entry].replace("entity first kind place\n", "");
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    assert_eq!(app.map_form.target, Some(TargetRef::new("entity", "first")));
    assert_eq!(app.map_form.binding.pending(), app.map_form.target.as_ref());
    assert!(!app.marker_binding_ready());
    let source = format!(
        "{}\nentity first kind place\n",
        app.project.sources()[&entry]
    );
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    assert_eq!(app.map_form.target, Some(TargetRef::new("entity", "first")));
    assert!(app.marker_binding_ready());
    app.cancel_map_form();
    assert!(!app.map_navigation_blocked());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn cancel_binding_form_preserves_unrelated_move_and_calibration_work() {
    for measurement in [false, true] {
        let (root, _ctx, mut app) = fixture(if measurement {
            "binding-keep-calibration"
        } else {
            "binding-keep-move"
        });
        let unrelated = if measurement {
            EditIntent::SetMeasurement(worldline_core::presentation::MapMeasurement {
                points: [[0.0, 0.0], [0.5, 0.0]],
                distance: 5.0,
                unit: "m".into(),
                extra: Default::default(),
            })
        } else {
            EditIntent::Move {
                placement: "lighthouse".into(),
                geometry: point(),
            }
        };
        app.map_canvas.restore_failed_preview(&unrelated);
        app.map_failed_command = Some(PendingMapCommand {
            map_id: "harbor".into(),
            intent: unrelated.clone(),
        });
        app.map_canvas.edit_intents = vec![EditIntent::Create(point()), unrelated.clone()];
        app.map_canvas.intent_baselines = vec![None, app.map_command_baseline("harbor")];
        app.select_marker_binding(TargetRef::new("entity", "first"));
        app.cancel_map_form();
        assert_eq!(app.map_failed_command.as_ref().unwrap().intent, unrelated);
        assert_eq!(app.map_canvas.edit_intents, vec![unrelated]);
        assert_eq!(app.map_canvas.intent_baselines.len(), 1);
        if measurement {
            assert!(app.map_canvas.measurement.calibration.is_some());
        } else {
            assert!(app.map_canvas.draft.is_some());
        }
        assert!(app.history.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn scene_stale_search_blocks_submission_and_cancel_removes_only_owned_retry_operations() {
    let (root, ctx, mut app) = fixture("binding-scene-guard");
    let mut node = SceneNode::new(
        "node",
        "places",
        SceneGeometry::Point {
            position: [0.4, 0.5],
        },
    );
    node.target_ref = Some(TargetRef::new("entity", "first"));
    app.map_canvas.scene.inspector = Some(node.clone());
    app.map_canvas.scene.binding_node = node.id.clone();
    app.map_canvas.scene.binding = binding::BindingGuard::Search {
        query: String::new(),
        version: app.version,
    };
    app.map_canvas.scene.inspector_dirty = true;
    app.map_canvas.scene.operations = vec![SceneOp::Insert {
        node: node.clone(),
        index: None,
    }];
    app.map_canvas.scene.binding_query = "second".into();
    app.submit_scene_operations(&ctx);
    assert!(app.map_canvas.scene.job.is_none());
    assert!(app.map_canvas.scene.binding.pending().is_some());
    assert!(app
        .map_canvas
        .scene
        .inspector
        .as_ref()
        .unwrap()
        .target_ref
        .is_none());
    let other = SceneNode::new(
        "other",
        "places",
        SceneGeometry::Point {
            position: [0.2, 0.5],
        },
    );
    app.map_canvas.scene.retry_operations = vec![
        SceneOp::Update { node },
        SceneOp::Update {
            node: other.clone(),
        },
    ];
    app.discard_scene_binding_submission("node");
    assert!(app.map_canvas.scene.operations.is_empty());
    assert_eq!(
        app.map_canvas.scene.retry_operations,
        vec![SceneOp::Update { node: other }]
    );
    assert!(app.history.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn scene_recompile_invalidates_only_search_pick_and_keeps_loaded_author_identity() {
    let (root, _ctx, mut app) = fixture("binding-scene-source");
    let mut node = SceneNode::new(
        "node",
        "places",
        SceneGeometry::Point {
            position: [0.4, 0.5],
        },
    );
    node.target_ref = Some(TargetRef::new("entity", "first"));
    app.map_canvas.scene.inspector = Some(node.clone());
    app.recompile();
    assert_eq!(
        app.map_canvas.scene.inspector.as_ref().unwrap().target_ref,
        node.target_ref
    );
    assert!(!app.map_canvas.scene.inspector_dirty);
    app.map_canvas.scene.binding = binding::BindingGuard::Search {
        query: String::new(),
        version: app.version,
    };
    app.recompile();
    assert!(app
        .map_canvas
        .scene
        .inspector
        .as_ref()
        .unwrap()
        .target_ref
        .is_none());
    assert_eq!(
        app.map_canvas.scene.binding.pending(),
        node.target_ref.as_ref()
    );
    assert!(app.map_canvas.scene.inspector_dirty);
    app.map_canvas.scene.binding = binding::BindingGuard::default();
    app.map_canvas.scene.inspector = Some(node.clone());
    app.map_canvas.scene.inspector_dirty = false;
    let entry = app.project.entry.clone();
    let source = app.project.sources()[&entry].replace("entity first kind place\n", "");
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    assert_eq!(
        app.map_canvas.scene.inspector.as_ref().unwrap().target_ref,
        node.target_ref
    );
    assert_eq!(
        app.map_canvas.scene.binding.pending(),
        node.target_ref.as_ref()
    );
    assert!(!app.map_canvas.scene.inspector_dirty);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn loaded_unresolved_scene_binding_can_edit_other_fields_but_cannot_change_to_missing_target() {
    use worldline_core::vector_scene::{preview_batch, Affine, SceneBatch};
    let (root, ctx, mut app) = fixture("binding-scene-unresolved-edit");
    let mut node = SceneNode::new(
        "node",
        "places",
        SceneGeometry::Point {
            position: [0.4, 0.5],
        },
    );
    node.target_ref = Some(TargetRef::new("entity", "first"));
    let baseline = app.map_command_baseline("harbor").unwrap();
    let plan = preview_batch(
        &app.project,
        app.map_revision,
        SceneBatch {
            map_id: "harbor".into(),
            expected_revision: baseline.revision,
            expected_documents: baseline.expected_documents,
            operations: vec![
                SceneOp::EnableScene,
                SceneOp::Insert {
                    node: node.clone(),
                    index: None,
                },
            ],
        },
    )
    .unwrap();
    app.apply_scene_plan(&plan).unwrap();
    app.map_canvas.scene.inspector = Some(node.clone());
    let entry = app.project.entry.clone();
    let source = app.project.sources()[&entry].replace("entity first kind place\n", "");
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    assert!(app.map_canvas.scene.binding.pending().is_some());
    assert!(!app.map_canvas.scene.binding.blocks_search_binding());
    node.name = "Renamed existing node".into();
    node.transform = Affine([1.0, 0.0, 0.0, 1.0, 10.0, 20.0]);
    app.map_canvas.scene.inspector = Some(node.clone());
    app.map_canvas.scene.inspector_dirty = true;
    app.map_canvas.scene.operations = vec![SceneOp::Update { node: node.clone() }];
    app.submit_scene_operations(&ctx);
    assert!(
        app.map_canvas.scene.job.is_some(),
        "{:?}",
        app.map_canvas.scene.error
    );
    for _ in 0..300 {
        app.poll_scene_operations();
        if app.map_canvas.scene.job.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(app.map_canvas.scene.job.is_none());
    assert!(
        app.map_canvas.scene.error.is_none(),
        "{:?}",
        app.map_canvas.scene.error
    );
    if let Some(plan) = app.map_canvas.scene.review_plan.take() {
        app.apply_scene_plan(&plan).unwrap();
    }
    let saved = &app.snapshot.as_ref().unwrap().map_index.maps["harbor"]
        .scene
        .as_ref()
        .unwrap()
        .nodes["node"];
    assert_eq!(saved.name, node.name);
    assert_eq!(saved.transform, node.transform);
    assert_eq!(saved.target_ref, node.target_ref);
    let baseline = app.map_command_baseline("harbor").unwrap();
    node.target_ref = Some(TargetRef::new("entity", "another_missing"));
    assert!(preview_batch(
        &app.project,
        app.map_revision,
        SceneBatch {
            map_id: "harbor".into(),
            expected_revision: baseline.revision,
            expected_documents: baseline.expected_documents,
            operations: vec![SceneOp::Update { node }],
        }
    )
    .is_err());
    std::fs::remove_dir_all(root).unwrap();
}
