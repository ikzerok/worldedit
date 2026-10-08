//! 紧凑源码的问题提示始终可见，详情在真实菜单内可滚动访问。
use super::super::tests::app;
use crate::app::{Tab, WorldeditApp};
use crate::theme::{Density, PaletteId, StylePreset, ThemeMode};
use egui::{Context, Event, Pos2, Rect};

fn frame(ctx: &Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0))),
            events,
            ..Default::default()
        },
        |ctx| eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest()),
    )
}

fn settle(ctx: &Context, app: &mut WorldeditApp) -> egui::FullOutput {
    for _ in 0..6 {
        frame(ctx, app, vec![]);
    }
    frame(ctx, app, vec![])
}

fn label(output: &egui::FullOutput, text: &str) -> Option<Rect> {
    let screen = Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 300.0));
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(value) if value.galley.text() == text => {
            let rect = value.galley.rect.translate(value.pos.to_vec2());
            shape
                .clip_rect
                .intersect(screen)
                .contains_rect(rect)
                .then_some(rect)
        }
        _ => None,
    })
}

fn source_clip(output: &egui::FullOutput, source: &str) -> Rect {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(value) if value.galley.text() == source => Some(shape.clip_rect),
            _ => None,
        })
        .expect("真实源码必须仍然绘制")
}

fn viewport_geometry(
    app: &WorldeditApp,
    output: &egui::FullOutput,
    source: &str,
) -> (Pos2, serde_json::Value) {
    let origin = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(value) if value.galley.text() == source => Some(value.pos),
            _ => None,
        })
        .unwrap();
    let (path, view) = app.personal.source_view.as_ref().unwrap();
    assert_eq!(path, &app.active_file);
    let layout = serde_json::to_value(view).unwrap()["layout"].clone();
    // layout来自ScrollArea.inner_rect；加回滚动偏移得到同一正文的布局起点。
    (
        origin + egui::vec2(app.personal.source_scroll[0], app.personal.source_scroll[1]),
        layout,
    )
}

fn trace_geometry(stage: &str, app: &WorldeditApp, output: &egui::FullOutput, source: &str) {
    eprintln!(
        "compact source {stage}: paint_clip={:?}; scroll={:?}; origin/layout={:?}",
        source_clip(output, source),
        app.personal.source_scroll,
        viewport_geometry(app, output, source),
    );
}

fn click(ctx: &Context, app: &mut WorldeditApp, text: &str) {
    let mut output = settle(ctx, app);
    for _ in 0..32 {
        if let Some(rect) = label(&output, text) {
            let point = rect.center();
            for pressed in [true, false] {
                frame(
                    ctx,
                    app,
                    vec![
                        Event::PointerMoved(point),
                        Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            return;
        }
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(egui::pos2(250.0, 150.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -64.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        output = settle(ctx, app);
    }
    panic!("实际菜单滚动后仍无法访问：{text}");
}

#[test]
fn compact_source_problem_status_is_visible_stable_and_details_keep_input_guard() {
    let (ctx, mut app) = app();
    app.personal.pending_restore = false;
    app.personal.settings.navigation = false;
    app.personal.settings.diagnostics = false;
    app.personal.settings.style = StylePreset::Manuscript;
    app.personal.settings.palette = PaletteId::Vellum;
    app.personal.settings.theme = ThemeMode::Light;
    app.personal.settings.density = Density::Spacious;
    app.personal.settings.ui_scale = 2.0;
    app.personal.settings.reduce_motion = true;
    app.tab = Tab::Edit;
    let path = app.active_file.clone();
    let source = app.project.document(&path).unwrap().to_owned();
    let baseline = app.project.content_baseline();
    let unpositioned = settle(&ctx, &mut app);
    trace_geometry("before positioning", &app, &unpositioned, &source);
    let report = app.problems.report.as_ref().unwrap();
    let entry = report
        .entries
        .iter()
        .find(|entry| {
            app.project
                .problem_location(report, &entry.id, None)
                .is_ok_and(|location| {
                    location.char_range.is_some()
                        && location
                            .path
                            .as_ref()
                            .is_some_and(|relative| app.project.root.join(relative) == path)
                })
        })
        .unwrap()
        .clone();
    let location = app
        .project
        .problem_location(report, &entry.id, None)
        .unwrap();
    let range = location.byte_range.unwrap();
    // 先以同一core范围固定焦点、选区和滚动，只让问题身份成为后续帧的变量。
    // TextEdit在首行的内边距会收窄paint clip；不能把离开首行后的clip变化误判为页头位移。
    crate::app::search::request_diagnostic_selection(
        &ctx,
        path.clone(),
        source.clone(),
        range.start..range.end,
    );
    let initial = settle(&ctx, &mut app);
    trace_geometry("same target without problem", &app, &initial, &source);
    assert_eq!(ctx.pixels_per_point(), 2.0);
    let clip = source_clip(&initial, &source);
    let geometry = viewport_geometry(&app, &initial, &source);
    assert!(
        clip.height() >= app.personal.settings.source_size * app.personal.settings.line_spacing
    );
    let tools = label(&initial, "源码工具").unwrap();
    assert!(app.problem_source_status(&path).is_none());
    app.problems.select(entry.id);
    app.locate_problem(&ctx, None);
    let current = settle(&ctx, &mut app);
    trace_geometry("current problem", &app, &current, &source);
    let severity = super::super::view::severity_label(entry.severity);
    assert_eq!(app.problem_source_status(&path).unwrap().0, severity);
    assert!(
        label(&current, severity).is_some(),
        "当前问题严重度须在闭合页头中可见"
    );
    assert_eq!(viewport_geometry(&app, &current, &source), geometry);
    assert_eq!(source_clip(&current, &source), clip);
    assert_eq!(label(&current, "源码工具"), Some(tools));
    app.problems
        .reject_source_navigation("来源已变化，等待重检".into());
    let stale = settle(&ctx, &mut app);
    trace_geometry("stale problem", &app, &stale, &source);
    assert!(label(&stale, "待重检").is_some(), "失效问题不能静默隐藏");
    assert_eq!(viewport_geometry(&app, &stale, &source), geometry);
    assert_eq!(source_clip(&stale, &source), clip);
    assert_eq!(label(&stale, "源码工具"), Some(tools));
    click(&ctx, &mut app, "源码工具");
    app.ime_source_draft = Some((path.clone(), source.clone(), source.clone()));
    click(&ctx, &mut app, "回到问题详情");
    assert!(
        !app.personal.settings.diagnostics,
        "未提交输入时菜单动作仍受原保护"
    );
    assert!(app.ime_source_draft.is_some());
    app.ime_source_draft = None;
    if label(&settle(&ctx, &mut app), "回到问题详情").is_none() {
        click(&ctx, &mut app, "源码工具");
    }
    click(&ctx, &mut app, "回到问题详情");
    assert!(app.personal.settings.diagnostics);
    assert!(app.problems.narrow_detail);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&path).unwrap(), source);
}
