use super::*;

fn select_binding_with_source(
    source: String,
    schema: &str,
) -> (egui::Context, WorldeditApp, String) {
    let (ctx, mut app) = app();
    let entry = app.active_file.clone();
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    app.open_schema_authoring(&ctx, TargetRef::new("entity", "shared"));
    let baseline = app.project.content_baseline();
    session(&mut app).selected_schema = Some(schema.into());
    (ctx, app, baseline)
}

fn assert_existing_property_source(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    baseline: &str,
    field_id: &str,
    property_line: &str,
) {
    let source = app.project.document(&app.active_file).unwrap();
    let line = source
        .lines()
        .position(|line| line.trim() == property_line)
        .unwrap() as u32
        + 1;
    click(ctx, app, "预览所选绑定");
    assert_eq!(app.project.content_baseline(), baseline);
    let form = session(app);
    let field = form
        .projection
        .fields
        .iter()
        .find(|field| field.id == field_id)
        .unwrap();
    assert!(field.present);
    let source = field
        .property_source
        .clone()
        .expect("绑定预览必须保留现有属性的可定位来源");
    assert_eq!(source.line, line);
    assert_eq!(
        source,
        form.preview
            .as_ref()
            .unwrap()
            .after
            .fields
            .iter()
            .find(|field| field.id == field_id)
            .unwrap()
            .property_source
            .clone()
            .unwrap()
    );
    // 两个夹具中的该字段均为表单第一项，真实点击其来源并返回。
    click(ctx, app, "字段身份与来源");
    click(ctx, app, "查看现值来源");
    assert_eq!(app.jump, Some((source.line, source.column)));
    assert_eq!(app.active_file, std::path::PathBuf::from(&source.file));
    assert!(app.schema_authoring.source_return);
    click(ctx, app, "返回资料表单");
    assert!(app.schema_authoring.open);
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn unbound_existing_property_keeps_value_source_after_binding_preview() {
    let (ctx, mut app, baseline) = select_binding_with_source(
        SOURCE.replace("bind entity shared to place_data\n", ""),
        "place_data",
    );
    assert!(session(&mut app).base.fields.is_empty());
    assert_existing_property_source(
        &ctx,
        &mut app,
        &baseline,
        "population_id",
        "property population = 0",
    );
}

#[test]
fn changing_schema_exposes_source_of_previously_unknown_property() {
    let source = format!(
        "schema extra_data for entity entity_type place\n  field unknown_id unknown text\n{SOURCE}"
    );
    let (ctx, mut app, baseline) = select_binding_with_source(source, "extra_data");
    assert!(session(&mut app)
        .base
        .fields
        .iter()
        .all(|field| field.key != "unknown"));
    assert_existing_property_source(
        &ctx,
        &mut app,
        &baseline,
        "unknown_id",
        "property unknown = \"不改写\"",
    );
}
