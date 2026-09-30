use super::tests_support::*;
use super::*;
#[test]
fn clicking_an_existing_marker_selects_without_creating_history() {
    let ctx = egui::Context::default();
    let mut canvas = point_canvas();
    canvas.set_mode(CanvasMode::Edit);
    canvas.set_tool(CanvasTool::Select);
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
        .normalized_to_screen(pos2(0.25, 0.25), canvas.viewport());
    click_canvas(&ctx, &mut canvas, screen_rect, point, 1.0);
    assert_eq!(
        canvas.selected,
        Some(("point".into(), GeometryHit::Vertex(0)))
    );
    assert!(canvas.edit_intents().is_empty());
    assert!(canvas.draft.is_none());
}

#[test]
fn pointer_release_outside_canvas_finishes_drag_without_stale_capture() {
    let ctx = egui::Context::default();
    let mut canvas = point_canvas();
    canvas.set_mode(CanvasMode::Edit);
    canvas.set_tool(CanvasTool::Select);
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
    let start = canvas
        .camera()
        .normalized_to_screen(pos2(0.25, 0.25), canvas.viewport());
    let moved = start + vec2(24.0, 16.0);
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            events: vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            events: vec![Event::PointerMoved(moved)],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );
    assert!(canvas.drag.is_some());
    let outside = pos2(-20.0, moved.y);
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            events: vec![
                Event::PointerMoved(outside),
                Event::PointerButton {
                    pos: outside,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );

    assert!(canvas.drag.is_none());
    assert!(canvas.draft.is_none());
    assert!(matches!(
        canvas.edit_intents().first(),
        Some(EditIntent::Move { placement, .. }) if placement == "point"
    ));
}

#[test]
fn delete_key_queues_placement_delete_only_in_edit_mode() {
    let ctx = egui::Context::default();
    let mut canvas = point_canvas();
    canvas.set_mode(CanvasMode::Edit);
    canvas.set_tool(CanvasTool::Select);
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
        .normalized_to_screen(pos2(0.25, 0.25), canvas.viewport());
    click_canvas(&ctx, &mut canvas, screen_rect, point, 1.0);
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            events: vec![Event::Key {
                key: egui::Key::Delete,
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
    assert!(matches!(
        canvas.edit_intents().first(),
        Some(EditIntent::Delete { placement }) if placement == "point"
    ));
}
#[test]
fn edit_canvas_hit_test_uses_screen_pixel_tolerance_for_non_square_extent() {
    let ctx = egui::Context::default();
    let mut canvas = MapCanvas::new(MapRenderSnapshot {
        map_id: String::new(),
        title: String::new(),
        extent: vec2(1000.0, 100.0),
        canvas: core_canvas(1000.0 as u32, 100.0 as u32),
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
                geometry: MapGeometry::Point(NormalizedPoint::new(0.5, 0.5)),
                style: MapStyle::default(),
            }],
        }],
    });
    canvas.set_mode(CanvasMode::Edit);
    canvas.set_tool(CanvasTool::Select);

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
    let exact = canvas
        .camera()
        .normalized_to_screen(pos2(0.5, 0.5), canvas.viewport());
    let near = exact + vec2(0.0, 8.0);
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            events: vec![
                Event::PointerMoved(near),
                Event::PointerButton {
                    pos: near,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            events: vec![
                Event::PointerMoved(near),
                Event::PointerButton {
                    pos: near,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );
    assert_eq!(
        canvas.selected,
        Some(("point".into(), GeometryHit::Vertex(0)))
    );
}

#[test]
fn edit_drag_keeps_preview_until_release_and_emits_one_intent() {
    let ctx = egui::Context::default();
    let mut canvas = MapCanvas::new(MapRenderSnapshot {
        map_id: String::new(),
        title: String::new(),
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
    canvas.set_mode(CanvasMode::Edit);
    canvas.set_tool(CanvasTool::Select);
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
    let start = canvas
        .camera()
        .normalized_to_screen(pos2(0.25, 0.25), canvas.viewport());
    let first_move = start + vec2(40.0, 20.0);
    let second_move = start + vec2(80.0, 40.0);

    for (events, expected_intents) in [
        (
            vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            0,
        ),
        (vec![Event::PointerMoved(first_move)], 0),
        (vec![Event::PointerMoved(second_move)], 0),
    ] {
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
        assert_eq!(canvas.edit_intents().len(), expected_intents);
    }
    assert!(canvas.draft.is_some());

    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            events: vec![
                Event::PointerMoved(second_move),
                Event::PointerButton {
                    pos: second_move,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );
    assert_eq!(canvas.edit_intents().len(), 1);
    assert!(canvas.draft.is_none());
    assert!(matches!(
        canvas.edit_intents().first(),
        Some(EditIntent::Move {
            placement,
            geometry: MapGeometry::Point(point)
        }) if placement == "point" && point.x > 0.25 && point.y > 0.25
    ));
}

#[test]
fn locked_layer_allows_selection_but_rejects_drag_intent() {
    let ctx = egui::Context::default();
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
            locked: true,
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
    canvas.set_mode(CanvasMode::Edit);
    canvas.set_tool(CanvasTool::Select);
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
        .normalized_to_screen(pos2(0.25, 0.25), canvas.viewport());
    click_canvas(&ctx, &mut canvas, screen_rect, point, 1.0);

    assert_eq!(
        canvas.selected,
        Some(("point".into(), GeometryHit::Vertex(0)))
    );
    assert!(canvas.edit_intents().is_empty());
    assert_eq!(canvas.validation_error(), Some("图层已锁定，只能浏览标记"));
}

#[test]
fn point_tool_rejects_clicks_in_the_margin_outside_the_map() {
    let ctx = egui::Context::default();
    let mut canvas = MapCanvas::new(MapRenderSnapshot::empty(vec2(400.0, 200.0)));
    canvas.set_mode(CanvasMode::Edit);
    canvas.set_tool(CanvasTool::Point);
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
    let margin = canvas
        .camera()
        .normalized_to_screen(pos2(0.5, -0.1), canvas.viewport());
    assert!(canvas.viewport().contains(margin));
    click_canvas(&ctx, &mut canvas, screen_rect, margin, 1.0);
    assert!(canvas.edit_intents().is_empty(), "地图留白不能生成越界点");
    assert_eq!(canvas.validation_error(), Some("点必须位于地图范围内"));
}

#[test]
fn dragging_a_control_point_outside_the_map_is_rejected() {
    let ctx = egui::Context::default();
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
    canvas.set_mode(CanvasMode::Edit);
    canvas.set_tool(CanvasTool::Select);
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
    let start = canvas
        .camera()
        .normalized_to_screen(pos2(0.25, 0.25), canvas.viewport());
    let outside = canvas
        .camera()
        .normalized_to_screen(pos2(-0.1, 0.25), canvas.viewport());
    assert!(canvas.viewport().contains(outside));

    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            events: vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            events: vec![Event::PointerMoved(outside)],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(screen_rect),
            events: vec![
                Event::PointerMoved(outside),
                Event::PointerButton {
                    pos: outside,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );

    assert!(canvas.edit_intents().is_empty());
    assert!(canvas.draft.is_some());
    assert_eq!(canvas.validation_error(), Some("点必须位于地图范围内"));
}
