use super::tests_support::*;
use super::*;
#[test]
fn refreshing_a_map_preserves_selection_and_draft_without_resetting_camera_or_layers() {
    let mut canvas = MapCanvas::new(MapRenderSnapshot {
        map_id: "map".into(),
        title: "测试地图".into(),
        extent: vec2(400.0, 400.0),
        canvas: core_canvas(400.0 as u32, 400.0 as u32),
        measurement: None,
        raster_layers: Vec::new(),
        layers: vec![MapLayer {
            id: "places".into(),
            title: "地点".into(),
            visible: true,
            locked: false,
            placements: vec![MapPlacement {
                id: "point".into(),
                target_ref: None,
                navigation: None,
                annotation: String::new(),
                role: String::new(),
                label_override: None,
                geometry: MapGeometry::Point(NormalizedPoint::new(0.25, 0.25)),
                style: MapStyle::default(),
            }],
        }],
    });
    canvas.camera.pan_by(vec2(12.0, -8.0));
    canvas.set_layer_visible("places", false);
    canvas.selected = Some(("point".into(), GeometryHit::Vertex(0)));
    canvas.drag = Some(DragState {
        placement: "point".into(),
        vertex: 0,
        initial_geometry: MapGeometry::Point(NormalizedPoint::new(0.25, 0.25)),
        start_screen: pos2(0.0, 0.0),
        grab_offset: [0.0, 0.0],
        baseline: None,
        active: true,
    });
    canvas.draft = Some(MapGeometry::Point(NormalizedPoint::new(0.3, 0.3)));
    let camera = *canvas.camera();
    canvas.set_snapshot(
        1,
        MapRenderSnapshot {
            map_id: "map".into(),
            title: "更新地图".into(),
            extent: vec2(400.0, 400.0),
            canvas: core_canvas(400.0 as u32, 400.0 as u32),
            measurement: None,
            raster_layers: Vec::new(),
            layers: vec![MapLayer {
                id: "places".into(),
                title: "地点".into(),
                visible: true,
                locked: false,
                placements: vec![MapPlacement {
                    id: "point".into(),
                    target_ref: None,
                    navigation: None,
                    annotation: String::new(),
                    role: String::new(),
                    label_override: None,
                    geometry: MapGeometry::Point(NormalizedPoint::new(0.75, 0.75)),
                    style: MapStyle::default(),
                }],
            }],
        },
    );

    assert_eq!(
        canvas.selected,
        Some(("point".into(), GeometryHit::Vertex(0)))
    );
    assert!(canvas.drag.is_some());
    assert_eq!(
        canvas.draft,
        Some(MapGeometry::Point(NormalizedPoint::new(0.3, 0.3)))
    );
    assert!(canvas.edit_intents().is_empty());
    assert_eq!(*canvas.camera(), camera);
    assert!(!canvas.layer_states()[0].1);
    assert_eq!(canvas.selected_placement(), None);
}

#[test]
fn selection_uses_stable_placement_id_when_layer_order_changes() {
    let placement = |id: &str, x: f32| MapPlacement {
        id: id.into(),
        target_ref: None,
        navigation: None,
        annotation: String::new(),
        role: String::new(),
        label_override: None,
        geometry: MapGeometry::Point(NormalizedPoint::new(x, 0.5)),
        style: MapStyle::default(),
    };
    let mut canvas = MapCanvas::new(MapRenderSnapshot {
        map_id: "map".into(),
        title: "测试地图".into(),
        extent: vec2(400.0, 400.0),
        canvas: core_canvas(400.0 as u32, 400.0 as u32),
        measurement: None,
        raster_layers: Vec::new(),
        layers: vec![MapLayer {
            id: "places".into(),
            title: "地点".into(),
            visible: true,
            locked: false,
            placements: vec![placement("first", 0.2), placement("second", 0.4)],
        }],
    });
    canvas.selected = Some(("second".into(), GeometryHit::Vertex(0)));
    canvas.set_snapshot(
        0,
        MapRenderSnapshot {
            map_id: "map".into(),
            title: "测试地图".into(),
            extent: vec2(400.0, 400.0),
            canvas: core_canvas(400.0 as u32, 400.0 as u32),
            measurement: None,
            raster_layers: Vec::new(),
            layers: vec![MapLayer {
                id: "places".into(),
                title: "地点".into(),
                visible: true,
                locked: false,
                placements: vec![
                    placement("inserted", 0.1),
                    placement("first", 0.2),
                    placement("second", 0.4),
                ],
            }],
        },
    );

    assert_eq!(
        canvas.selected_placement().map(|placement| placement.id),
        Some("second".into())
    );
}

#[test]
fn polygon_tool_draws_a_concave_shape_from_canvas_clicks() {
    let ctx = egui::Context::default();
    let mut canvas = MapCanvas::new(MapRenderSnapshot::empty(vec2(400.0, 400.0)));
    canvas.set_mode(CanvasMode::Edit);
    canvas.set_tool(CanvasTool::Polygon);
    let screen_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );
    let points = [
        pos2(0.1, 0.1),
        pos2(0.9, 0.1),
        pos2(0.9, 0.4),
        pos2(0.5, 0.4),
        pos2(0.5, 0.9),
        pos2(0.1, 0.9),
    ];
    for (index, point) in points.iter().copied().enumerate() {
        let screen_point = canvas
            .camera()
            .normalized_to_screen(point, canvas.viewport());
        click_canvas(
            &ctx,
            &mut canvas,
            screen_rect,
            screen_point,
            index as f64 + 1.0,
        );
    }
    assert!(matches!(canvas.draft, Some(MapGeometry::Polygon(ref points)) if points.len() == 6));

    let last = canvas
        .camera()
        .normalized_to_screen(points[5], canvas.viewport());
    click_canvas(&ctx, &mut canvas, screen_rect, last, 6.02);
    assert!(canvas.draft.is_none());
    assert!(matches!(
        canvas.edit_intents().first(),
        Some(EditIntent::Create(MapGeometry::Polygon(points))) if points.len() == 6
    ));
}

#[test]
fn escape_cancels_a_local_geometry_draft() {
    let ctx = egui::Context::default();
    let mut canvas = MapCanvas::new(MapRenderSnapshot::empty(vec2(400.0, 400.0)));
    canvas.set_mode(CanvasMode::Edit);
    canvas.set_tool(CanvasTool::Polyline);
    let screen_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );
    let point = canvas
        .camera()
        .normalized_to_screen(pos2(0.2, 0.2), canvas.viewport());
    click_canvas(&ctx, &mut canvas, screen_rect, point, 1.0);
    assert!(canvas.draft.is_some());

    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            events: vec![Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );

    assert!(canvas.draft.is_none());
    assert!(canvas.edit_intents().is_empty());
}
