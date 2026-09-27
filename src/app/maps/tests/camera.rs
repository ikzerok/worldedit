use super::tests_support::*;
use super::*;
#[test]
fn opening_a_map_fits_its_extent_and_redraw_preserves_the_camera() {
    let root = test_workspace("initial-fit");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.clone()));
    let frame = || RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
        ..Default::default()
    };
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    let viewport = app.map_canvas.viewport();
    for corner in [pos2(0.0, 0.0), pos2(1.0, 1.0)] {
        let screen = app
            .map_canvas
            .camera()
            .normalized_to_screen(corner, viewport);
        assert!(
            viewport.expand(0.01).contains(screen),
            "地图边界应在画布内: {screen:?} {viewport:?}"
        );
    }
    app.map_canvas.camera.pan_by(vec2(17.0, 9.0));
    let camera = *app.map_canvas.camera();
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    assert_eq!(*app.map_canvas.camera(), camera);
    assert!(!app.project.is_dirty());
    let _ = std::fs::remove_dir_all(root);
}
