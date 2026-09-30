use super::tests_support::*;
use super::*;

fn app(name: &str) -> (super::super::WorldeditApp, egui::Context, PathBuf) {
    let root = test_workspace(name);
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1100.0, 700.0))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );
    app.map_canvas.set_mode(CanvasMode::Edit);
    app.map_canvas
        .set_command_baseline(app.map_command_baseline("harbor"));
    (app, ctx, root)
}
fn draft(
    app: &super::super::WorldeditApp,
    placement: Option<String>,
    text: &str,
) -> text_labels::TextDraft {
    text_labels::TextDraft {
        map_id: "harbor".into(),
        layer_id: "places".into(),
        placement_id: placement,
        position: NormalizedPoint::new(0.2, 0.3),
        text: text.into(),
        font_size: 24.0,
        color: "#243447".into(),
        baseline: app.map_command_baseline("harbor"),
    }
}
#[test]
fn text_label_create_edit_undo_save_reopen_is_independent_from_content() {
    let (mut app, _, root) = app("text-lifecycle");
    let sources = app.project.sources();
    let history = app.history.len();
    app.map_form.text_draft = Some(draft(&app, None, "北境\n山脉"));
    assert!(app.commit_map_text(false), "{:?}", app.io_error);
    assert_eq!(app.history.len(), history + 1);
    assert_eq!(app.project.sources(), sources);
    let first = app.project.map_index().maps["harbor"].placements["marker_1"]
        .geometry
        .clone();
    assert!(
        app.project.map_index().maps["harbor"].placements["marker_1"]
            .target_ref
            .is_none()
    );
    app.map_form.text_draft = Some(draft(&app, Some("marker_1".into()), "南境"));
    assert!(app.commit_map_text(false), "{:?}", app.io_error);
    app.undo(false);
    assert_eq!(
        app.project.map_index().maps["harbor"].placements["marker_1"].geometry,
        first
    );
    app.undo(true);
    assert!(app.save(), "{:?}", app.io_error);
    let reopened = worldline_core::project::Project::open(&root.join("world.wl")).unwrap();
    assert!(
        matches!(&reopened.map_index().maps["harbor"].placements["marker_1"].geometry,worldline_core::presentation::MapGeometry::Text { text, .. } if text=="南境")
    );
    let _ = std::fs::remove_dir_all(root);
}
#[test]
fn text_draft_rejects_stale_changes_preserves_input_and_cancel_has_no_history() {
    let (mut app, _, root) = app("text-stale");
    app.map_form.text_draft = Some(draft(&app, None, "草稿"));
    let history = app.history.len();
    app.map_revision = app.map_revision.next_presentation();
    assert!(!app.commit_map_text(false));
    assert_eq!(app.map_form.text_draft.as_ref().unwrap().text, "草稿");
    assert_eq!(app.history.len(), history);
    app.cancel_map_form();
    assert!(app.map_form.text_draft.is_none());
    assert_eq!(app.history.len(), history);
    let _ = std::fs::remove_dir_all(root);
}
#[test]
fn text_body_is_selectable_at_actual_glyph_extent_and_hidden_layer_is_not() {
    let ctx = egui::Context::default();
    let mut canvas = point_canvas();
    canvas.snapshot.layers[0].placements[0].geometry = MapGeometry::Text {
        position: NormalizedPoint::new(0.25, 0.25),
        text: "A long map label".into(),
        font_size: 24.0,
        color: "#ffffff".into(),
    };
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
        },
    );
    let size = canvas.text_sizes["point"].0;
    let origin = canvas
        .camera
        .normalized_to_screen(pos2(0.25, 0.25), canvas.viewport);
    let screen = origin + vec2(size.x * 0.8, size.y * 0.5);
    let point = canvas.camera.screen_to_normalized(screen, canvas.viewport);
    assert_eq!(
        canvas.hit_test(NormalizedPoint::new(point.x, point.y), 1.0),
        Some(("point".into(), GeometryHit::Vertex(0)))
    );
    canvas.set_layer_visible("places", false);
    assert!(canvas
        .hit_test(NormalizedPoint::new(point.x, point.y), 1.0)
        .is_none());
}
