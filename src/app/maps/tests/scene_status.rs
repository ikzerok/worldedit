use super::super::render_budget::{AdmissionError, BudgetStats};
use super::*;

#[test]
fn render_status_transitions_never_resize_the_canvas_or_cause_redraw_feedback() {
    let mut authoring = SceneAuthoring::default();
    let mut scene = MapScene::new(400.0, 200.0);
    scene.root_order.insert("layer".into(), Vec::new());
    authoring.sync(Some(&scene));
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let stats = BudgetStats {
        limit: 1024,
        cached: 900,
        reserved: 0,
        active: 0,
        generation: 3,
    };
    let mut time = 0.0;
    for open in [false, true] {
        let mut height: Option<f32> = None;
        for status in [
            RenderStatus::Rendering,
            RenderStatus::Queued(AdmissionError::Waiting {
                required: 1000,
                stats,
            }),
            RenderStatus::Ready,
            RenderStatus::Blocked(
                AdmissionError::Capacity {
                    required: 1000,
                    stats,
                },
                3,
            ),
            RenderStatus::Failed(
                "失败原因很长，应保留详情但不能反过来改变场景的视口尺寸".repeat(8),
            ),
            RenderStatus::Idle,
            RenderStatus::Ready,
        ] {
            authoring.layers.get_mut("layer").unwrap().renderer.status = status;
            for _ in 0..3 {
                time += 0.1;
                let mut current = 0.0;
                let _ = ctx.run(
                    egui::RawInput {
                        time: Some(time),
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(340.0, 500.0),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            let id = ui.make_persistent_id("scene-render-status");
                            let mut state =
                                egui::collapsing_header::CollapsingState::load_with_default_open(
                                    ctx, id, open,
                                );
                            state.set_open(open);
                            state.store(ctx);
                            let before = ui.cursor().top();
                            authoring.render_status_panel(ui);
                            current = ui.cursor().top() - before;
                        });
                    },
                );
                if let Some(expected) = height {
                    assert!(
                        (current - expected).abs() <= 0.1,
                        "open={open}: status panel height {current} changed from {expected}"
                    );
                } else {
                    height = Some(current);
                }
            }
        }
    }
}

#[test]
fn temporary_hide_all_releases_derived_views_without_editing_document_defaults() {
    let mut canvas = super::super::tests_support::point_canvas();
    let original = canvas.core_snapshot.layers.clone();
    let source = MapScene::new(400.0, 400.0);
    canvas.sync_scene(Some(&source));
    canvas.set_all_layers_visible(false);
    assert!(canvas.snapshot.layers.iter().all(|layer| !layer.visible));
    assert_eq!(canvas.core_snapshot.layers, original);
    assert_eq!(canvas.scene.source, Some(source.clone()));
    canvas.set_all_layers_visible(true);
    assert!(canvas.snapshot.layers.iter().all(|layer| layer.visible));
    assert_eq!(canvas.core_snapshot.layers, original);
    assert_eq!(canvas.scene.source, Some(source));
    assert!(!canvas.has_uncommitted_work());
}
