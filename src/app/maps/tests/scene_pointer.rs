//! 真实 egui 指针/键盘事件回归，不用直接调用完成手势代替输入。
use super::*;
use worldline_core::vector_scene::{
    Affine, MapScene, PathSegment, SceneGeometry, SceneNode, SceneOp,
};

fn frame(
    ctx: &egui::Context,
    canvas: &mut MapCanvas,
    time: &mut f64,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) {
    *time += 0.1;
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
            time: Some(*time),
            events,
            modifiers,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );
}

fn setup(scene: Option<MapScene>) -> (egui::Context, MapCanvas, f64) {
    let mut snapshot = MapRenderSnapshot::empty(Vec2::splat(400.0));
    snapshot.map_id = "pointer".into();
    snapshot.layers.push(MapLayer {
        id: "places".into(),
        title: "绘制层".into(),
        visible: true,
        locked: false,
        placements: Vec::new(),
    });
    let mut canvas = MapCanvas::new(snapshot);
    canvas.sync_scene(scene.as_ref());
    canvas.set_mode(CanvasMode::Edit);
    let ctx = egui::Context::default();
    let mut time = 1.0;
    frame(
        &ctx,
        &mut canvas,
        &mut time,
        Vec::new(),
        egui::Modifiers::NONE,
    );
    (ctx, canvas, time)
}

fn screen(canvas: &MapCanvas, point: [f64; 2]) -> Pos2 {
    canvas.scene_to_screen(point).unwrap_or_else(|| {
        canvas.camera.normalized_to_screen(
            Pos2::new(point[0] as f32 / 400.0, point[1] as f32 / 400.0),
            canvas.viewport,
        )
    })
}

fn pointer(
    ctx: &egui::Context,
    canvas: &mut MapCanvas,
    time: &mut f64,
    position: Pos2,
    pressed: Option<bool>,
    modifiers: egui::Modifiers,
) {
    let mut events = vec![egui::Event::PointerMoved(position)];
    if let Some(pressed) = pressed {
        events.push(egui::Event::PointerButton {
            pos: position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers,
        });
    }
    frame(ctx, canvas, time, events, modifiers);
}

fn drag(
    ctx: &egui::Context,
    canvas: &mut MapCanvas,
    time: &mut f64,
    start: [f64; 2],
    end: [f64; 2],
    modifiers: egui::Modifiers,
) {
    let from = screen(canvas, start);
    let to = screen(canvas, end);
    pointer(ctx, canvas, time, from, Some(true), modifiers);
    pointer(ctx, canvas, time, to, None, modifiers);
    pointer(ctx, canvas, time, to, Some(false), modifiers);
}

fn key(ctx: &egui::Context, canvas: &mut MapCanvas, time: &mut f64, key: egui::Key, pressed: bool) {
    frame(
        ctx,
        canvas,
        time,
        vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        egui::Modifiers::NONE,
    );
}

fn near(actual: [f64; 2], expected: [f64; 2]) {
    assert!(
        (actual[0] - expected[0]).abs() < 0.0001,
        "{actual:?} != {expected:?}"
    );
    assert!(
        (actual[1] - expected[1]).abs() < 0.0001,
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn pointer_drag_draws_native_rectangle_and_ellipse_once() {
    for tool in [CanvasTool::Rectangle, CanvasTool::Ellipse] {
        let (ctx, mut canvas, mut time) = setup(Some(MapScene::new(400.0, 400.0)));
        canvas.set_tool(tool);
        drag(
            &ctx,
            &mut canvas,
            &mut time,
            [25.0, 30.0],
            [95.0, 90.0],
            egui::Modifiers::NONE,
        );
        assert_eq!(canvas.scene.operations.len(), 1);
        assert!(canvas.scene.gesture.is_none());
        let SceneOp::Insert { node, .. } = &canvas.scene.operations[0] else {
            panic!("insert expected");
        };
        match node.geometry {
            SceneGeometry::Rect {
                x,
                y,
                width,
                height,
                ..
            } => {
                near([x, y], [25.0, 30.0]);
                near([width, height], [70.0, 60.0]);
            }
            SceneGeometry::Ellipse { cx, cy, rx, ry } => {
                near([cx, cy], [60.0, 60.0]);
                near([rx, ry], [35.0, 30.0]);
            }
            _ => panic!("drag was flattened instead of preserving the native primitive"),
        }
    }
}

#[test]
fn pointer_bezier_tangents_and_enter_keep_open_or_closed_creation_style() {
    for closed in [false, true] {
        let (ctx, mut canvas, mut time) = setup(Some(MapScene::new(400.0, 400.0)));
        canvas.set_tool(CanvasTool::Bezier);
        canvas.scene.close_path = closed;
        canvas.scene.style.fill = Some("#88c0d0".into());
        drag(
            &ctx,
            &mut canvas,
            &mut time,
            [30.0, 40.0],
            [60.0, 20.0],
            egui::Modifiers::NONE,
        );
        drag(
            &ctx,
            &mut canvas,
            &mut time,
            [140.0, 80.0],
            [175.0, 110.0],
            egui::Modifiers::NONE,
        );
        key(&ctx, &mut canvas, &mut time, egui::Key::Enter, true);
        assert_eq!(canvas.scene.operations.len(), 1);
        let SceneOp::Insert { node, .. } = &canvas.scene.operations[0] else {
            panic!("insert expected");
        };
        let SceneGeometry::Path { segments } = &node.geometry else {
            panic!("path expected");
        };
        let PathSegment::Cubic {
            control1,
            control2,
            to,
        } = segments[1]
        else {
            panic!("real Cubic expected");
        };
        near(control1, [60.0, 20.0]);
        near(control2, [105.0, 50.0]);
        near(to, [140.0, 80.0]);
        assert_eq!(matches!(segments.last(), Some(PathSegment::Close)), closed);
        assert_eq!(
            node.style.fill.as_deref(),
            Some(if closed { "#88c0d0" } else { "none" })
        );
        assert!(canvas.scene.path.is_empty());
    }
}

#[test]
fn alt_pointer_starts_a_second_closed_subpath_without_losing_the_first() {
    let (ctx, mut canvas, mut time) = setup(Some(MapScene::new(400.0, 400.0)));
    canvas.set_tool(CanvasTool::Bezier);
    canvas.scene.close_path = true;
    canvas.scene.style.fill_rule = Some("evenodd".into());
    for (start, end, alt) in [
        ([20.0, 20.0], [45.0, 10.0], false),
        ([170.0, 150.0], [195.0, 160.0], false),
        ([60.0, 60.0], [70.0, 50.0], true),
        ([100.0, 100.0], [110.0, 115.0], false),
    ] {
        drag(
            &ctx,
            &mut canvas,
            &mut time,
            start,
            end,
            egui::Modifiers {
                alt,
                ..Default::default()
            },
        );
    }
    key(&ctx, &mut canvas, &mut time, egui::Key::Enter, true);
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
    assert_eq!(
        segments
            .iter()
            .filter(|segment| matches!(segment, PathSegment::Cubic { .. }))
            .count(),
        2
    );
    assert_eq!(node.style.fill_rule.as_deref(), Some("evenodd"));
}

#[test]
fn dragging_a_bezier_handle_uses_parent_inverse_and_preserves_selection_camera() {
    let mut scene = MapScene::new(400.0, 400.0);
    let mut group = SceneNode::new(
        "group",
        "places",
        SceneGeometry::Group {
            children: vec!["curve".into()],
        },
    );
    group.transform = Affine([1.0, 0.2, 0.3, 1.0, 40.0, 20.0]);
    let mut curve = SceneNode::new(
        "curve",
        "places",
        SceneGeometry::Path {
            segments: vec![
                PathSegment::Move { to: [10.0, 10.0] },
                PathSegment::Cubic {
                    control1: [20.0, 10.0],
                    control2: [70.0, 70.0],
                    to: [80.0, 80.0],
                },
            ],
        },
    );
    curve.parent_id = Some("group".into());
    curve.transform = Affine([2.0, 0.0, 0.0, 1.0, 10.0, 20.0]);
    let world = group.transform.then(curve.transform);
    let original = curve.transform;
    scene
        .root_order
        .insert("places".into(), vec!["group".into()]);
    scene.nodes.insert("group".into(), group);
    scene.nodes.insert("curve".into(), curve);
    let (ctx, mut canvas, mut time) = setup(Some(scene));
    canvas.select_scene("curve", false);
    canvas.set_tool(CanvasTool::Nodes);
    let camera = canvas.camera_state();
    drag(
        &ctx,
        &mut canvas,
        &mut time,
        world.point([20.0, 10.0]),
        world.point([50.0, 30.0]),
        egui::Modifiers::NONE,
    );
    let SceneOp::Update { node } = &canvas.scene.operations[0] else {
        panic!("update expected");
    };
    let SceneGeometry::Path { segments } = &node.geometry else {
        panic!("native path expected");
    };
    let PathSegment::Cubic {
        control1,
        control2,
        to,
    } = segments[1]
    else {
        panic!("Cubic expected");
    };
    near(control1, [50.0, 30.0]);
    near(control2, [70.0, 70.0]);
    near(to, [80.0, 80.0]);
    assert_eq!(node.transform, original);
    assert_eq!(node.parent_id.as_deref(), Some("group"));
    assert!(canvas.scene.selection.contains("curve"));
    assert_eq!(canvas.camera_state(), camera);
}

#[test]
fn clicking_an_evenodd_hole_does_not_turn_a_zero_size_marquee_into_selection() {
    let square = |a, b| {
        vec![
            PathSegment::Move { to: [a, a] },
            PathSegment::Line { to: [b, a] },
            PathSegment::Line { to: [b, b] },
            PathSegment::Line { to: [a, b] },
            PathSegment::Close,
        ]
    };
    let mut segments = square(20.0, 180.0);
    segments.extend(square(80.0, 120.0));
    let mut ring = SceneNode::new("ring", "places", SceneGeometry::Path { segments });
    ring.style.fill_rule = Some("evenodd".into());
    let mut scene = MapScene::new(400.0, 400.0);
    scene
        .root_order
        .insert("places".into(), vec!["ring".into()]);
    scene.nodes.insert("ring".into(), ring);
    let (ctx, mut canvas, mut time) = setup(Some(scene));
    canvas.select_scene("ring", false);
    drag(
        &ctx,
        &mut canvas,
        &mut time,
        [100.0, 100.0],
        [100.0, 100.0],
        egui::Modifiers::NONE,
    );
    assert!(canvas.scene.selection.is_empty());
    assert!(canvas.scene.operations.is_empty());
}

#[test]
fn space_pressed_during_a_shape_drag_does_not_leave_a_stuck_gesture() {
    let (ctx, mut canvas, mut time) = setup(Some(MapScene::new(400.0, 400.0)));
    canvas.set_tool(CanvasTool::Rectangle);
    let start = screen(&canvas, [20.0, 20.0]);
    let end = screen(&canvas, [80.0, 70.0]);
    pointer(
        &ctx,
        &mut canvas,
        &mut time,
        start,
        Some(true),
        egui::Modifiers::NONE,
    );
    key(&ctx, &mut canvas, &mut time, egui::Key::Space, true);
    pointer(
        &ctx,
        &mut canvas,
        &mut time,
        end,
        None,
        egui::Modifiers::NONE,
    );
    pointer(
        &ctx,
        &mut canvas,
        &mut time,
        end,
        Some(false),
        egui::Modifiers::NONE,
    );
    assert!(canvas.scene.gesture.is_none());
    assert_eq!(canvas.scene.operations.len(), 1);
}

#[test]
fn legacy_line_still_finishes_with_enter_without_enabling_scene() {
    let (ctx, mut canvas, mut time) = setup(None);
    canvas.set_tool(CanvasTool::Polyline);
    drag(
        &ctx,
        &mut canvas,
        &mut time,
        [20.0, 20.0],
        [20.0, 20.0],
        egui::Modifiers::NONE,
    );
    drag(
        &ctx,
        &mut canvas,
        &mut time,
        [100.0, 80.0],
        [100.0, 80.0],
        egui::Modifiers::NONE,
    );
    key(&ctx, &mut canvas, &mut time, egui::Key::Enter, true);
    assert!(canvas.scene.source.is_none());
    assert!(canvas.draft.is_none());
    assert!(
        matches!(canvas.edit_intents.first(), Some(EditIntent::Create(MapGeometry::Polyline(points))) if points.len() == 2)
    );
}
