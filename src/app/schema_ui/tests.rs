use super::*;
use egui::{pos2, vec2, Event, PointerButton, RawInput, Rect};
use std::sync::atomic::{AtomicUsize, Ordering};

fn app() -> (egui::Context, WorldeditApp) {
    static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = std::env::temp_dir().join(format!(
        "schema-ui-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
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

#[test]
fn character_constraint_in_113_previews_cancel_applies_and_undoes_without_coercion() {
    let (ctx, mut app) = app();
    let manifest = app.project.root.join(".world/project.json");
    app.project.set_authoring_document(&manifest, br#"{"schema_version":1,"language_version":"1.13","required_features":["content.object_refs.v1","content.character_refs.v1"]}"#.to_vec()).unwrap();
    let path = app.active_file.clone();
    let source = "schema city for entity entity_type place\n  field mayor_id mayor ref character required\ncharacter lin as \"林舟\"\nentity town kind place\n  property mayor = ref(\"character\", \"lin\")\nbind entity town to city\nevent start\n  -> END\n";
    app.project.set_text(&path, source.into()).unwrap();
    app.recompile();
    app.load_schema_file(path.clone());
    let baseline = app.project.content_baseline();
    app.schema_ui.source = source.replace("mayor ref character", "mayor text");
    click(&ctx, &mut app, "预览约束影响");
    assert!(app
        .schema_ui
        .preview
        .as_ref()
        .unwrap()
        .after_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SCH005"));
    assert_eq!(app.project.content_baseline(), baseline);
    click(&ctx, &mut app, "取消影响预览");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.schema_ui.has_unsubmitted_work());
    click(&ctx, &mut app, "预览约束影响");
    click(&ctx, &mut app, "应用约束草稿");
    assert!(app
        .project
        .document(&path)
        .unwrap()
        .contains("property mayor = ref(\"character\", \"lin\")"));
    app.undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
}

fn plan_frame(plan: &SchemaEditPreview, width: f32) -> egui::FullOutput {
    let ctx = egui::Context::default();
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 1200.0))),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw_plan(ui, plan));
        },
    )
}

fn visible(output: &egui::FullOutput, text: &str) -> bool {
    output
        .shapes
        .iter()
        .any(|shape| find(&shape.shape, text).is_some_and(|point| shape.clip_rect.contains(point)))
}

#[test]
fn missing_source_warning_is_visible_while_cancel_apply_and_undo_preserve_drafts() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    let original = app.schema_ui.source.clone();
    app.schema_ui.source.push_str("include \"missing.wl\"\n");
    let draft = app.schema_ui.source.clone();
    click(&ctx, &mut app, "预览约束影响");
    let plan = app.schema_ui.preview.as_ref().unwrap();
    assert!(!plan.complete);
    assert!(plan.instance_impacts.is_empty());
    let output = plan_frame(plan, 520.0);
    assert!(visible(
        &output,
        "部分影响预览：0 项已知字段/约束变化，0 个已知受影响实例"
    ));
    assert!(visible(
        &output,
        "无法确定全部实例影响；当前列表不是完整结果。允许保留错误草稿，运行或发布前必须修复。"
    ));
    assert!(visible(
        &output,
        "源码未完整加载（缺失、不可读、越界或 include 异常）"
    ));
    assert!(visible(&output, "当前没有已知受影响实例，不代表没有影响。"));
    assert_eq!(app.project.content_baseline(), baseline);
    click(&ctx, &mut app, "取消影响预览");
    assert!(app.schema_ui.preview.is_none());
    assert_eq!(app.schema_ui.source, draft);
    app.schema_ui.close(&ctx);
    app.open_schema_editor(&ctx);
    assert_eq!(app.schema_ui.source, draft);
    click(&ctx, &mut app, "预览约束影响");
    click(&ctx, &mut app, "应用约束草稿");
    assert_eq!(app.project.document(&app.active_file).unwrap(), draft);
    assert!(app.project.compile().has_errors());
    assert_eq!(app.history.len(), 1);
    app.undo(false);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.schema_ui.source, original);
}

#[test]
fn complete_zero_instances_and_unknown_zero_instances_have_distinct_visible_text() {
    let (_, mut app) = app();
    let source = app
        .schema_ui
        .source
        .replace("bind entity town to city\n", "");
    app.project
        .set_text(&app.active_file, source.clone())
        .unwrap();
    let request = SourceEditRequest {
        schema_version: 1,
        path: app
            .active_file
            .strip_prefix(&app.project.root)
            .unwrap()
            .to_owned(),
        expected_baseline: app.project.content_baseline(),
        source: source.replace("population number", "population text"),
    };
    let complete = app.project.preview_schema_edit(&request).unwrap();
    let incomplete = app
        .project
        .preview_schema_edit(&SourceEditRequest {
            source: request.source.clone() + "include \"../outside.wl\"\n",
            ..request
        })
        .unwrap();
    assert!(complete.complete);
    assert!(!incomplete.complete);
    assert!(complete.instance_impacts.is_empty());
    assert!(incomplete.instance_impacts.is_empty());
    for width in [320.0, 1000.0] {
        let known = plan_frame(&complete, width);
        let unknown = plan_frame(&incomplete, width);
        assert!(visible(
            &known,
            "完整影响预览：1 项字段/约束变化，0 个受影响实例"
        ));
        assert!(visible(&known, "已检查全部可用源码，确认没有受影响实例。"));
        assert!(!visible(
            &unknown,
            "已检查全部可用源码，确认没有受影响实例。"
        ));
        assert!(visible(
            &unknown,
            "部分影响预览：1 项已知字段/约束变化，0 个已知受影响实例"
        ));
        assert!(visible(
            &unknown,
            "当前没有已知受影响实例，不代表没有影响。"
        ));
    }
}

#[test]
fn incomplete_preview_shows_known_instances_and_core_reasons_without_reclassifying_diagnostics() {
    let (_, app) = app();
    let source = app
        .schema_ui
        .source
        .replace("population number", "population text")
        .replace(
            "property population = 0",
            "property population = 0\n  property population = 1",
        )
        + "include \"missing.wl\"\nperiod\nschema broken for entity\n  field incomplete\n";
    let request = SourceEditRequest {
        schema_version: 1,
        path: app
            .active_file
            .strip_prefix(&app.project.root)
            .unwrap()
            .to_owned(),
        expected_baseline: app.project.content_baseline(),
        source,
    };
    let mut plan = app.project.preview_schema_edit(&request).unwrap();
    assert_eq!(plan.incomplete_reasons.len(), 4);
    assert_eq!(plan.instance_impacts.len(), 1);
    // 展示原因只来自 core 字段；删掉诊断仍必须保留全部原因提示。
    plan.before_diagnostics.clear();
    plan.after_diagnostics.clear();
    let output = plan_frame(&plan, 520.0);
    for reason in &plan.incomplete_reasons {
        assert!(visible(&output, reason.label()), "原因不可见：{reason:?}");
    }
    assert!(output.shapes.iter().any(|shape| find(
        &shape.shape,
        "entity:town · [\"city\"] → [\"city\"] · 诊断 0 → 1"
    )
    .is_some()));
}

#[test]
fn repaired_source_remains_unknown_for_transition_then_becomes_complete_in_ui() {
    let (ctx, mut app) = app();
    let source = app.schema_ui.source.clone();
    app.project
        .set_text(
            &app.active_file,
            source.clone() + "include \"missing.wl\"\n",
        )
        .unwrap();
    app.recompile();
    app.load_schema_file(app.active_file.clone());
    app.schema_ui.source = source;
    click(&ctx, &mut app, "预览约束影响");
    assert!(!app.schema_ui.preview.as_ref().unwrap().complete);
    click(&ctx, &mut app, "应用约束草稿");
    click(&ctx, &mut app, "预览约束影响");
    let plan = app.schema_ui.preview.as_ref().unwrap();
    assert!(plan.complete);
    assert!(plan.incomplete_reasons.is_empty());
    assert!(visible(
        &plan_frame(plan, 520.0),
        "已检查全部可用源码，确认没有受影响实例。"
    ));
}
