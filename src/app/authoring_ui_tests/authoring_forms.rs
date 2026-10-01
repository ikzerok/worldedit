use super::*;

#[test]
fn entity_apply_is_one_real_ui_command_with_undo_and_redo() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    app.edit_entity(None);
    let form = app.entity_editor.as_mut().unwrap();
    let id = form.draft.id.clone();
    form.draft.display = "点击创建的地点".into();
    form.draft.description = "多行资料\n不是地图占位".into();
    click(&ctx, &mut app, 0, "应用资料");
    assert!(app.entity_editor.is_none(), "{:?}", app.io_error);
    assert_eq!(
        app.snapshot
            .as_ref()
            .unwrap()
            .result
            .analysis
            .catalog
            .entities[&id]
            .display,
        "点击创建的地点"
    );
    assert_eq!(app.history.len(), 1);
    app.undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.redo.len() == 1);
    app.undo(true);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities
        .contains_key(&id));
}
#[test]
fn stale_entity_apply_button_is_disabled_and_keeps_draft() {
    let (ctx, mut app) = app();
    app.edit_entity(Some("a"));
    app.entity_editor.as_mut().unwrap().draft.display = "尚未合并的输入".into();
    app.recompile();
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 0, "应用资料");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.display,
        "尚未合并的输入"
    );
}
#[test]
fn explicit_relation_and_type_apply_use_ui_buttons_and_keep_identity() {
    let (ctx, mut app) = app();
    app.edit_relation_type(None);
    let form = app.relation_type_editor.as_mut().unwrap();
    form.draft.display = "维护".into();
    let kind_id = form.draft.id.clone();
    click(&ctx, &mut app, 2, "应用关系类型");
    assert!(app.relation_type_editor.is_none(), "{:?}", app.io_error);
    app.edit_relation(None, Some(TargetRef::new("entity", "a")));
    let form = app.relation_editor.as_mut().unwrap();
    form.draft.relation_type = kind_id;
    form.draft.to = TargetRef::new("entity", "b");
    form.draft.source_note = Some("显式作者来源".into());
    let relation_id = form.draft.id.clone();
    click(&ctx, &mut app, 1, "应用独立关系");
    assert!(app.relation_editor.is_none(), "{:?}", app.io_error);
    let relation = &app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .relations[&relation_id];
    assert_eq!(relation.from_ref, TargetRef::new("entity", "a"));
    assert_eq!(relation.to_ref, TargetRef::new("entity", "b"));
    assert_eq!(relation.source_note.as_deref(), Some("显式作者来源"));
    assert_eq!(app.history.len(), 2);
}
#[test]
fn deletion_checkbox_is_required_by_the_actual_button() {
    let (ctx, mut app) = app();
    app.plan_content_deletion(TargetRef::new("entity", "a"));
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 3, "确认删除内容");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    click(&ctx, &mut app, 3, "我确认删除此内容，而不是仅隐藏显示");
    click(&ctx, &mut app, 3, "确认删除内容");
    assert!(app.delete_form.is_none(), "{:?}", app.io_error);
    assert!(!app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities
        .contains_key("a"));
    app.undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn rename_preview_and_apply_are_two_explicit_ui_steps_with_undo() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    app.plan_target_rename(TargetRef::new("entity", "a"));
    app.rename_form.as_mut().unwrap().new_id = "alpha".into();

    click(&ctx, &mut app, 5, "预览重命名");
    assert!(app.rename_form.as_ref().unwrap().plan.is_some());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());

    click(&ctx, &mut app, 5, "应用跨视图重命名");
    assert!(app.rename_form.is_none(), "{:?}", app.io_error);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities
        .contains_key("alpha"));
    assert!(!app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities
        .contains_key("a"));
    assert_eq!(app.history.len(), 1);
    app.undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn rename_preview_shows_real_context_and_keeps_plain_text() {
    let (ctx, mut app) = app();
    app.project
        .set_text(
            &app.active_file.clone(),
            concat!(
                "entity a kind place as \"entity a 中文\" // entity a 不改\n",
                "alias entity a as \"entity a\"\n",
                "entity b kind organization as \"同名\"\n",
            )
            .into(),
        )
        .unwrap();
    app.recompile();
    app.plan_target_rename(TargetRef::new("entity", "a"));
    app.rename_form.as_mut().unwrap().new_id = "alpha".into();
    click(&ctx, &mut app, 5, "预览重命名");
    let plan = app
        .rename_form
        .as_ref()
        .unwrap()
        .plan
        .as_ref()
        .unwrap()
        .clone();
    let mut rendered = rendered_text_in_window(&ctx, &mut app, 5, "修改前");
    assert!(rendered.contains("world.wl"), "{rendered}");
    assert!(rendered.contains("运行指纹保持不变"), "{rendered}");
    assert!(rendered.contains(&plan.content_baseline), "{rendered}");
    let output = frame(&ctx, &mut app, Vec::new(), 5);
    let scroll_point = visible_text_position(&output, "修改前").expect("逐处预览滚动区应可见");
    for _ in 0..12 {
        if plan.changes[0].occurrences.iter().all(|item| {
            rendered.contains(&item.before_context) && rendered.contains(&item.after_context)
        }) {
            break;
        }
        let output = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(scroll_point),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -120.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            5,
        );
        for shape in &output.shapes {
            collect_text(&shape.shape, &mut rendered);
        }
    }
    for item in &plan.changes[0].occurrences {
        assert!(rendered.contains(&item.before_context), "{rendered}");
        assert!(rendered.contains(&item.after_context), "{rendered}");
    }
    for _ in 0..4 {
        let _ = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(scroll_point),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -120.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            5,
        );
    }
    click(&ctx, &mut app, 5, "应用跨视图重命名");
    let source = app.project.document(&app.active_file).unwrap();
    assert!(source.contains("entity alpha kind place as \"entity a 中文\" // entity a 不改"));
    assert!(source.contains("alias entity alpha as \"entity a\""));
}

#[test]
fn rename_failed_preview_discards_old_plan_and_display_route_keeps_id() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    app.plan_target_rename(TargetRef::new("entity", "a"));
    app.rename_form.as_mut().unwrap().new_id = "alpha".into();
    click(&ctx, &mut app, 5, "预览重命名");
    app.rename_form.as_mut().unwrap().new_id = "b".into();
    click(&ctx, &mut app, 5, "预览重命名");
    assert!(app.rename_form.as_ref().unwrap().plan.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    click(&ctx, &mut app, 5, "改显示名（保留 ID）");
    assert!(app.rename_form.is_none());
    assert_eq!(app.entity_editor.as_ref().unwrap().draft.id, "a");
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn state_owner_rename_ui_refuses_with_details_and_keeps_display_name_route() {
    let (ctx, mut app) = app();
    let source = concat!(
        "entity a kind item as \"封存账册\"\n",
        "tag sealed as \"封存\"\n",
        "state ledger_status on entity a with sealed as \"账册状态\"\n",
    );
    app.project
        .set_text(&app.active_file.clone(), source.into())
        .unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    app.plan_target_rename(TargetRef::new("entity", "a"));
    app.rename_form.as_mut().unwrap().new_id = "renamed".into();
    click(&ctx, &mut app, 5, "预览重命名");
    assert!(app.rename_form.as_ref().unwrap().plan.is_none());
    let error = app.io_error.as_ref().unwrap();
    for detail in ["state ledger_status", "world.wl:3", "fingerprint", "save"] {
        assert!(error.contains(detail), "{error}");
    }
    assert_eq!(app.project.content_baseline(), baseline);
    click(&ctx, &mut app, 5, "改显示名（保留 ID）");
    assert!(app.rename_form.is_none());
    assert_eq!(app.entity_editor.as_ref().unwrap().draft.id, "a");
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
}

#[test]
fn display_name_route_keeps_both_inputs_when_an_entity_form_is_dirty() {
    let (ctx, mut app) = app();
    app.edit_entity(Some("b"));
    app.entity_editor.as_mut().unwrap().draft.display = "尚未应用的资料".into();
    app.plan_target_rename(TargetRef::new("entity", "a"));
    app.rename_form.as_mut().unwrap().new_id = "alpha".into();
    click(&ctx, &mut app, 5, "改显示名（保留 ID）");
    assert_eq!(app.rename_form.as_ref().unwrap().new_id, "alpha");
    assert_eq!(app.entity_editor.as_ref().unwrap().draft.id, "b");
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.display,
        "尚未应用的资料"
    );
}

#[test]
fn rename_preview_remains_scrollable_at_native_minimum_and_large_type() {
    for (size, scale) in [
        (vec2(1040.0, 660.0), 1.0),
        (vec2(1040.0, 660.0), 1.5),
        (vec2(1040.0, 660.0), 2.0),
        (vec2(1440.0, 1000.0), 1.5),
    ] {
        let (ctx, mut app) = app();
        ctx.style_mut(|style| {
            for font in style.text_styles.values_mut() {
                font.size *= scale;
            }
        });
        let source = format!(
            "entity a kind item as \"长中文资料名称\"\n{}",
            (0..30)
                .map(|i| format!(
                    "alias entity a as \"第{i}处中文显示文字与同行注释保持原样\" // entity a\n"
                ))
                .collect::<String>()
        );
        app.project
            .set_text(&app.active_file.clone(), source)
            .unwrap();
        app.recompile();
        app.plan_target_rename(TargetRef::new("entity", "a"));
        let form = app.rename_form.as_mut().unwrap();
        form.new_id = "reviewed_record".into();
        form.preview(&app.project, app.version).unwrap();
        let baseline = app.project.content_baseline();
        let screen = Rect::from_min_size(pos2(0.0, 0.0), size);
        let mut events = Vec::new();
        let mut reachable = false;
        let mut last_rendered = String::new();
        let mut last_area = screen;
        for _ in 0..100 {
            let output = ctx.run(
                RawInput {
                    screen_rect: Some(screen),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    app.target_rename_window(ctx);
                },
            );
            last_rendered.clear();
            for shape in &output.shapes {
                collect_text(&shape.shape, &mut last_rendered);
            }
            if let Some(point) = visible_text_position(&output, "应用跨视图重命名") {
                if screen.contains(point) {
                    let area = ctx
                        .memory(|memory| memory.area_rect(egui::Id::new("target-rename")))
                        .unwrap();
                    assert!(
                        screen.expand(1.0).contains_rect(area),
                        "完整确认窗口须在屏幕内：{area:?}"
                    );
                    reachable = true;
                    break;
                }
            }
            let area = ctx
                .memory(|memory| memory.area_rect(egui::Id::new("target-rename")))
                .unwrap();
            last_area = area;
            let point = area.intersect(screen).center();
            events = vec![
                Event::PointerMoved(point),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -300.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ];
        }
        assert!(
            reachable,
            "{size:?} × {scale} 下应用动作必须可滚动到达；区域{last_area:?}；末帧{last_rendered}"
        );
        assert_eq!(app.project.content_baseline(), baseline);
        assert!(app.history.is_empty());
    }
}

#[test]
fn template_fields_preserve_body_and_custom_values_and_suggestions_only_open_drafts() {
    let (ctx, mut app) = app();
    app.edit_entity(None);
    {
        let form = app.entity_editor.as_mut().unwrap();
        form.draft.display = "模板地点".into();
        form.draft.description = "第一段\n第二段正文".into();
        form.draft.properties.push((
            "custom_unknown".into(),
            worldline_core::ast::PropertyValue::Str("保留".into()),
        ));
    }
    click(&ctx, &mut app, 0, "创作模板 · 地理与地点");
    click(&ctx, &mut app, 0, "＋ 视觉与感官印象");
    {
        let form = app.entity_editor.as_mut().unwrap();
        let field = form
            .draft
            .properties
            .iter_mut()
            .find(|(key, _)| key == "place_1")
            .unwrap();
        field.1 = worldline_core::ast::PropertyValue::Str("海风与白石".into());
        form.draft.entity_type = "organization".into();
    }
    click(&ctx, &mut app, 0, "应用资料");
    let id = app.catalog_target.as_ref().unwrap().id.clone();
    let entity = &app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .entities[&id];
    assert_eq!(entity.description, "第一段\n第二段正文");
    assert_eq!(
        entity.properties["custom_unknown"],
        worldline_core::ast::PropertyValue::Str("保留".into())
    );
    assert_eq!(
        entity.properties["place_1"],
        worldline_core::ast::PropertyValue::Str("海风与白石".into())
    );

    let baseline = app.project.content_baseline();
    app.edit_entity(Some(&id));
    click(&ctx, &mut app, 0, "创作模板 · 组织与制度");
    click(&ctx, &mut app, 0, "任职于");
    assert!(app.relation_editor.is_some());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app
        .relation_editor
        .as_ref()
        .is_some_and(|form| form.draft.from == TargetRef::new("entity", &id)));
}
