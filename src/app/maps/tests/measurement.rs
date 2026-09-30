//! 尺子与校准的局部状态、相机独立性及零写入边界。
use super::tests_support::*;
use super::*;
use worldline_core::presentation::MapMeasurement;

fn calibration() -> MapMeasurement {
    MapMeasurement {
        points: [[0.0, 0.0], [0.5, 0.0]],
        distance: 10.0,
        unit: "千米".into(),
        extra: Default::default(),
    }
}

fn calibrated_canvas() -> MapCanvas {
    let mut canvas = point_canvas();
    canvas.snapshot.canvas = core_canvas(1000, 500);
    canvas.snapshot.measurement = Some(calibration());
    canvas.core_snapshot = canvas.snapshot.clone();
    canvas
}

fn fill_calibration(canvas: &mut MapCanvas) {
    assert!(canvas.begin_calibration());
    canvas.measurement_click([0.1, 0.1]);
    canvas.measurement_click([0.8, 0.1]);
    let draft = canvas.measurement.calibration.as_mut().unwrap();
    draft.distance = "12.5".into();
    draft.unit = "里".into();
}

fn frame(ctx: &egui::Context, canvas: &mut MapCanvas, size: Vec2, events: Vec<Event>) {
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );
}

#[test]
fn ruler_clicks_never_queue_intents_and_next_click_starts_new_segment() {
    let mut canvas = calibrated_canvas();
    canvas.set_ruler(true);
    canvas.measurement_click([0.0, 0.0]);
    canvas.measurement_click([0.0, 1.0]);
    assert_eq!(canvas.ruler_distance().unwrap(), 10.0);
    assert!(canvas.edit_intents.is_empty());
    assert!(!canvas.has_uncommitted_work());
    canvas.measurement_click([0.2, 0.3]);
    assert_eq!(canvas.measurement.ruler_points, [[0.2, 0.3]]);
    assert!(canvas.ruler_distance().is_err());
    canvas.set_ruler(false);
    assert!(canvas.measurement.ruler_points.is_empty());
    assert!(canvas.edit_intents.is_empty());
}

#[test]
fn ruler_real_pointer_events_do_not_select_or_move_existing_markers() {
    let ctx = egui::Context::default();
    let mut canvas = calibrated_canvas();
    canvas.set_ruler(true);
    let size = vec2(900.0, 700.0);
    frame(&ctx, &mut canvas, size, vec![]);
    for (index, point) in [pos2(0.25, 0.25), pos2(0.75, 0.25)].iter().enumerate() {
        let pointer = canvas.camera.normalized_to_screen(*point, canvas.viewport);
        click_canvas(
            &ctx,
            &mut canvas,
            Rect::from_min_size(Pos2::ZERO, size),
            pointer,
            index as f64 + 1.0,
        );
    }
    assert_eq!(canvas.measurement.ruler_points.len(), 2);
    assert!(canvas.selected.is_none());
    assert!(canvas.edit_intents.is_empty());
    assert!(canvas.draft.is_none());
    assert!(canvas.ruler_distance().unwrap() > 0.0);
}

#[test]
fn ruler_distance_is_unchanged_by_pan_zoom_reset_fit_and_resize() {
    let ctx = egui::Context::default();
    let mut canvas = calibrated_canvas();
    canvas.set_ruler(true);
    canvas.measurement_click([0.125, 0.25]);
    canvas.measurement_click([0.75, 0.75]);
    let result = canvas.ruler_distance().unwrap();
    let points = canvas.measurement.ruler_points.clone();
    frame(&ctx, &mut canvas, vec2(900.0, 700.0), vec![]);
    canvas.camera.pan_by(vec2(-400.0, 170.0));
    canvas
        .camera
        .zoom_at(pos2(300.0, 400.0), 3.0, canvas.viewport);
    assert_eq!(canvas.ruler_distance().unwrap(), result);
    frame(&ctx, &mut canvas, vec2(640.0, 500.0), vec![]);
    canvas.camera.fit(canvas.viewport);
    assert_eq!(canvas.ruler_distance().unwrap(), result);
    canvas.camera.reset();
    assert_eq!(canvas.measurement.ruler_points, points);
    assert_eq!(canvas.ruler_distance().unwrap(), result);
}

#[test]
fn calibration_requires_explicit_confirmation_and_keeps_start_baseline() {
    let mut canvas = calibrated_canvas();
    canvas.set_mode(CanvasMode::Edit);
    let baseline = MapCommandBaseline {
        revision: Default::default(),
        expected_documents: BTreeMap::new(),
    };
    canvas.set_command_baseline(Some(baseline.clone()));
    fill_calibration(&mut canvas);
    assert!(canvas.has_uncommitted_work());
    assert!(canvas.edit_intents.is_empty());
    canvas.set_command_baseline(None);
    assert!(canvas.confirm_calibration());
    assert!(!canvas.confirm_calibration());
    let (intents, baselines) = canvas.take_edit_batch();
    assert_eq!(intents.len(), 1);
    assert!(
        matches!(&intents[0], EditIntent::SetMeasurement(value) if value.distance == 12.5 && value.unit == "里")
    );
    assert_eq!(baselines, [Some(baseline)]);
}

#[test]
fn calibration_cancel_and_escape_never_leave_a_write_intent() {
    for escape in [false, true] {
        let mut canvas = calibrated_canvas();
        canvas.set_mode(CanvasMode::Edit);
        fill_calibration(&mut canvas);
        if escape {
            let ctx = egui::Context::default();
            frame(
                &ctx,
                &mut canvas,
                vec2(900.0, 700.0),
                vec![Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        } else {
            assert!(canvas.cancel_measurement());
        }
        assert!(canvas.measurement.calibration.is_none());
        assert!(!canvas.has_uncommitted_work());
        assert!(canvas.edit_intents.is_empty());
        assert_eq!(canvas.snapshot.measurement, Some(calibration()));
    }
}

#[test]
fn calibration_blocks_navigation_until_cancel_and_survives_browse_roundtrip() {
    let mut canvas = calibrated_canvas();
    canvas.set_mode(CanvasMode::Edit);
    fill_calibration(&mut canvas);
    let original_points = canvas
        .measurement
        .calibration
        .as_ref()
        .unwrap()
        .points
        .clone();
    let mut next = canvas.snapshot.clone();
    next.map_id = "next-map".into();
    assert!(!canvas.set_snapshot(10, next.clone()));
    assert!(!canvas.reset_for_navigation());
    canvas.set_mode(CanvasMode::Browse);
    assert!(!canvas.confirm_calibration());
    canvas.measurement_click([0.9, 0.9]);
    assert_eq!(
        canvas.measurement.calibration.as_ref().unwrap().points,
        original_points
    );
    assert_eq!(
        canvas.measurement.calibration.as_ref().unwrap().distance,
        "12.5"
    );
    canvas.set_mode(CanvasMode::Edit);
    assert!(canvas.calibration_value().is_ok());
    assert!(canvas.cancel_measurement());
    assert!(canvas.set_snapshot(10, next));
    assert!(canvas.measurement.calibration.is_none());
}

#[test]
fn measurement_preserves_geometry_and_does_not_start_during_an_import() {
    let mut canvas = calibrated_canvas();
    canvas.set_mode(CanvasMode::Edit);
    canvas.draft = Some(MapGeometry::Polyline(vec![NormalizedPoint::new(0.2, 0.2)]));
    let draft = canvas.draft.clone();
    assert!(!canvas.begin_calibration());
    canvas.set_mode(CanvasMode::Browse);
    canvas.set_ruler(true);
    canvas.measurement_click([0.1, 0.1]);
    canvas.measurement_click([0.9, 0.9]);
    canvas.cancel_measurement();
    canvas.set_mode(CanvasMode::Edit);
    assert_eq!(canvas.draft, draft);
    canvas.draft = None;
    canvas.svg_import.open = true;
    assert!(!canvas.begin_calibration());
    assert!(canvas.svg_import.open);
    canvas.set_mode(CanvasMode::Browse);
    assert!(!canvas.svg_import.open);
}

#[test]
fn invalid_inputs_are_rejected_by_core_without_queuing_an_intent() {
    let mut canvas = calibrated_canvas();
    canvas.set_mode(CanvasMode::Edit);
    fill_calibration(&mut canvas);
    for distance in ["0", "-1", "NaN", "inf", "1e400", "abc"] {
        canvas.measurement.calibration.as_mut().unwrap().distance = distance.into();
        assert!(!canvas.confirm_calibration(), "{distance}");
        assert!(canvas.edit_intents.is_empty());
    }
    canvas.measurement.calibration.as_mut().unwrap().distance = "1".into();
    for unit in [
        "",
        "  ",
        "bad\nunit",
        "超过二十四个字符超过二十四个字符超过二十四个字符超过二十四个字符",
    ] {
        canvas.measurement.calibration.as_mut().unwrap().unit = unit.into();
        assert!(!canvas.confirm_calibration(), "{unit}");
    }
    let draft = canvas.measurement.calibration.as_mut().unwrap();
    draft.unit = "千米".into();
    draft.points = vec![[0.5, 0.5], [0.5, 0.5]];
    assert!(!canvas.confirm_calibration());
    canvas.measurement.calibration.as_mut().unwrap().points = vec![[0.0, 0.0], [1.1, 0.5]];
    assert!(!canvas.confirm_calibration());
    canvas.measurement.calibration.as_mut().unwrap().points = vec![[0.0, 0.0], [f64::NAN, 0.5]];
    assert!(!canvas.confirm_calibration());
    assert!(canvas.edit_intents.is_empty());
}

#[test]
fn ruler_rejects_zero_length_out_of_bounds_and_overflow() {
    let mut canvas = calibrated_canvas();
    canvas.set_ruler(true);
    for points in [[[0.2, 0.2], [0.2, 0.2]], [[0.0, 0.0], [1.1, 0.5]]] {
        canvas.measurement.ruler_points = points.to_vec();
        assert!(canvas.ruler_distance().is_err());
    }
    canvas.snapshot.measurement.as_mut().unwrap().distance = f64::MAX;
    canvas.measurement.ruler_points = vec![[0.0, 0.0], [1.0, 1.0]];
    assert!(canvas.ruler_distance().is_err());
    assert!(canvas.edit_intents.is_empty());
}

#[test]
fn small_and_large_positive_distances_use_bounded_nonzero_scientific_labels() {
    for value in [f64::MIN_POSITIVE, f64::from_bits(1), 1e-12, 1e20, f64::MAX] {
        let label = measurement::distance_label(value, "步");
        assert!(label.contains('e'), "{label}");
        assert!(!label.starts_with("0 "));
        assert!(label.chars().count() < 30, "{label}");
    }
    assert_eq!(measurement::distance_label(12.5, "千米"), "12.5 千米");
}

#[test]
fn calibration_and_ruler_do_not_need_any_unlocked_content_layer() {
    let mut canvas = calibrated_canvas();
    canvas.snapshot.layers.clear();
    canvas.set_mode(CanvasMode::Edit);
    fill_calibration(&mut canvas);
    assert!(canvas.confirm_calibration());
    canvas.cancel_measurement();
    canvas.set_mode(CanvasMode::Browse);
    canvas.set_ruler(true);
    canvas.measurement_click([0.0, 0.0]);
    canvas.measurement_click([0.5, 0.0]);
    assert_eq!(canvas.ruler_distance().unwrap(), 10.0);
    assert!(canvas.edit_intents.is_empty());
}

#[test]
fn unchanged_calibration_is_local_and_does_not_queue_a_failed_command() {
    let mut canvas = calibrated_canvas();
    canvas.set_mode(CanvasMode::Edit);
    assert!(canvas.begin_calibration());
    for point in calibration().points {
        canvas.measurement_click(point);
    }
    assert_eq!(canvas.calibration_value().unwrap(), calibration());
    assert!(!canvas.confirm_calibration());
    assert_eq!(
        canvas.measurement.error.as_deref(),
        Some("校准未改变，无需保存")
    );
    assert!(canvas.edit_intents.is_empty());
    assert!(canvas.intent_baselines.is_empty());
    assert!(canvas.measurement.calibration.is_some());
    assert!(canvas.cancel_measurement());
}

#[test]
fn suspended_calibration_cannot_enter_ruler_or_lose_its_points() {
    let mut canvas = calibrated_canvas();
    canvas.set_mode(CanvasMode::Edit);
    fill_calibration(&mut canvas);
    let value = canvas.calibration_value().unwrap();
    canvas.set_mode(CanvasMode::Browse);
    canvas.set_ruler(true);
    assert!(!canvas.measurement.ruler);
    assert!(canvas.measurement.ruler_points.is_empty());
    canvas.measurement_click([0.4, 0.8]);
    assert_eq!(canvas.calibration_value().unwrap(), value);
    assert!(canvas.edit_intents.is_empty());
    canvas.set_mode(CanvasMode::Edit);
    assert_eq!(canvas.calibration_value().unwrap(), value);
}
