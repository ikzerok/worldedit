use super::tests_support::*;
use super::*;
use worldline_core::vector_scene::{
    Affine, MapScene, PathSegment, SceneGeometry, SceneNode, SceneOp,
};

fn canvas_scene(scene: MapScene) -> MapCanvas {
    let mut canvas = point_canvas();
    canvas.sync_scene(Some(&scene));
    canvas.set_mode(CanvasMode::Edit);
    canvas.viewport = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 400.0));
    canvas.fit_pending = false;
    canvas
}

fn rectangle(id: &str, x: f64) -> SceneNode {
    SceneNode::new(
        id,
        "places",
        SceneGeometry::Rect {
            x,
            y: 10.0,
            width: 40.0,
            height: 40.0,
            rx: 0.0,
            ry: 0.0,
        },
    )
}

#[test]
fn open_and_closed_curves_take_their_creation_styles_without_flattening() {
    let mut canvas = canvas_scene(MapScene::new(400.0, 400.0));
    canvas.tool = CanvasTool::Bezier;
    canvas.scene.style.fill = Some("#123456".into());
    canvas.scene.style.stroke = Some("#abcdef".into());
    for closed in [false, true] {
        canvas.scene.close_path = closed;
        canvas.scene_add_anchor([10.0, 10.0], false);
        canvas.scene.outgoing = Some([40.0, 5.0]);
        canvas.scene_add_anchor([80.0, 80.0], false);
        canvas.scene_finish_path();
        let SceneOp::Insert { node, .. } = canvas.scene.operations.pop().unwrap() else {
            panic!("insert expected");
        };
        let SceneGeometry::Path { segments } = &node.geometry else {
            panic!("path was flattened");
        };
        assert!(matches!(
            segments[1],
            PathSegment::Cubic {
                control1: [40.0, 5.0],
                ..
            }
        ));
        assert_eq!(matches!(segments.last(), Some(PathSegment::Close)), closed);
        assert_eq!(
            node.style.fill.as_deref(),
            Some(if closed { "#123456" } else { "none" })
        );
        assert_eq!(node.style.stroke.as_deref(), Some("#abcdef"));
    }
}

#[test]
fn multi_subpath_creation_keeps_each_close_and_fill_rule() {
    let mut canvas = canvas_scene(MapScene::new(400.0, 400.0));
    canvas.tool = CanvasTool::Bezier;
    canvas.scene.close_path = true;
    canvas.scene.style.fill_rule = Some("evenodd".into());
    for (point, separate) in [
        ([10.0, 10.0], false),
        ([80.0, 80.0], false),
        ([25.0, 25.0], true),
        ([50.0, 50.0], false),
    ] {
        canvas.scene_add_anchor(point, separate);
    }
    canvas.scene_finish_path();
    let SceneOp::Insert { node, .. } = &canvas.scene.operations[0] else {
        panic!("insert expected");
    };
    let SceneGeometry::Path { segments } = &node.geometry else {
        panic!("path expected");
    };
    assert_eq!(
        segments
            .iter()
            .filter(|segment| matches!(segment, PathSegment::Move { .. }))
            .count(),
        2
    );
    assert_eq!(
        segments
            .iter()
            .filter(|segment| matches!(segment, PathSegment::Close))
            .count(),
        2
    );
    assert_eq!(node.style.fill_rule.as_deref(), Some("evenodd"));
}

#[test]
fn canvas_hit_respects_rotated_typed_viewport_clip() {
    let mut scene = MapScene::new(400.0, 400.0);
    let mut root = SceneNode::new(
        "root",
        "places",
        SceneGeometry::Group {
            children: vec!["rect".into()],
        },
    );
    root.transform = Affine([0.0, 1.0, -1.0, 0.0, 100.0, 0.0]);
    root.clip_rect = Some([0.0, 0.0, 40.0, 40.0]);
    root.extra.insert("svg_root".into(), true.into());
    let mut child = rectangle("rect", 0.0);
    child.parent_id = Some("root".into());
    child.geometry = SceneGeometry::Rect {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
        rx: 0.0,
        ry: 0.0,
    };
    scene
        .root_order
        .insert("places".into(), vec!["root".into()]);
    scene.nodes.insert("root".into(), root);
    scene.nodes.insert("rect".into(), child);
    let canvas = canvas_scene(scene);
    let inside = canvas.scene_to_screen([80.0, 20.0]).unwrap();
    let cropped = canvas.scene_to_screen([80.0, 60.0]).unwrap();
    assert_eq!(canvas.scene_hit(inside).as_deref(), Some("rect"));
    assert_eq!(canvas.scene_hit(cropped), None);
}

#[test]
fn hidden_ancestor_reveal_never_becomes_inspector_source_truth() {
    let mut scene = MapScene::new(400.0, 400.0);
    let mut root = SceneNode::new(
        "root",
        "places",
        SceneGeometry::Group {
            children: vec!["rect".into()],
        },
    );
    root.visible = false;
    let mut child = rectangle("rect", 20.0);
    child.parent_id = Some("root".into());
    scene
        .root_order
        .insert("places".into(), vec!["root".into()]);
    scene.nodes.insert("root".into(), root);
    scene.nodes.insert("rect".into(), child);
    let mut canvas = canvas_scene(scene);
    assert!(!canvas.placement_layer("rect").unwrap().1);
    canvas.reveal_scene_node_for_session("rect");
    assert!(canvas.select_placement_id("root"));
    assert!(canvas.scene.layers["places"].scene.nodes["root"].visible);
    assert!(!canvas.scene.source.as_ref().unwrap().nodes["root"].visible);
    assert!(!canvas.scene.inspector.as_ref().unwrap().visible);
}

#[test]
fn five_thousand_object_rows_are_virtualized_without_cloning_scene_per_frame() {
    let mut scene = MapScene::new(400.0, 400.0);
    for i in 0..5000 {
        let id = format!("rect_{i}");
        scene
            .root_order
            .entry("places".into())
            .or_default()
            .push(id.clone());
        scene
            .nodes
            .insert(id.clone(), rectangle(&id, (i % 100) as f64));
    }
    let mut canvas = canvas_scene(scene);
    let ctx = egui::Context::default();
    let output = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(320.0, 700.0))),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.scene_tree_panel(ui));
        },
    );
    assert_eq!(canvas.scene.tree_rows.len(), 5000);
    assert!(
        output.shapes.len() < 800,
        "hidden rows should not generate widgets"
    );
    assert!(!canvas.scene.tree_dirty);
}

#[test]
fn discarded_draft_retains_nontrivial_core_viewport_mapping() {
    let mut scene = MapScene::new(400.0, 400.0);
    scene.view_box = [20.0, 10.0, 40.0, 80.0];
    let mut canvas = canvas_scene(scene);
    let before = canvas.scene_to_screen([20.0, 10.0]).unwrap();
    canvas.scene.path.push(PathSegment::Move { to: [1.0, 1.0] });
    canvas.discard_local_work();
    assert!(canvas.scene.path.is_empty());
    assert_eq!(canvas.scene_to_screen([20.0, 10.0]).unwrap(), before);
}

fn paint(ctx: &egui::Context, app: &mut super::super::WorldeditApp) {
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1100.0, 720.0))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn batch(
    ctx: &egui::Context,
    app: &mut super::super::WorldeditApp,
    operations: Vec<SceneOp>,
    review: bool,
) {
    app.map_canvas.scene.operations = operations;
    app.map_canvas.scene.review_requested = review;
    app.submit_scene_operations(ctx);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.map_canvas.scene.job.is_some() {
        app.poll_scene_operations();
        assert!(
            std::time::Instant::now() < deadline,
            "scene preview timeout"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    paint(ctx, app);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn asynchronous_curve_apply_undo_save_reopen_and_svg_exchange_share_one_truth() {
    let root = test_workspace("scene-author-loop");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    paint(&ctx, &mut app);
    app.map_canvas.set_mode(CanvasMode::Edit);
    let history = app.history.len();
    let curve = SceneNode::new(
        "curve",
        "places",
        SceneGeometry::Path {
            segments: vec![
                PathSegment::Move { to: [10.0, 10.0] },
                PathSegment::Cubic {
                    control1: [30.0, 2.0],
                    control2: [50.0, 80.0],
                    to: [90.0, 90.0],
                },
                PathSegment::Close,
            ],
        },
    );
    batch(
        &ctx,
        &mut app,
        vec![
            SceneOp::EnableScene,
            SceneOp::Insert {
                node: curve,
                index: None,
            },
        ],
        false,
    );
    assert!(
        app.map_canvas.scene.error.is_none(),
        "{:?}",
        app.map_canvas.scene.error
    );
    assert_eq!(app.history.len(), history + 1);
    assert!(app.map_canvas.scene.selection.contains("curve"));
    let map = &app.snapshot.as_ref().unwrap().map_index.maps["harbor"];
    let svg = worldline_core::vector_scene::map_to_safe_svg(map, None).unwrap();
    let roundtrip = worldline_core::svg_import::preview_scene(&svg).unwrap();
    assert!(roundtrip.scene.nodes.values().any(|node| matches!(&node.geometry, SceneGeometry::Path { segments }
        if segments.iter().any(|segment| matches!(segment, PathSegment::Cubic { .. })) && matches!(segments.last(), Some(PathSegment::Close)))));
    app.undo(false);
    assert!(app.snapshot.as_ref().unwrap().map_index.maps["harbor"]
        .scene
        .is_none());
    app.undo(true);
    assert!(app.save(), "{:?}", app.io_error);
    let reopened = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    assert!(reopened.snapshot.as_ref().unwrap().map_index.maps["harbor"]
        .scene
        .as_ref()
        .unwrap()
        .nodes
        .contains_key("curve"));
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn non_contiguous_group_warns_and_waits_for_explicit_confirmation() {
    let root = test_workspace("scene-group-review");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    paint(&ctx, &mut app);
    app.map_canvas.set_mode(CanvasMode::Edit);
    batch(
        &ctx,
        &mut app,
        vec![
            SceneOp::EnableScene,
            SceneOp::Insert {
                node: rectangle("a", 1.0),
                index: None,
            },
            SceneOp::Insert {
                node: rectangle("b", 10.0),
                index: None,
            },
            SceneOp::Insert {
                node: rectangle("c", 20.0),
                index: None,
            },
        ],
        false,
    );
    let before = app.project.content_baseline();
    let history = app.history.len();
    batch(
        &ctx,
        &mut app,
        vec![SceneOp::Group {
            group_id: "group".into(),
            node_ids: vec!["a".into(), "c".into()],
            name: "组合".into(),
        }],
        false,
    );
    assert_eq!(app.project.content_baseline(), before);
    assert_eq!(app.history.len(), history);
    let plan = app
        .map_canvas
        .scene
        .review_plan
        .take()
        .expect("warning requires explicit confirmation");
    assert!(plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SCENE_GROUP_REORDER"));
    app.apply_scene_plan(&plan).unwrap();
    assert_eq!(app.history.len(), history + 1);
    assert_eq!(
        app.snapshot.as_ref().unwrap().map_index.maps["harbor"]
            .scene
            .as_ref()
            .unwrap()
            .root_order["places"],
        vec!["b", "group"]
    );
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn cancelled_or_stale_scene_result_never_mutates_the_project() {
    let root = test_workspace("scene-stale-cancel");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    paint(&ctx, &mut app);
    app.map_canvas.set_mode(CanvasMode::Edit);
    let before = app.project.content_baseline();
    app.map_canvas.scene.operations = vec![SceneOp::EnableScene];
    app.submit_scene_operations(&ctx);
    app.map_canvas.scene.job = None;
    app.poll_scene_operations();
    assert_eq!(app.project.content_baseline(), before);
    app.map_canvas.scene.operations = vec![SceneOp::EnableScene];
    app.submit_scene_operations(&ctx);
    app.map_revision = app.map_revision.next_presentation();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.map_canvas.scene.job.is_some() {
        app.poll_scene_operations();
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(app.project.content_baseline(), before);
    assert!(!app.map_canvas.scene.retry_operations.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn dashed_layer_projection_keeps_required_features_without_amplifying_unknown_root_data() {
    use worldline_core::vector_scene::SCENE_DASH_FEATURE;
    let mut scene = MapScene::new(400.0, 400.0);
    scene.extra.insert(
        "required_features".into(),
        serde_json::json!(vec![SCENE_DASH_FEATURE; 100]),
    );
    scene.extra.insert(
        "future-root".into(),
        serde_json::json!("x".repeat(128 * 1024)),
    );
    for (id, layer, y) in [("a", "places", 20.0), ("b", "routes", 40.0)] {
        let mut node = SceneNode::new(
            id,
            layer,
            SceneGeometry::Polyline {
                points: vec![[10.0, y], [100.0, y]],
            },
        );
        node.style.stroke = Some("blue".into());
        node.style.stroke_dasharray = Some(vec![15.0, 12.0]);
        scene.root_order.insert(layer.into(), vec![id.into()]);
        scene.nodes.insert(id.into(), node);
    }
    let canvas = canvas_scene(scene.clone());
    assert_eq!(canvas.scene.source.as_ref().unwrap(), &scene);
    assert_eq!(canvas.scene.layers.len(), 2);
    for layer in canvas.scene.layers.values() {
        assert_eq!(
            layer.scene.extra["required_features"],
            serde_json::json!([SCENE_DASH_FEATURE])
        );
        assert!(!layer.scene.extra.contains_key("future-root"));
        let svg =
            worldline_core::vector_scene::scene_to_safe_svg(&layer.scene, 400.0, 400.0).unwrap();
        assert!(svg.contains("stroke-dasharray=\"15 12\""));
        let bytes = crate::scene_raster::render_scene(
            &layer.scene,
            [400.0, 400.0],
            &crate::scene_raster::RasterSpec {
                width: 100,
                height: 100,
                zoom: 0.25,
                pan: [0.0, 0.0],
                dpi: 1.0,
            },
        )
        .unwrap();
        assert!(bytes.chunks_exact(4).any(|pixel| pixel[3] != 0));
    }
}
