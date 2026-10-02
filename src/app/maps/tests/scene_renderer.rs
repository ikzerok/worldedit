use super::*;

#[test]
fn transparent_crop_preserves_pixels_and_position() {
    let mut pixels = vec![0; 6 * 5 * 4];
    for (x, y) in [(2, 1), (3, 3)] {
        pixels[(y * 6 + x) * 4..(y * 6 + x) * 4 + 4].copy_from_slice(&[40, 20, 10, 80]);
    }
    let (origin, size, cropped) = crop_transparent(&pixels, [6, 5]);
    assert_eq!(origin, [2, 1]);
    assert_eq!(size, [2, 3]);
    assert_eq!(cropped.len(), 2 * 3 * 4);
    assert_eq!(&cropped[..4], &[40, 20, 10, 80]);
    assert_eq!(&cropped[cropped.len() - 4..], &[40, 20, 10, 80]);
    assert!(crop_transparent(&[0; 6 * 5 * 4], [6, 5]).2.is_empty());
}

#[test]
fn forged_pixel_length_drops_its_resource_account() {
    let budget = RenderBudget::with_limit(1024);
    let (permit, running) = budget.start([4, 4]).unwrap();
    drop(running);
    let key = RenderKey {
        generation: 1,
        extent: [4.0, 4.0],
        spec: RasterSpec {
            width: 4,
            height: 4,
            zoom: 1.0,
            pan: [0.0, 0.0],
            dpi: 1.0,
        },
    };
    let ready = RasterReady {
        key,
        pixels: Ok(vec![0; 63]),
        permit,
    };
    assert!(cache_result(&egui::Context::default(), ready).is_err());
    assert_eq!(budget.stats().reserved, 0);
    assert_eq!(budget.stats().cached, 0);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn actual_many_layer_renderers_queue_then_report_workset_capacity() {
    use worldline_core::vector_scene::{SceneGeometry, SceneNode};
    let budget = RenderBudget::with_limit(128 * 1024);
    let mut renderers: Vec<_> = (0..5000)
        .map(|_| SceneRenderer {
            budget: budget.clone(),
            ..Default::default()
        })
        .collect();
    let mut scene = MapScene::new(100.0, 100.0);
    scene.root_order.insert("layer".into(), vec!["rect".into()]);
    scene.nodes.insert(
        "rect".into(),
        SceneNode::new(
            "rect",
            "layer",
            SceneGeometry::Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
                rx: 0.0,
                ry: 0.0,
            },
        ),
    );
    let ctx = egui::Context::default();
    let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::splat(32.0));
    let camera = Camera2D::new(Vec2::splat(100.0));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(viewport),
                ..Default::default()
            },
            |ctx| {
                let painter =
                    egui::Painter::new(ctx.clone(), egui::LayerId::background(), viewport);
                for renderer in &mut renderers {
                    renderer.show(
                        &painter,
                        &scene,
                        1,
                        SceneView {
                            camera: &camera,
                            viewport,
                            extent: [100.0, 100.0],
                        },
                    );
                    let stats = budget.stats();
                    assert!(stats.active <= 2);
                    assert!(stats.cached + stats.reserved <= stats.limit);
                }
            },
        );
        if !renderers.iter().any(SceneRenderer::busy) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "render jobs failed to settle"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(renderers
        .iter()
        .any(|renderer| matches!(renderer.status, RenderStatus::Ready)));
    assert!(renderers.iter().any(|renderer| matches!(
        renderer.status,
        RenderStatus::Blocked(AdmissionError::Capacity { .. }, _)
    )));
    assert!(renderers
        .iter()
        .all(|renderer| !matches!(renderer.status, RenderStatus::Failed(_))));
    for renderer in &mut renderers {
        renderer.clear();
    }
    assert_eq!(budget.stats().active, 0);
    assert_eq!(budget.stats().cached + budget.stats().reserved, 0);
}
