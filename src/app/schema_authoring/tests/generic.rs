use super::*;

fn generic_frame(
    ctx: &egui::Context,
    app: &WorldeditApp,
    values: &mut Vec<(String, PropertyValue)>,
) -> egui::FullOutput {
    let projection = app
        .current_schema_authoring_shared(&TargetRef::new("entity", "shared"))
        .unwrap();
    let coverage = SchemaCoverage {
        keys: projection
            .fields
            .iter()
            .map(|field| field.key.clone())
            .collect(),
        projection: Some(projection),
    };
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 1000.0))),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                crate::app::inspector::properties_excluding(
                    ui,
                    values,
                    &app.snapshot.as_ref().unwrap().result.analysis.catalog,
                    app.project.compile_options(),
                    &coverage,
                );
            });
        },
    )
}

#[test]
fn newly_typed_duplicate_schema_key_stays_visible_and_unchanged() {
    let (ctx, app) = app();
    let mut values = vec![
        ("population".into(), PropertyValue::Num(0.0)),
        (String::new(), PropertyValue::Str("保留的新增值".into())),
    ];
    generic_frame(&ctx, &app, &mut values);
    values[1].0 = "population".into();
    let expected = values.clone();
    for _ in 0..3 {
        let output = generic_frame(&ctx, &app, &mut values);
        assert_eq!(values, expected);
        assert!(output.shapes.iter().any(|shape| find(
            &shape.shape,
            "population · 在“约束与属性”中填写"
        )
        .is_some()));
        assert!(output
            .shapes
            .iter()
            .any(|shape| find(&shape.shape, "保留的新增值").is_some()));
        assert!(output.shapes.iter().any(|shape| find(
            &shape.shape,
            "此键由 schema 管理；新增或重复输入已保留，可改名或移除，不会自动合并。"
        )
        .is_some()));
    }
}

#[test]
fn new_row_for_missing_schema_field_is_not_hidden_or_defaulted() {
    let (ctx, app) = app();
    let mut values = vec![("owner".into(), PropertyValue::Str("手写非法原值".into()))];
    let expected = values.clone();
    let output = generic_frame(&ctx, &app, &mut values);
    assert_eq!(values, expected);
    assert!(!output
        .shapes
        .iter()
        .any(|shape| find(&shape.shape, "owner · 在“约束与属性”中填写").is_some()));
    assert!(output
        .shapes
        .iter()
        .any(|shape| find(&shape.shape, "手写非法原值").is_some()));
}
