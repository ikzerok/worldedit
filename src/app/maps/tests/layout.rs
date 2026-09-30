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
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(("map-inspector-visible", true)), true));
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
    paint(&ctx, &mut app, 1100.);
    app.map_canvas.set_mode(CanvasMode::Edit);
    app.map_canvas.select_placement_id("lighthouse");
    for _ in 0..3 {
        paint(&ctx, &mut app, 1100.);
    }
    let output = paint(&ctx, &mut app, 1100.);
    let selected = text_position(&output, "标记 ID：lighthouse").expect("选中对象应在首屏");
    let navigation = text_position(&output, "地图浏览").unwrap();
    assert!(selected.y < navigation.y);
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
    ctx.data_mut(|data| data.insert_temp(egui::Id::new(("map-inspector-visible", true)), false));
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
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.map_canvas.set_mode(CanvasMode::Edit);
    for _ in 0..3 {
        paint(&ctx, &mut app, 1100.);
    }
    for _ in 0..5 {
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(1100., 700.))),
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
    let scrolled = paint(&ctx, &mut app, 1100.);
    assert!(
        text_position(&scrolled, "标记信息").is_none(),
        "先确实离开详情区域"
    );
    // 真实选择发生在画布，滚轮的剩余平滑输入不应继续路由到检查器。
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0., 0.), vec2(1100., 700.))),
            events: vec![Event::PointerMoved(pos2(400., 400.))],
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );
    app.map_canvas.select_placement_id("lighthouse");
    for _ in 0..3 {
        paint(&ctx, &mut app, 1100.);
    }
    let output = paint(&ctx, &mut app, 1100.);
    assert!(text_position(&output, "标记 ID：lighthouse").is_some());
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
