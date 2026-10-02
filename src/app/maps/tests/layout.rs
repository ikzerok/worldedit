//! 地图画布的宽窄布局与选中上下文回归。
use super::tests_support::*;
use super::*;

fn paint(
    ctx: &egui::Context,
    app: &mut super::super::WorldeditApp,
    width: f32,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(width, 700.))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    )
}

fn text_position(output: &egui::FullOutput, needle: &str) -> Option<egui::Pos2> {
    fn find(shape: &egui::Shape, needle: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text().contains(needle) => Some(text.pos),
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, needle)),
            _ => None,
        }
    }
    output.shapes.iter().find_map(|shape| {
        find(&shape.shape, needle).filter(|position| shape.clip_rect.contains(*position))
    })
}

#[test]
fn narrow_map_defaults_to_canvas_and_can_restore_inspector_without_writes() {
    let root = test_workspace("layout-narrow");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    let baseline = app.project.content_baseline();
    for _ in 0..3 {
        paint(&ctx, &mut app, 640.);
    }
    let output = paint(&ctx, &mut app, 640.);
    assert!(text_position(&output, "显示地图面板").is_some());
    assert!(app.map_canvas.viewport.width() > 550.);
    let camera = *app.map_canvas.camera();
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("map-inspector-visible"), true));
    for _ in 0..3 {
        paint(&ctx, &mut app, 640.);
    }
    let output = paint(&ctx, &mut app, 640.);
    assert!(text_position(&output, "收起地图面板").is_some());
    assert!(text_position(&output, "地图浏览").is_some());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(*app.map_canvas.camera(), camera);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn selected_map_context_precedes_navigation_and_long_edit_form() {
    let root = test_workspace("layout-selection");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    paint(&ctx, &mut app, 1400.);
    app.map_canvas.set_mode(CanvasMode::Edit);
    app.map_canvas.select_placement_id("lighthouse");
    for _ in 0..3 {
        paint(&ctx, &mut app, 1400.);
    }
    let output = paint(&ctx, &mut app, 1400.);
    let selected = text_position(&output, "标记 ID：lighthouse").expect("选中对象应在首屏");
    let navigation = text_position(&output, "地图浏览").unwrap();
    assert!(
        selected.x > navigation.x,
        "当前选择在右检查器，地图导航在左索引"
    );
    assert!(text_position(&output, "打开资料").is_some());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn unfinished_map_form_keeps_inspector_accessible_in_narrow_layout() {
    let root = test_workspace("layout-form");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    paint(&ctx, &mut app, 640.);
    app.map_canvas.set_mode(CanvasMode::Edit);
    app.map_form.annotation = "保留草稿".into();
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("map-inspector-visible"), false));
    for _ in 0..3 {
        paint(&ctx, &mut app, 640.);
    }
    let output = paint(&ctx, &mut app, 640.);
    assert!(text_position(&output, "收起地图面板").is_some());
    assert!(text_position(&output, "地图浏览").is_some());
    assert_eq!(app.map_form.annotation, "保留草稿");
    assert!(app.map_canvas.viewport.width() > 200.);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn selecting_map_object_returns_scrolled_inspector_to_its_details() {
    let root = test_workspace("layout-selection-after-scroll");
    let path = root.join(".world/maps/harbor.json");
    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    document["placements"]["lighthouse"]["annotation"] = (0..80)
        .map(|i| format!("第 {i} 行灯塔说明\n"))
        .collect::<String>()
        .into();
    let mut second = document["placements"]["lighthouse"].clone();
    second["geometry"]["position"] = serde_json::json!([0.6, 0.55]);
    second["annotation"] = "第二个对象".into();
    document["placements"]["second"] = second;
    std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.map_canvas.set_mode(CanvasMode::Edit);
    for _ in 0..3 {
        paint(&ctx, &mut app, 1100.);
    }
    app.map_canvas.select_placement_id("lighthouse");
    let output = paint(&ctx, &mut app, 1100.);
    assert!(text_position(&output, "标记 ID：lighthouse").is_some());
    let screen = Rect::from_min_size(pos2(0., 0.), vec2(1100., 700.));
    let mut last = output;
    for index in 0..5 {
        last = ctx.run(
            RawInput {
                screen_rect: Some(screen),
                time: Some(10.0 + index as f64 * 0.1),
                events: vec![
                    Event::PointerMoved(pos2(1000., 550.)),
                    Event::MouseWheel {
                        unit: MouseWheelUnit::Point,
                        delta: vec2(0., -500.),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| app.map_tab(ctx),
        );
    }
    assert!(
        text_position(&last, "标记 ID：lighthouse").is_none(),
        "确实将原对象详情滚出右检查器"
    );
    for index in 0..20 {
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen),
                time: Some(11.0 + index as f64 * 0.1),
                ..Default::default()
            },
            |ctx| app.map_tab(ctx),
        );
    }
    let camera = *app.map_canvas.camera();
    let target = app
        .map_canvas
        .camera
        .normalized_to_screen(pos2(0.6, 0.55), app.map_canvas.viewport);
    for (time, pressed) in [(20.0, true), (20.1, false)] {
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen),
                time: Some(time),
                events: vec![
                    Event::PointerMoved(target),
                    Event::PointerButton {
                        pos: target,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| app.map_tab(ctx),
        );
    }
    let output = paint(&ctx, &mut app, 1100.);
    assert!(text_position(&output, "标记 ID：second").is_some());
    assert_eq!(*app.map_canvas.camera(), camera);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn native_minimum_window_work_area_uses_compact_inspector_default() {
    let root = test_workspace("layout-native-minimum");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    // 原生最小窗口1040，左侧导航约212，留给地图工作区约828。
    for _ in 0..3 {
        paint(&ctx, &mut app, 828.);
    }
    let output = paint(&ctx, &mut app, 828.);
    assert!(text_position(&output, "显示地图面板").is_some());
    assert!(app.map_canvas.viewport.width() > 700.);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn map_diagnostic_long_source_stays_inside_inspector_and_keeps_open_action_visible() {
    let root = test_workspace("layout-diagnostic-source");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    let file = root
        .join(format!(
            ".world/maps/{}map.json",
            "very-long-segment/".repeat(20)
        ))
        .display()
        .to_string();
    app.snapshot
        .as_mut()
        .unwrap()
        .map_index
        .diagnostics
        .push(worldline_core::Diagnostic::error(
            "MAP007",
            &file,
            worldline_core::Span::new(1, 1, 1),
            "标记缺少 target_ref",
        ));
    for _ in 0..3 {
        paint(&ctx, &mut app, 1100.0);
    }
    let output = paint(&ctx, &mut app, 1100.0);
    assert!(
        text_position(&output, "打开原文").is_some(),
        "source action must stay reachable in the inspector"
    );
    fn find(shape: &egui::Shape, clip: Rect) -> bool {
        match shape {
            egui::Shape::Text(text)
                if text.galley.text().contains("very-long-segment") && clip.contains(text.pos) =>
            {
                assert!(
                    text.pos.x + text.galley.size().x <= 1100.0,
                    "diagnostic source escaped the window instead of truncating"
                );
                true
            }
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| find(shape, clip)),
            _ => false,
        }
    }
    assert!(output
        .shapes
        .iter()
        .any(|shape| find(&shape.shape, shape.clip_rect)));
    let _ = std::fs::remove_dir_all(root);
}

fn workspace_frame(
    ctx: &egui::Context,
    app: &mut super::super::WorldeditApp,
    size: egui::Vec2,
    navigation: bool,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| {
            if navigation {
                egui::SidePanel::left("layout-test-global-nav")
                    .exact_width(212.0)
                    .show(ctx, |ui| {
                        ui.label("全局导航");
                    });
            }
            app.map_tab(ctx);
        },
    )
}

#[test]
fn browse_layout_releases_empty_inspector_and_budgets_remaining_canvas() {
    for (size, navigation) in [
        (vec2(1040.0, 660.0), true),
        (vec2(1188.0, 848.0), true),
        (vec2(1188.0, 848.0), false),
        (vec2(1600.0, 1000.0), true),
    ] {
        let root = test_workspace(&format!("content-first-{}-{navigation}", size.x));
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
        let baseline = app.project.content_baseline();
        for _ in 0..5 {
            workspace_frame(&ctx, &mut app, size, navigation, vec![]);
        }
        let output = workspace_frame(&ctx, &mut app, size, navigation, vec![]);
        assert!(text_position(&output, "显示地图面板").is_some());
        // 目录说明也含“标记信息”；检查仅检查器拥有的标签，避免子串误报。
        assert!(text_position(&output, "当前选择").is_none());
        assert!(text_position(&output, "点击画布上的标记查看信息。").is_none());
        assert!(app.map_canvas.viewport.width() >= 600.0);
        app.map_canvas.select_placement_id("lighthouse");
        for _ in 0..5 {
            workspace_frame(&ctx, &mut app, size, navigation, vec![]);
        }
        let output = workspace_frame(&ctx, &mut app, size, navigation, vec![]);
        assert!(text_position(&output, "标记 ID：lighthouse").is_some());
        if size.x == 1188.0 && navigation {
            assert!(text_position(&output, "显示地图目录").is_some());
            assert!(app.map_canvas.viewport.width() >= 600.0);
        }
        assert_eq!(app.project.content_baseline(), baseline);
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn resized_panels_and_larger_text_trigger_compact_map_without_losing_controls() {
    let root = test_workspace("content-first-resized-panels");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    for _ in 0..3 {
        paint(&ctx, &mut app, 1300.0);
    }
    app.map_canvas.select_placement_id("lighthouse");
    ctx.data_mut(|data| {
        data.insert_persisted(
            egui::Id::new("map-index"),
            egui::containers::panel::PanelState {
                rect: Rect::from_min_size(egui::Pos2::ZERO, vec2(360.0, 700.0)),
            },
        );
        data.insert_persisted(
            egui::Id::new("map-inspector"),
            egui::containers::panel::PanelState {
                rect: Rect::from_min_size(egui::Pos2::ZERO, vec2(420.0, 700.0)),
            },
        );
    });
    for _ in 0..3 {
        paint(&ctx, &mut app, 1300.0);
    }
    let output = paint(&ctx, &mut app, 1300.0);
    assert!(text_position(&output, "显示地图目录").is_some());
    assert!(app.map_canvas.viewport.width() > 800.0);
    let camera = *app.map_canvas.camera();
    // 手动展开仍可覆盖自动布局，不删命令、不写工程也不重置镜头。
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("map-index-visible"), true));
    for _ in 0..3 {
        paint(&ctx, &mut app, 1300.0);
    }
    let output = paint(&ctx, &mut app, 1300.0);
    assert!(text_position(&output, "收起地图目录").is_some());
    assert!(text_position(&output, "地图浏览").is_some());
    assert_eq!(*app.map_canvas.camera(), camera);
    ctx.data_mut(|data| data.remove::<bool>(egui::Id::new("map-index-visible")));
    ctx.style_mut(|style| {
        for font in style.text_styles.values_mut() {
            font.size *= 1.5;
        }
    });
    for _ in 0..3 {
        paint(&ctx, &mut app, 1500.0);
    }
    let output = paint(&ctx, &mut app, 1500.0);
    assert!(text_position(&output, "显示地图目录").is_some());
    assert!(text_position(&output, "收起地图面板").is_some());
    let _ = std::fs::remove_dir_all(root);
}
