use super::*;

#[test]
fn project_template_manager_browses_builtin_and_project_templates_without_writing() {
    let (ctx, mut app) = app();
    register_project_template(
        &mut app,
        r#"{"schema_version":1,"id":"project:typed","title":"Typed fields","applies_to":{"kind":"entity","entity_type":"place"},"fields":[{"id":"memo","key":"memo","label":"Memo","type":"text","required":false}]}"#,
    );
    let baseline = app.project.content_baseline();
    let sources = app.project.sources().clone();
    let was_dirty = app.project.is_dirty();
    click(&ctx, &mut app, 18, "工程模板");
    assert_eq!(app.tab, super::Tab::Templates);

    let output = frame(&ctx, &mut app, Vec::new(), 17);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }

    assert!(rendered.contains("模板管理"), "{rendered}");
    assert!(rendered.contains("内置模板"), "{rendered}");
    assert!(rendered.contains("工程模板"), "{rendered}");
    assert!(rendered.contains("project:typed"), "{rendered}");
    assert!(rendered.contains("Typed fields"), "{rendered}");
    click(&ctx, &mut app, 17, "地理与地点 · template_place");
    let builtin_details = rendered_text_in_window(&ctx, &mut app, 17, "来源：内置");
    assert!(
        builtin_details.contains("来源：内置 · schema v1"),
        "{builtin_details}"
    );
    click(&ctx, &mut app, 17, "Typed fields · project:typed");
    let project_details = rendered_text_in_window(&ctx, &mut app, 17, "来源：工程");
    assert!(
        project_details.contains("来源：工程 · schema v1"),
        "{project_details}"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.sources(), sources);
    assert_eq!(app.project.is_dirty(), was_dirty);
}

#[test]
fn project_template_replacement_requires_preview_and_confirm_and_keeps_instances() {
    let (ctx, mut app) = app();
    let original_source = concat!(
        "entity a kind place as \"同名\"\n",
        "  property memo = \"保留的实例文字\"\n",
        "entity b kind place as \"同名\"\n"
    );
    app.project
        .set_text(&app.active_file.clone(), original_source.into())
        .unwrap();
    let original_template = r#"{"schema_version":1,"id":"project:typed","title":"Typed fields","applies_to":{"kind":"entity","entity_type":"place"},"fields":[{"id":"memo","key":"memo","label":"Memo","type":"text","required":false}]}"#;
    register_project_template(&mut app, original_template);
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 18, "工程模板");
    click(&ctx, &mut app, 17, "Typed fields · project:typed");

    let replacement = r#"{
  "schema_version": 1,
  "id": "project:typed",
  "title": "Typed fields",
  "applies_to": {"kind": "entity", "entity_type": "place"},
  "fields": [
    {"id": "memo", "key": "memo", "label": "Memo", "type": "number", "required": false}
  ]
}"#;
    replace_text_area(&ctx, &mut app, 17, "\"type\":\"text\"", replacement);
    click(&ctx, &mut app, 17, "预览导入 / 替换");
    let preview_text = rendered_text_in_window(&ctx, &mut app, 17, "类型变化");
    assert!(preview_text.contains("实例影响"), "{preview_text}");
    assert!(preview_text.contains("TypeMismatch"), "{preview_text}");
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(
        app.project.template_index().projects["project:typed"]
            .template
            .as_ref()
            .unwrap()
            .fields[0]
            .field_type,
        "text"
    );

    click(&ctx, &mut app, 17, "取消预览");
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(
        app.project.document(&app.active_file).unwrap(),
        original_source
    );

    click(&ctx, &mut app, 17, "预览导入 / 替换");
    click(&ctx, &mut app, 17, "应用预览中的模板变更");
    assert_eq!(
        app.project.template_index().projects["project:typed"]
            .template
            .as_ref()
            .unwrap()
            .fields[0]
            .field_type,
        "number"
    );
    assert!(app
        .project
        .document(&app.active_file)
        .unwrap()
        .contains("保留的实例文字"));
    assert_eq!(app.history.len(), 1);
    app.undo(false);
    assert_eq!(
        app.project.template_index().projects["project:typed"]
            .template
            .as_ref()
            .unwrap()
            .fields[0]
            .field_type,
        "text"
    );
}

#[test]
fn project_template_fields_render_by_type_and_leave_unowned_values_intact() {
    let (ctx, mut app) = app();
    let source = concat!(
        "entity a kind place as \"同名\"\n",
        "  property weight = 1.5\n",
        "  property available = false\n",
        "  property status = \"draft\"\n",
        "  property home = ref(\"entity\", \"b\")\n",
        "  property legacy_weight = \"old-format\"\n",
        "  property extension_value = \"keep me\"\n",
        "entity b kind place as \"同名\"\n",
        "entity c kind place as \"同名\"\n"
    );
    app.project
        .set_text(&app.active_file.clone(), source.into())
        .unwrap();
    register_project_template(
        &mut app,
        r#"{"schema_version":1,"id":"project:typed","title":"Typed fields","applies_to":{"kind":"entity","entity_type":"place"},"fields":[{"id":"memo","key":"memo","label":"Memo","type":"text","required":false,"default":"suggested only"},{"id":"weight","key":"weight","label":"Weight","type":"number","required":false},{"id":"available","key":"available","label":"Available","type":"boolean","required":false},{"id":"status","key":"status","label":"Status","type":"enum","required":false,"choices":["draft","ready"]},{"id":"home","key":"home","label":"Home","type":"object_ref","required":false,"target":{"kind":"entity","entity_type":"place"}},{"id":"legacy_weight","key":"legacy_weight","label":"Legacy weight","type":"number","required":false},{"id":"group","label":"Details","type":"group","fields":[{"id":"nested","key":"nested_note","label":"Nested note","type":"text","required":false}]}]}"#,
    );
    let template_doc = app.project.template_index().projects["project:typed"].clone();
    assert_eq!(
        template_doc.template.as_ref().unwrap().fields.len(),
        7,
        "{:?}",
        template_doc.diagnostics
    );
    app.edit_entity(Some("a"));
    assert!(app.entity_editor.is_some(), "{:?}", app.io_error);

    let rendered = rendered_text_in_window(&ctx, &mut app, 0, "工程模板");

    assert!(rendered.contains("工程模板 · Typed fields"), "{rendered}");
    assert!(rendered.contains("Weight · weight"), "{rendered}");
    assert!(rendered.contains("Available · available"), "{rendered}");
    assert!(rendered.contains("Status · status"), "{rendered}");
    assert!(rendered.contains("Home · home"), "{rendered}");
    drag_numeric_value(&ctx, &mut app, 0, "1.5", 10.0);
    click(&ctx, &mut app, 0, "draft");
    click(&ctx, &mut app, 0, "ready");
    click(&ctx, &mut app, 0, "Available · available");
    click(&ctx, &mut app, 0, "同名 · entity:b");
    click(&ctx, &mut app, 0, "同名 · entity:c");
    let scrolled = scroll_window(&ctx, &mut app, 0, "Home · home", -12.0);
    assert!(scrolled.contains("old-format"), "{scrolled}");
    assert!(scrolled.contains("Details"), "{scrolled}");
    click(&ctx, &mut app, 0, "应用资料");

    let entity = &app.project.compile().analysis.catalog.entities["a"];
    assert_eq!(
        entity.properties["weight"],
        worldline_core::ast::PropertyValue::Num(2.5)
    );
    assert_eq!(
        entity.properties["available"],
        worldline_core::ast::PropertyValue::Bool(true)
    );
    assert_eq!(
        entity.properties["status"],
        worldline_core::ast::PropertyValue::Str("ready".into())
    );
    assert_eq!(
        entity.properties["home"],
        worldline_core::ast::PropertyValue::Ref(TargetRef::new("entity", "c"))
    );
    assert_eq!(
        entity.properties["legacy_weight"],
        worldline_core::ast::PropertyValue::Str("old-format".into())
    );
    assert_eq!(
        entity.properties["extension_value"],
        worldline_core::ast::PropertyValue::Str("keep me".into())
    );
    assert!(!entity.properties.contains_key("memo"));
    assert!(!entity.properties.contains_key("nested_note"));
}

#[test]
fn read_only_project_template_shows_reason_and_preserves_instance_values() {
    let (ctx, mut app) = app();
    let root = app.project.root.clone();
    let source = "entity a kind place as \"同名\"\n  property memo = \"原始字段\"\n";
    let template = r#"{"schema_version":1,"id":"project:future","title":"Future template","applies_to":{"kind":"entity","entity_type":"place"},"required_features":["vendor.future.v2"],"fields":[{"id":"memo","key":"memo","label":"Memo","type":"text","required":false}]}"#;
    std::fs::create_dir_all(root.join(".world/templates")).unwrap();
    std::fs::write(root.join("world.wl"), source).unwrap();
    std::fs::write(
        root.join(".world/project.json"),
        br#"{"schema_version":1,"language_version":"1.10","entry":"world.wl","required_features":["content.entities.v1","content.templates.v1"],"maps":{},"graph_views":{},"templates":{"project:future":".world/templates/future.json"}}"#,
    )
    .unwrap();
    std::fs::write(root.join(".world/templates/future.json"), template).unwrap();
    app.project = Project::open(&root).unwrap();
    app.active_file = app.project.entry.clone();
    app.reset_views();
    app.recompile();
    let template_path = app.project.root.join(".world/templates/future.json");
    let template_bytes = app
        .project
        .authoring_document(&template_path)
        .unwrap()
        .bytes()
        .to_vec();
    assert!(app.project.template_index().projects["project:future"].read_only);

    app.edit_entity(Some("a"));
    let rendered = rendered_text_in_window(&ctx, &mut app, 0, "Future template");
    assert!(
        rendered.contains("模板格式或必需能力不受支持"),
        "{rendered}"
    );
    assert!(rendered.contains("TPL002"), "{rendered}");
    assert!(!rendered.contains("＋ Memo"), "{rendered}");
    click(&ctx, &mut app, 0, "应用资料");

    assert_eq!(
        app.project
            .authoring_document(&template_path)
            .unwrap()
            .bytes(),
        template_bytes
    );
    assert_eq!(
        app.project.compile().analysis.catalog.entities["a"].properties["memo"],
        worldline_core::ast::PropertyValue::Str("原始字段".into())
    );
}

#[test]
fn stale_project_template_preview_is_rejected_and_keeps_the_json_draft() {
    let (ctx, mut app) = app();
    let original_template = r#"{"schema_version":1,"id":"project:typed","title":"Typed fields","applies_to":{"kind":"entity","entity_type":"place"},"fields":[{"id":"memo","key":"memo","label":"Memo","type":"text","required":false}]}"#;
    register_project_template(&mut app, original_template);
    click(&ctx, &mut app, 18, "工程模板");
    click(&ctx, &mut app, 17, "Typed fields · project:typed");
    let replacement = r#"{"schema_version":1,"id":"project:typed","title":"Updated draft","applies_to":{"kind":"entity","entity_type":"place"},"fields":[{"id":"memo","key":"memo","label":"Memo","type":"text","required":false}]}"#;
    replace_text_area(
        &ctx,
        &mut app,
        17,
        "\"title\":\"Typed fields\"",
        replacement,
    );
    click(&ctx, &mut app, 17, "预览导入 / 替换");
    assert_eq!(
        app.project.template_index().projects["project:typed"]
            .template
            .as_ref()
            .unwrap()
            .title,
        "Typed fields"
    );

    let external_source = "entity a kind place as \"外部版本\"\n";
    app.project
        .set_text(&app.active_file.clone(), external_source.into())
        .unwrap();
    app.recompile();
    let external_baseline = app.project.content_baseline();
    click(&ctx, &mut app, 17, "应用预览中的模板变更");

    assert!(app
        .io_error
        .as_deref()
        .is_some_and(|error| error.contains("StaleRevision")));
    assert_eq!(app.project.content_baseline(), external_baseline);
    assert_eq!(
        app.project.document(&app.active_file).unwrap(),
        external_source
    );
    assert_eq!(
        app.project.template_index().projects["project:typed"]
            .template
            .as_ref()
            .unwrap()
            .title,
        "Typed fields"
    );
    assert!(app.history.is_empty());
    let rendered = rendered_text_in_window(&ctx, &mut app, 17, "Updated draft");
    assert!(rendered.contains("Updated draft"), "{rendered}");
}

#[test]
fn stale_project_template_form_cannot_overwrite_external_instance_edits() {
    let (ctx, mut app) = app();
    app.project
        .set_text(
            &app.active_file.clone(),
            "entity a kind place as \"同名\"\n  property weight = 1.5\n".into(),
        )
        .unwrap();
    register_project_template(
        &mut app,
        r#"{"schema_version":1,"id":"project:typed","title":"Typed fields","applies_to":{"kind":"entity","entity_type":"place"},"fields":[{"id":"weight","key":"weight","label":"Weight","type":"number","required":false}]}"#,
    );
    app.edit_entity(Some("a"));
    let rendered = rendered_text_in_window(&ctx, &mut app, 0, "Weight · weight");
    assert!(rendered.contains("Weight · weight"), "{rendered}");
    drag_numeric_value(&ctx, &mut app, 0, "1.5", 10.0);
    let draft_value = app.entity_editor.as_ref().unwrap().draft.properties[0]
        .1
        .clone();
    assert_eq!(draft_value, worldline_core::ast::PropertyValue::Num(2.5));

    let external_source =
        "entity a kind place as \"外部版本\"\n  property weight = 9.0\n  property external = \"保留\"\n";
    app.project
        .set_text(&app.active_file.clone(), external_source.into())
        .unwrap();
    app.recompile();
    let external_baseline = app.project.content_baseline();

    click(&ctx, &mut app, 0, "应用资料");

    assert_eq!(
        app.project.document(&app.active_file).unwrap(),
        external_source
    );
    assert_eq!(app.project.content_baseline(), external_baseline);
    assert!(app.history.is_empty());
    let rendered = rendered_text_in_window(&ctx, &mut app, 0, "工程已变化");
    assert!(rendered.contains("工程已变化"), "{rendered}");
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.properties[0].1,
        draft_value
    );
}

#[test]
fn copying_importing_exporting_and_deactivating_templates_keeps_instance_content() {
    let (ctx, mut app) = app();
    let source = "entity a kind place as \"同名\"\n  property extension_value = \"keep me\"\n";
    app.project
        .set_text(&app.active_file.clone(), source.into())
        .unwrap();
    app.recompile();
    let original_baseline = app.project.content_baseline();

    click(&ctx, &mut app, 18, "工程模板");
    click(&ctx, &mut app, 19, "地理与地点 · template_place");
    click(&ctx, &mut app, 19, "复制为新模板 JSON");
    click(&ctx, &mut app, 19, "预览导入 / 替换");
    let import_preview = rendered_text_in_window(&ctx, &mut app, 19, "导入影响预览");
    assert!(
        import_preview.contains("template_copy_1"),
        "{import_preview}"
    );
    let import_details = rendered_text_in_window(&ctx, &mut app, 19, "实例影响");
    assert!(
        import_details.contains("应用预览中的模板变更"),
        "{import_details}"
    );
    assert!(!app
        .project
        .template_index()
        .projects
        .contains_key("project:template_copy_1"));
    assert_eq!(app.project.content_baseline(), original_baseline);
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);

    click(&ctx, &mut app, 19, "应用预览中的模板变更");
    assert!(app
        .project
        .template_index()
        .projects
        .contains_key("project:template_copy_1"));
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    assert_ne!(app.project.content_baseline(), original_baseline);
    #[cfg(not(target_arch = "wasm32"))]
    {
        app.project.save().unwrap();
        app.project = Project::open(&app.project.root).unwrap();
        app.active_file = app.project.entry.clone();
        app.recompile();
        assert!(app
            .project
            .template_index()
            .projects
            .contains_key("project:template_copy_1"));
        assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    }

    click(
        &ctx,
        &mut app,
        19,
        "地理与地点 副本 · project:template_copy_1",
    );
    click(&ctx, &mut app, 19, "导出 JSON 到剪贴板");
    let exported = rendered_text_in_window(&ctx, &mut app, 19, "已复制到剪贴板");
    assert!(exported.contains("模板 JSON 已复制到剪贴板"), "{exported}");

    let baseline_before_deactivation = app.project.content_baseline();
    click(&ctx, &mut app, 19, "预览停用模板（保留对象资料）");
    let deactivate_header = rendered_text_in_window(&ctx, &mut app, 19, "停用影响预览");
    assert!(
        deactivate_header.contains("停用影响预览"),
        "{deactivate_header}"
    );
    let deactivate_preview = rendered_text_in_window(&ctx, &mut app, 19, "entity:a");
    assert!(
        deactivate_preview.contains("entity:a"),
        "{deactivate_preview}"
    );
    assert!(app
        .project
        .template_index()
        .projects
        .contains_key("project:template_copy_1"));
    assert_eq!(app.project.content_baseline(), baseline_before_deactivation);
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);

    click(&ctx, &mut app, 19, "取消预览");
    assert!(app
        .project
        .template_index()
        .projects
        .contains_key("project:template_copy_1"));
    click(&ctx, &mut app, 19, "预览停用模板（保留对象资料）");
    let second_deactivate_details = rendered_text_in_window(&ctx, &mut app, 19, "entity:a");
    assert!(
        second_deactivate_details.contains("确认停用模板"),
        "{second_deactivate_details}"
    );
    click(&ctx, &mut app, 19, "确认停用模板");
    assert!(!app
        .project
        .template_index()
        .projects
        .contains_key("project:template_copy_1"));
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    assert_eq!(
        app.project.compile().analysis.catalog.entities["a"].properties["extension_value"],
        worldline_core::ast::PropertyValue::Str("keep me".into())
    );
    assert_eq!(app.history.len(), 2);
    app.undo(false);
    assert!(app
        .project
        .template_index()
        .projects
        .contains_key("project:template_copy_1"));
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
}
