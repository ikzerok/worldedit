use super::*;

#[test]
fn visual_template_new_fields_trial_preview_apply_and_single_undo() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    let sources = app.project.sources().clone();
    click(&ctx, &mut app, 19, "新建空白模板");
    assert!(
        app.template_manager.has_unsubmitted_work(),
        "新建未输入也须保护生成草稿"
    );
    for label in [
        "＋ 文本",
        "＋ 数字",
        "＋ 布尔",
        "＋ 枚举",
        "＋ 对象引用",
        "＋ 分组",
    ] {
        click(&ctx, &mut app, 19, label);
    }
    let projection = app.template_manager.projection.as_ref().unwrap();
    assert!(projection.editable, "{:?}", projection.diagnostics);
    let template = projection.template.as_ref().unwrap();
    assert_eq!(template.fields.len(), 6);
    assert_eq!(template.fields[5].fields.len(), 1);
    click(&ctx, &mut app, 19, "试填预览");
    click(&ctx, &mut app, 19, "＋ 新字段 · property_1");
    assert_eq!(app.template_manager.trial_values.len(), 1);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.sources(), sources);
    click(&ctx, &mut app, 19, "预览导入 / 替换");
    assert!(app.template_manager.preview.is_some(), "{:?}", app.io_error);
    click(&ctx, &mut app, 19, "应用预览中的模板变更");
    assert!(app
        .project
        .template_index()
        .projects
        .contains_key("project:new_template_1"));
    assert!(
        !app.template_manager.has_unsubmitted_work(),
        "纯试填不能阻止离开"
    );
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.project.sources(), sources);
    app.undo(false);
    assert!(!app
        .project
        .template_index()
        .projects
        .contains_key("project:new_template_1"));
    let _ = frame(&ctx, &mut app, vec![], 19);
    assert!(
        !app.template_manager.has_unsubmitted_work(),
        "撤销后清洁选中模板跟随工程，不产生幽灵草稿"
    );
}

#[test]
fn visual_template_replacement_navigation_cancel_and_invalid_json_preserve_input() {
    let (ctx, mut app) = app();
    click(&ctx, &mut app, 19, "新建空白模板");
    let generated = app.template_manager.editor.clone();
    click(&ctx, &mut app, 19, "新建空白模板");
    assert!(app.template_manager.pending.is_some());
    click(&ctx, &mut app, 19, "继续编辑当前模板");
    assert_eq!(app.template_manager.editor, generated);
    click(&ctx, &mut app, 19, "高级 JSON");
    replace_text_area(&ctx, &mut app, 19, "new_template_1", "{ broken 原文");
    click(&ctx, &mut app, 19, "可视字段");
    assert_eq!(app.template_manager.editor, "{ broken 原文");
    click(&ctx, &mut app, 19, "地理与地点 · template_place");
    assert!(app.template_manager.pending.is_some());
    click(&ctx, &mut app, 19, "继续编辑当前模板");
    assert_eq!(app.template_manager.editor, "{ broken 原文");
    click(&ctx, &mut app, 19, "地理与地点 · template_place");
    click(&ctx, &mut app, 19, "丢弃模板输入并继续");
    assert!(!app.template_manager.has_unsubmitted_work());
    assert!(app.template_manager.editor.is_empty());
}

#[test]
fn visual_template_invalid_property_input_survives_switch_and_failed_update() {
    let (ctx, mut app) = app();
    click(&ctx, &mut app, 19, "新建空白模板");
    click(&ctx, &mut app, 19, "＋ 数字");
    click(&ctx, &mut app, 19, "新字段 · 数字");
    // 属性控件缓冲仍属于真实 manager；断言失败候选不替换原稿。
    app.template_manager.properties.as_mut().unwrap().field.key = Some("bad key".into());
    let raw = app.template_manager.editor.clone();
    click(&ctx, &mut app, 19, "更新字段草稿");
    assert_eq!(app.template_manager.editor, raw);
    assert_eq!(
        app.template_manager
            .properties
            .as_ref()
            .unwrap()
            .field
            .key
            .as_deref(),
        Some("bad key")
    );
    click(&ctx, &mut app, 19, "高级 JSON");
    assert!(app.template_manager.has_unsubmitted_work());
    assert!(app
        .template_manager
        .error
        .as_deref()
        .unwrap()
        .contains("属性输入"));
    click(&ctx, &mut app, 19, "还原字段输入");
    click(&ctx, &mut app, 19, "高级 JSON");
    assert_eq!(app.template_manager.editor, raw);
}

#[test]
fn visual_template_external_definition_change_preserves_dirty_input_and_blocks_preview() {
    let (ctx, mut app) = app();
    let original = r#"{"schema_version":1,"id":"project:typed","title":"Typed fields","applies_to":{"kind":"entity","entity_type":"place"},"fields":[]}"#;
    register_project_template(&mut app, original);
    click(&ctx, &mut app, 19, "Typed fields · project:typed");
    click(&ctx, &mut app, 19, "＋ 文本");
    let draft = app.template_manager.editor.clone();
    let path = app.project.root.join(".world/templates/typed.json");
    app.project
        .set_authoring_document(
            &path,
            original.replace("Typed fields", "外部更新").into_bytes(),
        )
        .unwrap();
    app.recompile();
    let output = frame(&ctx, &mut app, vec![], 19);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("原模板已变化"), "{rendered}");
    assert_eq!(app.template_manager.editor, draft);
    assert!(app.template_manager.preview.is_none());
}

#[test]
fn visual_template_creates_two_real_records_and_revision_never_migrates_values() {
    let (ctx, mut app) = app();
    click(&ctx, &mut app, 19, "新建空白模板");
    click(&ctx, &mut app, 19, "＋ 文本");
    click(&ctx, &mut app, 19, "新字段 · 文本");
    {
        let input = app.template_manager.properties.as_mut().unwrap();
        input.field.label = "港口记录".into();
        input.field.key = Some("harbor_note".into());
        input.has_default = true;
        input.default_text = "只是建议，未选择不写入".into();
    }
    click(&ctx, &mut app, 19, "更新字段草稿");
    click(&ctx, &mut app, 19, "预览导入 / 替换");
    click(&ctx, &mut app, 19, "应用预览中的模板变更");
    let mut ids = Vec::new();
    for (name, note) in [("北港", "北岸实际记录"), ("南港", "南岸实际记录")] {
        app.edit_entity(None);
        let form = app.entity_editor.as_mut().unwrap();
        form.draft.display = name.into();
        form.draft.entity_type = "place".into();
        ids.push(form.draft.id.clone());
        let text = rendered_text_in_window(&ctx, &mut app, 0, "工程模板 · 新工程模板");
        assert!(text.contains("工程模板 · 新工程模板"), "{text}");
        assert!(app
            .entity_editor
            .as_ref()
            .unwrap()
            .draft
            .properties
            .is_empty());
        scroll_from_visible_anchor_to(
            &ctx,
            &mut app,
            0,
            "工程模板 · 新工程模板",
            "使用模板默认值 · 港口记录 · harbor_note",
        );
        click(&ctx, &mut app, 0, "使用模板默认值 · 港口记录 · harbor_note");
        replace_text_area(&ctx, &mut app, 0, "只是建议，未选择不写入", note);
        click(&ctx, &mut app, 0, "应用资料");
        assert!(app.entity_editor.is_none(), "{:?}", app.io_error);
    }
    assert_ne!(ids[0], ids[1]);
    click(&ctx, &mut app, 19, "新工程模板 · project:new_template_1");
    click(&ctx, &mut app, 19, "港口记录 · 文本");
    app.template_manager.properties.as_mut().unwrap().field.key = Some("renamed_note".into());
    click(&ctx, &mut app, 19, "更新字段草稿");
    click(&ctx, &mut app, 19, "预览导入 / 替换");
    assert!(app
        .template_manager
        .preview
        .as_ref()
        .unwrap()
        .field_changes
        .iter()
        .any(|change| change.old_key.as_deref() == Some("harbor_note")));
    click(&ctx, &mut app, 19, "应用预览中的模板变更");
    for (id, note) in ids.iter().zip(["北岸实际记录", "南岸实际记录"]) {
        let entity = &app
            .snapshot
            .as_ref()
            .unwrap()
            .result
            .analysis
            .catalog
            .entities[id];
        assert_eq!(
            entity.properties["harbor_note"],
            worldline_core::ast::PropertyValue::Str(note.into())
        );
        assert!(!entity.properties.contains_key("renamed_note"));
    }
    app.project.save().unwrap();
    let mut reopened = Project::open(&app.project.root).unwrap();
    assert_eq!(
        reopened.template_index().projects["project:new_template_1"]
            .template
            .as_ref()
            .unwrap()
            .fields[0]
            .key
            .as_deref(),
        Some("renamed_note")
    );
    for id in &ids {
        assert!(reopened.compile().analysis.catalog.entities[id]
            .properties
            .contains_key("harbor_note"));
    }
}

#[test]
fn template_invalid_original_repair_requires_distinct_preview_confirmation_and_undo_restores_bytes()
{
    let (ctx, mut app) = app();
    let broken = r#"{"schema_version":1,"id":"project:broken","title":"原文","title":"重复标题","applies_to":{"kind":"entity"},"fields":[]}"#;
    register_project_template(&mut app, broken);
    let path = app.project.root.join(".world/templates/broken.json");
    click(&ctx, &mut app, 19, "无法解析的模板 · project:broken · 只读");
    click(&ctx, &mut app, 19, "高级 JSON");
    let repaired = r#"{"schema_version":1,"id":"project:broken","title":"修复标题","applies_to":{"kind":"entity"},"fields":[]}"#;
    replace_text_area(&ctx, &mut app, 19, "重复标题", repaired);
    click(&ctx, &mut app, 19, "预览导入 / 替换");
    assert!(
        app.template_manager.preview.is_none(),
        "普通Replace仍拒绝坏原文"
    );
    assert_eq!(
        app.project.authoring_document(&path).unwrap().bytes(),
        broken.as_bytes()
    );
    click(&ctx, &mut app, 19, "修复坏原文（完整替换）");
    let preview = app.template_manager.preview.as_ref().unwrap();
    assert!(preview
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "TPL007"));
    click(&ctx, &mut app, 19, "取消预览");
    assert_eq!(app.template_manager.editor, repaired);
    assert_eq!(
        app.project.authoring_document(&path).unwrap().bytes(),
        broken.as_bytes()
    );
    click(&ctx, &mut app, 19, "修复坏原文（完整替换）");
    click(&ctx, &mut app, 19, "确认完整替换坏原文");
    assert_eq!(app.history.len(), 1);
    assert!(app.project.template_index().projects["project:broken"]
        .template
        .is_some());
    app.undo(false);
    assert_eq!(
        app.project.authoring_document(&path).unwrap().bytes(),
        broken.as_bytes()
    );
}

#[test]
fn first_template_import_and_continued_edit_keep_deleted_field_identity_reserved() {
    let (ctx, mut app) = app();
    click(&ctx, &mut app, 19, "新建空白模板");
    click(&ctx, &mut app, 19, "＋ 文本");
    click(&ctx, &mut app, 19, "新字段 · 文本");
    click(&ctx, &mut app, 19, "删除字段…");
    click(&ctx, &mut app, 19, "确认删除字段定义");
    click(&ctx, &mut app, 19, "预览导入 / 替换");
    click(&ctx, &mut app, 19, "应用预览中的模板变更");
    assert!(app
        .template_manager
        .reserved_field_ids
        .iter()
        .any(|id| id == "field_1"));
    click(&ctx, &mut app, 19, "＋ 文本");
    let template = app
        .template_manager
        .projection
        .as_ref()
        .unwrap()
        .template
        .as_ref()
        .unwrap();
    assert_ne!(template.fields[0].id, "field_1");
    assert_ne!(template.fields[0].key.as_deref(), Some("property_1"));
}

#[test]
fn template_preview_of_missing_source_is_explicitly_incomplete_and_cannot_apply() {
    let (ctx, mut app) = app();
    let broken = format!(
        "include \"missing.wl\"\n{}",
        app.project.document(&app.active_file).unwrap()
    );
    app.project
        .set_text(&app.active_file.clone(), broken)
        .unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 19, "新建空白模板");
    click(&ctx, &mut app, 19, "预览导入 / 替换");
    assert!(!app.template_manager.preview.as_ref().unwrap().complete);
    let rendered = rendered_text_in_window(&ctx, &mut app, 19, "影响检查不完整");
    assert!(rendered.contains("不能应用"), "{rendered}");
    click(&ctx, &mut app, 19, "应用预览中的模板变更");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.project.template_index().projects.is_empty());
    assert!(app.template_manager.has_unsubmitted_work());
}

#[test]
fn template_impact_pages_keep_full_report_and_new_preview_resets_to_first_page() {
    let (ctx, mut app) = app();
    let mut source = String::new();
    for index in 0..103 {
        source.push_str(&format!(
            "entity place_{index:03} kind place as \"地点{index}\"\n"
        ));
    }
    app.project
        .set_text(&app.active_file.clone(), source)
        .unwrap();
    app.recompile();
    click(&ctx, &mut app, 19, "新建空白模板");
    click(&ctx, &mut app, 19, "＋ 文本");
    click(&ctx, &mut app, 19, "预览导入 / 替换");
    assert_eq!(
        app.template_manager
            .preview
            .as_ref()
            .unwrap()
            .instances
            .len(),
        103
    );
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 19, "下一页");
    assert_eq!(app.template_manager.impact_page, 1);
    click(&ctx, &mut app, 19, "末页");
    assert_eq!(app.template_manager.impact_page, 2);
    let output = rendered_text_in_window(&ctx, &mut app, 19, "显示 101–103 / 103");
    assert!(output.contains("显示 101–103 / 103"), "{output}");
    assert_eq!(
        app.template_manager
            .preview
            .as_ref()
            .unwrap()
            .instances
            .len(),
        103
    );
    assert_eq!(app.project.content_baseline(), baseline);
    click(&ctx, &mut app, 19, "预览导入 / 替换");
    assert_eq!(app.template_manager.impact_page, 0);
    assert_eq!(
        app.template_manager
            .preview
            .as_ref()
            .unwrap()
            .instances
            .len(),
        103
    );
}

#[test]
fn template_impact_exact_property_snapshots_remain_visible_when_instance_page_changes() {
    let (ctx, mut app) = app();
    let mut source = app.project.document(&app.active_file).unwrap().to_owned();
    for index in 0..102 {
        source.push_str(&format!(
            "entity extra_{index:03} kind place as \"地点{index}\"\n"
        ));
    }
    app.project
        .set_text(&app.active_file.clone(), source)
        .unwrap();
    let original = serde_json::json!({"schema_version":1,"id":"project:precise","title":"Precise","applies_to":{"kind":"entity","entity_type":"place"},"fields":[
        {"id":"memo","key":"memo","label":"旧标题","type":"text","required":false,"default":"旧提示"},
        {"id":"stage","key":"stage","label":"阶段","type":"enum","required":false,"choices":["草稿","待校"],"default":"草稿"},
        {"id":"home","key":"home","label":"所属","type":"object_ref","required":false,"target":{"kind":"entity","entity_type":"place"},"default":{"kind":"entity","id":"a"}}
    ]});
    register_project_template(&mut app, &original.to_string());
    click(&ctx, &mut app, 19, "Precise · project:precise");
    click(&ctx, &mut app, 19, "高级 JSON");
    let mut replacement = original;
    replacement["fields"][0]["label"] = "新标题".into();
    replacement["fields"][0]["required"] = true.into();
    replacement["fields"][0]["default"] = "新提示".into();
    replacement["fields"][1]["choices"] = serde_json::json!(["草稿", "定稿"]);
    replacement["fields"][1]["default"] = "定稿".into();
    replacement["fields"][2]["target"]["entity_type"] = "organization".into();
    replacement["fields"][2]["default"]["id"] = "b".into();
    replace_text_area(&ctx, &mut app, 19, "旧标题", &replacement.to_string());
    click(&ctx, &mut app, 19, "预览导入 / 替换");
    let preview = app.template_manager.preview.as_ref().unwrap();
    assert!(preview.complete);
    assert_eq!(preview.instances.len(), 103);
    let before = app.project.content_baseline();
    for next in [false, true] {
        if next {
            click(&ctx, &mut app, 19, "下一页");
        }
        let output = frame(&ctx, &mut app, vec![], 19);
        let mut text = String::new();
        for shape in &output.shapes {
            collect_text(&shape.shape, &mut text);
        }
        for exact in [
            "显示名称：\"旧标题\" → \"新标题\"",
            "必填提示：关闭 → 开启",
            "默认提示：\"旧提示\" → \"新提示\"",
            "枚举选项：[\"草稿\"、\"待校\"] → [\"草稿\"、\"定稿\"]",
            "引用限制：entity / \"place\" → entity / \"organization\"",
        ] {
            assert!(text.contains(exact), "遗漏快照 {exact}: {text}");
        }
    }
    assert_eq!(app.template_manager.impact_page, 1);
    assert_eq!(app.project.content_baseline(), before);
}
