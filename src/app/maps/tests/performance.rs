//! release headless 探针用于定位 CPU 开销；不是原生 FPS/GPU/present 验收。
use super::*;
use std::time::{Duration, Instant};

#[test]
#[ignore = "release-only fixed-fixture performance evidence"]
#[allow(clippy::assertions_on_constants)]
fn dense_scene_headless_release_profile() {
    assert!(!cfg!(debug_assertions), "run with --release");
    let entry = std::env::var_os("WORLDEDIT_DENSE_FIXTURE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../native-fixture/world.wl")
        });
    assert!(
        entry.is_file(),
        "set WORLDEDIT_DENSE_FIXTURE to the documented fixed fixture world.wl"
    );
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let setup = Instant::now();
    let mut app = super::super::WorldeditApp::new(&creation, Some(entry));
    let map_id = app
        .snapshot
        .as_ref()
        .unwrap()
        .map_index
        .maps
        .values()
        .find(|map| {
            map.scene
                .as_ref()
                .is_some_and(|scene| scene.nodes.len() == 5000)
        })
        .expect("fixture must contain exactly 5000 scene nodes")
        .id
        .clone();
    app.map_selection = Some(map_id);
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(976.0, 768.0));
    let run = |app: &mut super::super::WorldeditApp, time, events| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(screen),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ctx| app.map_tab(ctx),
        )
    };
    let cold = Instant::now();
    let _ = run(&mut app, 1.0, Vec::new());
    println!(
        "headless-only map-work-area=976x768 setup_ms={:.3} cold_update_ms={:.3}",
        setup.elapsed().as_secs_f64() * 1000.0,
        cold.elapsed().as_secs_f64() * 1000.0
    );
    let wait = Instant::now();
    let mut time = 2.0;
    loop {
        let _ = run(&mut app, time, Vec::new());
        time += 0.02;
        if app.map_canvas.scene.layers.values().all(|layer| {
            matches!(
                layer.renderer.status,
                super::scene_renderer::RenderStatus::Ready
            )
        }) {
            break;
        }
        assert!(
            wait.elapsed() < Duration::from_secs(30),
            "warm render did not settle; inspect resource status"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let baseline = app.project.content_baseline();
    app.map_canvas.set_mode(CanvasMode::Edit);
    app.map_canvas.set_tool(CanvasTool::Pan);
    for (case, panel) in [
        ("canvas-pan", MapPanel::Inspector),
        ("object-tree", MapPanel::Layers),
    ] {
        app.map_canvas.panel = panel;
        for round in 0..3 {
            let mut update = Vec::new();
            let mut total = Vec::new();
            for frame in 0..60 {
                let center = app.map_canvas.viewport.center();
                let mut events = Vec::new();
                if panel == MapPanel::Inspector {
                    let pointer = center + Vec2::new((frame % 8) as f32, 0.0);
                    events.push(egui::Event::PointerMoved(pointer));
                    if frame == 0 || frame == 59 {
                        events.push(egui::Event::PointerButton {
                            pos: pointer,
                            button: egui::PointerButton::Primary,
                            pressed: frame == 0,
                            modifiers: egui::Modifiers::NONE,
                        });
                    }
                } else if frame % 10 == 0 {
                    events.push(egui::Event::PointerMoved(Pos2::new(920.0, 400.0)));
                    events.push(egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: Vec2::new(0.0, -120.0),
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                let started = Instant::now();
                let output = run(&mut app, time, events);
                time += 1.0 / 60.0;
                update.push(started.elapsed().as_secs_f64() * 1000.0);
                let _ = ctx.tessellate(output.shapes, output.pixels_per_point);
                total.push(started.elapsed().as_secs_f64() * 1000.0);
            }
            print_samples(case, round, "update", update);
            print_samples(case, round, "update+tessellate", total);
        }
    }
    assert_eq!(app.project.content_baseline(), baseline);
}

fn print_samples(case: &str, round: usize, metric: &str, mut values: Vec<f64>) {
    values.sort_by(f64::total_cmp);
    println!(
        "headless {case} round={round} {metric} p50_ms={:.3} p95_ms={:.3} max_ms={:.3}",
        values[values.len() / 2],
        values[(values.len() * 95).div_ceil(100) - 1],
        values[values.len() - 1]
    );
}
