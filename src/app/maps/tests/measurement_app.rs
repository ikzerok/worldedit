//! 编辑器完整命令链：确认、冲突、取消、只读、撤销和保存重开。
use super::tests_support::*;
use super::*;
use worldline_core::presentation::MapMeasurement;

type App = super::super::WorldeditApp;

fn paint(ctx: &egui::Context, app: &mut App) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1100.0, 700.0))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    )
}

fn app(name: &str) -> (App, egui::Context, PathBuf) {
    let root = navigation_workspace(name);
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = App::new(&creation, Some(root.join("world.wl")));
    app.map_selection = Some("harbor".into());
    paint(&ctx, &mut app);
    app.map_canvas.set_mode(CanvasMode::Edit);
    app.map_canvas
        .set_command_baseline(app.map_command_baseline("harbor"));
    (app, ctx, root)
}

fn map_bytes(app: &App) -> Vec<u8> {
    let path =
        worldline_core::presentation_commands::map_document_path(&app.project, "harbor").unwrap();
    app.project
        .authoring_document(&path)
        .unwrap()
        .bytes()
        .to_vec()
}

fn start(app: &mut App) {
    assert!(app.map_canvas.begin_calibration());
    app.map_canvas.measurement_click([0.1, 0.2]);
    app.map_canvas.measurement_click([0.8, 0.2]);
    let draft = app.map_canvas.measurement.calibration.as_mut().unwrap();
    draft.distance = "32".into();
    draft.unit = "千米".into();
}

fn confirm(app: &mut App) {
    assert!(app.map_canvas.confirm_calibration());
    let (intents, baselines) = app.map_canvas.take_edit_batch();
    app.apply_map_intents(intents, baselines);
}

#[test]
fn calibration_confirm_undo_redo_save_reopen_only_changes_presentation() {
    let (mut app, ctx, root) = app("measurement-lifecycle");
    let source = app.project.sources();
    let history = app.history.len();
    let bytes = map_bytes(&app);
    start(&mut app);
    assert_eq!(map_bytes(&app), bytes);
    assert_eq!(app.history.len(), history);
    confirm(&mut app);
    assert!(app.map_failed_command.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), history + 1);
    assert_eq!(app.project.sources(), source);
    let value = app.project.map_index().maps["harbor"]
        .measurement
        .clone()
        .unwrap();
    assert_eq!(value.distance, 32.0);
    assert_eq!(value.unit, "千米");
    let json: serde_json::Value = serde_json::from_slice(&map_bytes(&app)).unwrap();
    assert!(json["required_features"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "presentation.measurement.v1"));
    app.undo(false);
    assert!(app.project.map_index().maps["harbor"].measurement.is_none());
    assert_eq!(map_bytes(&app), bytes);
    app.undo(true);
    assert_eq!(
        app.project.map_index().maps["harbor"].measurement,
        Some(value.clone())
    );
    paint(&ctx, &mut app);
    assert_eq!(app.map_canvas.snapshot.measurement, Some(value.clone()));
    assert!(app.save(), "{:?}", app.io_error);
    let reopened = worldline_core::project::Project::open(&root.join("world.wl")).unwrap();
    assert_eq!(reopened.map_index().maps["harbor"].measurement, Some(value));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn canceled_calibration_and_browse_ruler_preserve_bytes_history_revision_and_dirty_state() {
    let (mut app, ctx, root) = app("measurement-zero-write");
    start(&mut app);
    confirm(&mut app);
    paint(&ctx, &mut app);
    assert!(app.save());
    let bytes = map_bytes(&app);
    let history = app.history.len();
    let revision = app.map_revision;
    let source = app.project.sources();
    let dirty = app.project.is_dirty();
    app.map_canvas
        .set_command_baseline(app.map_command_baseline("harbor"));
    start(&mut app);
    assert!(app.map_navigation_blocked());
    app.map_canvas.cancel_measurement();
    app.map_canvas.set_mode(CanvasMode::Browse);
    app.map_canvas.set_ruler(true);
    app.map_canvas.measurement_click([0.1, 0.2]);
    app.map_canvas.measurement_click([0.8, 0.2]);
    assert_eq!(app.map_canvas.ruler_distance().unwrap(), 32.0);
    paint(&ctx, &mut app);
    assert!(!app.map_navigation_blocked());
    assert!(app.map_canvas.edit_intents.is_empty());
    app.map_canvas.set_ruler(false);
    assert_eq!(map_bytes(&app), bytes);
    assert_eq!(app.project.sources(), source);
    assert_eq!(app.history.len(), history);
    assert_eq!(app.map_revision, revision);
    assert_eq!(app.project.is_dirty(), dirty);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stale_calibration_is_atomic_and_retry_uses_retained_edited_input() {
    let (mut app, _, root) = app("measurement-conflict-retry");
    start(&mut app);
    let bytes = map_bytes(&app);
    let history = app.history.len();
    app.map_revision = app.map_revision.next_presentation();
    confirm(&mut app);
    assert!(app.map_failed_command.is_some());
    assert_eq!(map_bytes(&app), bytes);
    assert_eq!(app.history.len(), history);
    assert_eq!(
        app.map_canvas
            .measurement
            .calibration
            .as_ref()
            .unwrap()
            .distance,
        "32"
    );
    app.map_canvas.set_mode(CanvasMode::Browse);
    app.retry_failed_map_command();
    assert_eq!(map_bytes(&app), bytes);
    app.map_canvas.set_mode(CanvasMode::Edit);
    app.map_canvas
        .measurement
        .calibration
        .as_mut()
        .unwrap()
        .distance = "45".into();
    app.retry_failed_map_command();
    assert!(app.map_failed_command.is_none(), "{:?}", app.io_error);
    assert_eq!(
        app.project.map_index().maps["harbor"]
            .measurement
            .as_ref()
            .unwrap()
            .distance,
        45.0
    );
    assert_eq!(app.history.len(), history + 1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn canceling_a_failed_calibration_unblocks_map_navigation_without_writes() {
    let (mut app, ctx, root) = app("measurement-conflict-cancel");
    start(&mut app);
    let bytes = map_bytes(&app);
    app.map_revision = app.map_revision.next_presentation();
    confirm(&mut app);
    assert!(app.map_failed_command.is_some());
    app.map_canvas.cancel_measurement();
    paint(&ctx, &mut app);
    assert!(app.map_failed_command.is_none());
    assert!(!app.map_navigation_blocked());
    assert_eq!(map_bytes(&app), bytes);
    assert!(app.history.is_empty());
    app.map_selection = Some("city".into());
    paint(&ctx, &mut app);
    assert_eq!(app.map_canvas.map_id(), "city");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn active_calibration_rejects_unrelated_commands_without_losing_its_form() {
    let (mut app, _, root) = app("measurement-mutual-exclusion");
    start(&mut app);
    let bytes = map_bytes(&app);
    assert!(!app.apply_map_command(
        "harbor",
        worldline_core::presentation_commands::Command::SetLayer {
            map_id: "harbor".into(),
            layer_id: "places".into(),
            title: None,
            visible_default: None,
            locked: Some(true),
            layer_order: None,
        },
        "不应提交"
    ));
    assert_eq!(map_bytes(&app), bytes);
    assert_eq!(
        app.map_canvas
            .measurement
            .calibration
            .as_ref()
            .unwrap()
            .unit,
        "千米"
    );
    assert!(app.history.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn render_snapshot_keeps_exact_core_dimensions_above_f32_integer_precision() {
    let (app, _, root) = app("measurement-precision");
    let mut document = app.project.map_index().maps["harbor"].clone();
    document.canvas.width = 16_777_217;
    document.canvas.height = 16_777_219;
    document.measurement = Some(MapMeasurement {
        points: [[0.0, 0.0], [1.0, 0.0]],
        distance: 1.0,
        unit: "步".into(),
        extra: Default::default(),
    });
    let snapshot = render_snapshot(&document);
    assert_eq!(snapshot.canvas, document.canvas);
    assert_ne!(snapshot.extent.x as u32, document.canvas.width);
    let mut canvas = MapCanvas::new(snapshot);
    canvas.set_ruler(true);
    canvas.measurement_click([0.0, 0.0]);
    canvas.measurement_click([0.0, 1.0]);
    let expected = worldline_core::presentation::measurement_distance(
        &document.canvas,
        document.measurement.as_ref().unwrap(),
        [[0.0, 0.0], [0.0, 1.0]],
    )
    .unwrap();
    assert_eq!(canvas.ruler_distance().unwrap(), expected);
    let _ = std::fs::remove_dir_all(root);
}

fn visible_text(output: &egui::FullOutput, needle: &str) -> bool {
    fn find(shape: &egui::Shape, needle: &str) -> Option<Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text().contains(needle) => Some(text.pos),
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, needle)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .any(|shape| find(&shape.shape, needle).is_some_and(|pos| shape.clip_rect.contains(pos)))
}

#[test]
fn compact_calibration_keeps_labels_confirm_and_cancel_in_view_in_both_modes() {
    let (mut app, ctx, root) = app("measurement-compact-form");
    start(&mut app);
    for mode in [CanvasMode::Edit, CanvasMode::Browse] {
        app.map_canvas.set_mode(mode);
        let output = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(640.0, 700.0))),
                ..Default::default()
            },
            |ctx| app.map_tab(ctx),
        );
        for label in ["已知距离", "单位", "确认保存校准", "取消校准"] {
            assert!(visible_text(&output, label), "{mode:?}: {label}");
        }
        assert!(app.map_canvas.viewport.height() > 150.0);
        assert_eq!(
            app.map_canvas
                .measurement
                .calibration
                .as_ref()
                .unwrap()
                .distance,
            "32"
        );
    }
    assert!(app.history.is_empty());
    let _ = std::fs::remove_dir_all(root);
}
