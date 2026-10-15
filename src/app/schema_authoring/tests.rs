//! 真实 egui 表单回归；不代表物理 IME、原生窗口或读屏认证。
use super::*;
mod archive_roundtrip;
mod full_app;
mod generic;
mod source_binding;
use crate::app::WorldeditApp;
use egui::{pos2, vec2, Event, PointerButton, RawInput, Rect};
use std::sync::atomic::{AtomicUsize, Ordering};

const SOURCE: &str = r#"schema place_data for entity entity_type place
  field population_id population number required
  field public_id public boolean required
  field note_id note text required
  field category_id category enum "city" "town"
  field owner_id owner ref entity entity_type organization
schema world_data for world
  field title_id title text
schema person_data for character
  field age_id age number
schema tie_data for relation
  field strength_id strength number
world shared as "共同世界"
character shared as "林舟"
entity shared kind place as "共同城"
  property population = 0
  property public = false
  property note = ""
  property category = "非法但保留"
  property unknown = "不改写"
entity council kind organization as "议会"
relation_type knows as "认识"
relation_def shared type knows from character shared to entity shared
bind entity shared to place_data
event start
  正文。
  -> END
"#;
fn app() -> (egui::Context, WorldeditApp) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = std::env::temp_dir().join(format!(
        "schema-fields-ui-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).expect("测试目录必须独占创建，不能复用既有路径");
    app.project = worldline_core::project::Project::new(&root);
    let entry = app.project.entry.clone();
    app.project.documents.retain(|path, _| path == &entry);
    app.project.create_authoring_document(&root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.13","required_features":["content.entities.v1","content.relations.v1","content.object_refs.v1"],"maps":{},"graph_views":{}}"#.to_vec()).unwrap();
    app.project.set_text(&entry, SOURCE.into()).unwrap();
    app.active_file = entry;
    app.recompile();
    app.open_schema_authoring(&ctx, TargetRef::new("entity", "shared"));
    (ctx, app)
}
fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)),
            events,
            ..Default::default()
        },
        |ctx| app.schema_authoring_window(ctx),
    )
}
fn find(shape: &egui::Shape, label: &str) -> Option<Rect> {
    match shape {
        egui::Shape::Text(text) if text.galley.text() == label => {
            Some(text.galley.rect.translate(text.pos.to_vec2()))
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, label)),
        _ => None,
    }
}
fn click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let size = vec2(1200.0, 900.0);
    for _ in 0..3 {
        frame(ctx, app, size, vec![]);
    }
    let output = frame(ctx, app, size, vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| find(&shape.shape, label))
        .unwrap_or_else(|| panic!("missing {label}"))
        .center();
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            size,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}
fn session(app: &mut WorldeditApp) -> &mut Session {
    app.schema_authoring
        .sessions
        .get_mut(&TargetRef::new("entity", "shared"))
        .unwrap()
}
#[test]
fn presence_invalid_value_and_filtered_reference_come_from_core() {
    let (_, mut app) = app();
    let session = session(&mut app);
    assert_eq!(
        session.values["population_id"],
        Some(PropertyValue::Num(0.0))
    );
    assert_eq!(
        session.values["public_id"],
        Some(PropertyValue::Bool(false))
    );
    assert_eq!(
        session.values["note_id"],
        Some(PropertyValue::Str(String::new()))
    );
    assert_eq!(session.values["owner_id"], None);
    assert_eq!(
        session.values["category_id"],
        Some(PropertyValue::Str("非法但保留".into()))
    );
    let owner = session
        .projection
        .fields
        .iter()
        .find(|field| field.id == "owner_id")
        .unwrap();
    assert_eq!(owner.reference_candidates.len(), 1);
    assert_eq!(
        owner.reference_candidates[0].target,
        TargetRef::new("entity", "council")
    );
    assert!(!session.dirty());
    session.values.insert("population_id".into(), None);
    assert!(
        matches!(&session.field_changes()[0], SchemaFieldMutation::Remove { field_id, .. } if field_id == "population_id")
    );
}
#[test]
fn switching_full_targets_closing_and_cancel_keep_each_draft_and_export_guard() {
    let (ctx, mut app) = app();
    session(&mut app).values.insert(
        "note_id".into(),
        Some(PropertyValue::Str("未应用中文".into())),
    );
    for kind in ["world", "character", "relation", "entity"] {
        app.open_schema_authoring(&ctx, TargetRef::new(kind, "shared"));
    }
    assert_eq!(app.schema_authoring.sessions.len(), 4);
    click(&ctx, &mut app, "取消（保留输入）");
    assert!(!app.schema_authoring.open);
    assert!(app.dirty_draft_names().contains(&DRAFT_KIND));
    assert!(app
        .unapplied_export_inputs()
        .iter()
        .any(|input| input.kind == DRAFT_KIND && input.source.contains("entity:shared")));
    app.return_to_export_input(DRAFT_KIND);
    assert!(app.schema_authoring.open);
    assert_eq!(
        session(&mut app).values["note_id"],
        Some(PropertyValue::Str("未应用中文".into()))
    );
}
#[test]
fn preview_apply_is_atomic_retains_unknown_and_second_click_is_inert() {
    let (ctx, mut app) = app();
    session(&mut app).values.insert(
        "category_id".into(),
        Some(PropertyValue::Str("city".into())),
    );
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, "预览字段修改");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(!session(&mut app).preview.as_ref().unwrap().has_errors);
    click(&ctx, &mut app, "应用到工程");
    assert_eq!(app.history.len(), 1);
    assert!(!app.schema_authoring.has_unsubmitted_work());
    let applied = app.project.content_baseline();
    assert_ne!(baseline, applied);
    assert!(app
        .project
        .document(&app.active_file)
        .unwrap()
        .contains("property unknown = \"不改写\""));
    click(&ctx, &mut app, "应用到工程");
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.project.content_baseline(), applied);
    app.undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
}
#[test]
fn stale_preview_keeps_input_and_cannot_apply() {
    let (ctx, mut app) = app();
    session(&mut app).values.insert(
        "category_id".into(),
        Some(PropertyValue::Str("city".into())),
    );
    click(&ctx, &mut app, "预览字段修改");
    let path = app.active_file.clone();
    let source = format!("{}\n// 外改\n", app.project.document(&path).unwrap());
    app.project.set_text(&path, source).unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, "应用到工程");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(session(&mut app).dirty());
    assert!(app.history.is_empty());
}
#[test]
fn actions_remain_visible_with_long_fields_narrow_view_and_double_size_text() {
    for (size, scale) in [
        (vec2(1188.0, 848.0), 1.0),
        (vec2(580.0, 620.0), 1.0),
        (vec2(900.0, 750.0), 2.0),
    ] {
        let (ctx, mut app) = app();
        ctx.style_mut(|style| {
            for font in style.text_styles.values_mut() {
                font.size *= scale;
            }
        });
        session(&mut app).values.insert(
            "note_id".into(),
            Some(PropertyValue::Str("长输入正文\n".repeat(100))),
        );
        for _ in 0..5 {
            frame(&ctx, &mut app, size, vec![]);
        }
        let output = frame(&ctx, &mut app, size, vec![]);
        for label in ["预览字段修改", "应用到工程", "取消（保留输入）"] {
            let rect = output
                .shapes
                .iter()
                .find_map(|shape| {
                    find(&shape.shape, label).filter(|rect| shape.clip_rect.contains_rect(*rect))
                })
                .unwrap_or_else(|| panic!("not visible {label}: {size:?}/{scale}"));
            assert!(Rect::from_min_size(pos2(0.0, 0.0), size).contains_rect(rect));
        }
    }
}

#[test]
fn actual_form_idle_scroll_and_view_changes_do_not_compile_or_replace_input() {
    let (ctx, mut app) = app();
    session(&mut app).values.insert(
        "note_id".into(),
        Some(PropertyValue::Str("尚未应用".into())),
    );
    let count = app.semantic_compile_count;
    let signature = app.schema_authoring.signature();
    for index in 0..120 {
        if index == 60 {
            ctx.set_visuals(egui::Visuals::light());
        }
        frame(
            &ctx,
            &mut app,
            vec2(1000.0, 700.0),
            vec![
                Event::PointerMoved(pos2(600.0, 500.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, if index % 2 == 0 { -30.0 } else { 30.0 }),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(app.semantic_compile_count, count);
    assert_eq!(app.schema_authoring.signature(), signature);
}
#[test]
fn binding_selection_round_trip_preserves_values_and_never_writes_before_apply() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    let values = session(&mut app).values.clone();
    session(&mut app).selected_schema = None;
    click(&ctx, &mut app, "预览所选绑定");
    assert!(session(&mut app).projection.fields.is_empty());
    assert_eq!(app.project.content_baseline(), baseline);
    session(&mut app).selected_schema = Some("place_data".into());
    click(&ctx, &mut app, "预览所选绑定");
    assert_eq!(session(&mut app).values, values);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(!session(&mut app).dirty());
}
#[test]
fn source_return_reopens_same_target_and_retains_text() {
    let (ctx, mut app) = app();
    session(&mut app).values.insert(
        "note_id".into(),
        Some(PropertyValue::Str("来源往返稿".into())),
    );
    let signature = app.schema_authoring.signature();
    // 来源链接统一经 jump_to_file，返回条只恢复窗口，不重建Session。
    let source = session(&mut app).base.object.file.clone();
    app.jump_to_file(&source, 1, 1);
    app.schema_authoring.open = false;
    app.schema_authoring.source_return = true;
    click(&ctx, &mut app, "返回资料表单");
    assert!(app.schema_authoring.open);
    assert_eq!(app.schema_authoring.signature(), signature);
}
#[test]
fn ime_does_not_apply_cancel_or_disable_the_field_input() {
    let (ctx, mut app) = app();
    session(&mut app).values.insert(
        "category_id".into(),
        Some(PropertyValue::Str("city".into())),
    );
    click(&ctx, &mut app, "预览字段修改");
    let baseline = app.project.content_baseline();
    app.command_palette.ime = true;
    click(&ctx, &mut app, "应用到工程");
    click(&ctx, &mut app, "取消（保留输入）");
    assert!(app.schema_authoring.open);
    assert_eq!(app.project.content_baseline(), baseline);
    let mut enabled_input = false;
    let mut field = session(&mut app)
        .projection
        .fields
        .iter()
        .find(|field| field.id == "note_id")
        .unwrap()
        .clone();
    field.value = Some(PropertyValue::Str("组合中的正文".into()));
    let _ = ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut session = Session::new(session(&mut app).base.clone());
            session.projection.fields = vec![field.clone()];
            super::fields::render(ui, &mut session, true, false);
            enabled_input = ui
                .ctx()
                .read_response(super::fields::input_id(&session.base.target, "note_id"))
                .expect("真实TextEdit应存在")
                .enabled();
        });
    });
    assert!(enabled_input);
}

#[test]
fn newly_inserted_property_shifts_candidate_diagnostic_but_not_source_anchor() {
    let (ctx, mut app) = app();
    let before_line = session(&mut app)
        .projection
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "SCH006")
        .unwrap()
        .span
        .line;
    let before_source = session(&mut app)
        .projection
        .fields
        .iter()
        .find(|field| field.id == "category_id")
        .unwrap()
        .property_source
        .clone();
    session(&mut app).values.insert(
        "owner_id".into(),
        Some(PropertyValue::Ref(TargetRef::new("entity", "council"))),
    );
    click(&ctx, &mut app, "预览字段修改");
    let session = session(&mut app);
    let candidate_line = session
        .preview
        .as_ref()
        .unwrap()
        .after
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "SCH006")
        .unwrap()
        .span
        .line;
    assert!(
        candidate_line > before_line,
        "新属性在对象头后插入，候选来源应已移位"
    );
    assert!(!super::window::diagnostic_sources_current(session));
    assert_eq!(
        session
            .projection
            .fields
            .iter()
            .find(|field| field.id == "category_id")
            .unwrap()
            .property_source,
        before_source
    );
}

#[test]
fn world_character_relation_bind_and_fill_using_same_working_sheet() {
    for (kind, schema, field, value) in [
        (
            "world",
            "world_data",
            "title_id",
            PropertyValue::Str("共同设定".into()),
        ),
        (
            "character",
            "person_data",
            "age_id",
            PropertyValue::Num(0.0),
        ),
        (
            "relation",
            "tie_data",
            "strength_id",
            PropertyValue::Num(0.0),
        ),
    ] {
        let (ctx, mut app) = app();
        let entry = app.active_file.clone();
        app.project
            .set_text(&entry, SOURCE.replace("非法但保留", "city"))
            .unwrap();
        app.recompile();
        let target = TargetRef::new(kind, "shared");
        app.open_schema_authoring(&ctx, target.clone());
        app.schema_authoring
            .sessions
            .get_mut(&target)
            .unwrap()
            .selected_schema = Some(schema.into());
        let baseline = app.project.content_baseline();
        click(&ctx, &mut app, "预览所选绑定");
        assert_eq!(app.project.content_baseline(), baseline);
        app.schema_authoring
            .sessions
            .get_mut(&target)
            .unwrap()
            .values
            .insert(field.into(), Some(value.clone()));
        click(&ctx, &mut app, "预览字段修改");
        let form = app.schema_authoring.sessions.get_mut(&target).unwrap();
        let plan = form.preview.as_ref().unwrap();
        assert_eq!(
            plan.runtime_changed,
            plan.before_fingerprint != plan.after_fingerprint
        );
        assert!(!plan.has_errors);
        let runtime_changed = plan.runtime_changed;
        if runtime_changed {
            click(&ctx, &mut app, "应用到工程");
            assert_eq!(app.project.content_baseline(), baseline);
            click(&ctx, &mut app, "确认应用这次运行指纹变更");
        }
        click(&ctx, &mut app, "应用到工程");
        assert_eq!(app.history.len(), 1, "{kind}");
        let projection = app.current_schema_authoring(&target).unwrap();
        assert_eq!(projection.binding.as_ref().unwrap().schema_id, schema);
        assert_eq!(
            projection
                .fields
                .iter()
                .find(|item| item.id == field)
                .unwrap()
                .value,
            Some(value)
        );
        app.schema_authoring.close(&ctx);
        app.open_schema_authoring(&ctx, target.clone());
        assert!(!app.schema_authoring.sessions[&target].dirty());
        app.project.save().unwrap();
        let reopened = worldline_core::project::Project::open(&app.project.root).unwrap();
        let saved = reopened.schema_authoring(&target).unwrap();
        assert_eq!(saved.binding.unwrap().schema_id, schema);
        assert_eq!(
            saved
                .fields
                .iter()
                .find(|item| item.id == field)
                .unwrap()
                .value,
            app.schema_authoring.sessions[&target].values[field]
        );
    }
}

#[test]
fn overlapping_template_keeps_prompt_without_second_value_editor_or_default() {
    use worldline_core::project_templates::{
        ProjectTemplate, ProjectTemplateDocument, ProjectTemplateField, ProjectTemplateIndex,
    };
    let ctx = egui::Context::default();
    let catalog = worldline_core::catalog::Catalog::default();
    let template = ProjectTemplate {
        id: "project:overlap".into(),
        title: "模板提示".into(),
        applies_to: TargetRef::new("entity", ""),
        applies_to_entity_type: Some("place".into()),
        fields: vec![ProjectTemplateField {
            id: "population_hint".into(),
            key: Some("population".into()),
            label: "人口的创作说明".into(),
            field_type: "text".into(),
            required: true,
            choices: vec![],
            target: None,
            target_entity_type: None,
            default: Some("不应自动使用".into()),
            fields: vec![],
        }],
    };
    let index = ProjectTemplateIndex {
        builtins: vec![],
        diagnostics: vec![],
        projects: [(
            "project:overlap".into(),
            ProjectTemplateDocument {
                file: ".world/templates/overlap.json".into(),
                template: Some(template),
                source_document: None,
                source_bytes: vec![],
                diagnostics: vec![],
                read_only: false,
            },
        )]
        .into_iter()
        .collect(),
    };
    let mut values = vec![
        ("population".into(), PropertyValue::Num(0.0)),
        ("unknown".into(), PropertyValue::Str("保留".into())),
    ];
    let original = values.clone();
    let output = ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            crate::app::templates::template_panel_excluding(
                ui,
                "entity",
                Some("place"),
                &index,
                &catalog,
                &mut values,
                &["population".into()],
            );
        });
    });
    assert_eq!(values, original);
    assert!(output.shapes.iter().any(|shape| find(
        &shape.shape,
        "人口的创作说明 · population · 使用约束字段的同一值入口"
    )
    .is_some()));
    assert!(!output.shapes.iter().any(|shape| find(
        &shape.shape,
        "使用模板默认值 · * 人口的创作说明 · population"
    )
    .is_some()));
}
