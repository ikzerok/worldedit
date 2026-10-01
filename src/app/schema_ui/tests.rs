use super::*;
use egui::{pos2, vec2, Event, PointerButton, RawInput, Rect};

fn app() -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = std::env::temp_dir().join(format!(
        "schema-ui-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.project = worldline_core::project::Project::new(&root);
    let path = app.project.entry.clone();
    app.project.documents.retain(|file, _| file == &path);
    app.project.create_authoring_document(&root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.12","required_features":[],"maps":{},"graph_views":{}}"#.to_vec()).unwrap();
    app.project.set_text(&path, "schema city for entity entity_type place\n  field population_id population number required\nbind entity town to city\nentity town kind place\n  property population = 0\nevent start\n  正文。\n  -> END\n".into()).unwrap();
    app.active_file = path;
    app.recompile();
    app.open_schema_editor(&ctx);
    (ctx, app)
}
fn frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1500.0, 1000.0))),
            events,
            ..Default::default()
        },
        |ctx| app.schema_editor_window(ctx),
    )
}
fn find(shape: &egui::Shape, text: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(value) if value.galley.job.text == text => {
            Some(value.pos + value.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(values) => values.iter().find_map(|shape| find(shape, text)),
        _ => None,
    }
}
fn click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for _ in 0..3 {
        frame(ctx, app, Vec::new());
    }
    let output = frame(ctx, app, Vec::new());
    let point = output
        .shapes
        .iter()
        .find_map(|shape| find(&shape.shape, label))
        .unwrap_or_else(|| panic!("找不到按钮 {label}"));
    frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos: point,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
}

#[test]
fn schema_controls_preview_cancel_apply_undo_and_close_preserve_draft() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    app.schema_ui.source = app
        .schema_ui
        .source
        .replace("population number", "population text");
    click(&ctx, &mut app, "预览约束影响");
    let plan = app.schema_ui.preview.as_ref().unwrap();
    assert_eq!(plan.field_changes.len(), 1);
    assert_eq!(plan.instance_impacts.len(), 1);
    assert!(plan
        .after_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SCH005"));
    assert_eq!(app.project.content_baseline(), baseline);
    click(&ctx, &mut app, "取消影响预览");
    assert!(app.schema_ui.preview.is_none());
    assert!(app.schema_ui.has_unsubmitted_work());
    app.schema_ui.close(&ctx);
    assert!(app.dirty_draft_names().contains(&"持续资料约束草稿"));
    app.open_schema_editor(&ctx);
    assert!(app.schema_ui.source.contains("population text"));
    click(&ctx, &mut app, "预览约束影响");
    click(&ctx, &mut app, "应用约束草稿");
    assert_eq!(app.history.len(), 1);
    assert!(app
        .project
        .document(&app.active_file)
        .unwrap()
        .contains("population text"));
    app.undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
    frame(&ctx, &mut app, Vec::new());
    assert!(app.schema_ui.source.contains("population number"));
}

#[test]
fn schema_stale_preview_and_discard_cancel_do_not_overwrite() {
    let (ctx, mut app) = app();
    app.schema_ui.source = app
        .schema_ui
        .source
        .replace("population number", "population text");
    click(&ctx, &mut app, "预览约束影响");
    let path = app.active_file.clone();
    let source = app.project.document(&path).unwrap().to_owned() + "// external buffer\n";
    app.project.set_text(&path, source.clone()).unwrap();
    click(&ctx, &mut app, "应用约束草稿");
    assert_eq!(app.project.document(&path).unwrap(), source);
    assert!(app.schema_ui.error.as_ref().unwrap().contains("过期"));
    click(&ctx, &mut app, "丢弃约束草稿");
    click(&ctx, &mut app, "保留输入");
    assert!(app.schema_ui.has_unsubmitted_work());
    assert!(app.history.is_empty());
}
