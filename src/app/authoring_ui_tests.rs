//! 真实 egui 窗口按钮回归；不是已显示桌面或浏览器的人工验收。
use super::WorldeditApp;
use egui::{pos2, vec2, Event, PointerButton, RawInput, Rect};
use worldline_core::{project::Project, TargetRef};

fn app() -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = std::env::temp_dir().join(format!("worldedit-form-ui-{}", std::process::id()));
    app.project = Project::new(&root);
    let entry = app.project.entry.clone();
    app.project.documents.retain(|path, _| path == &entry);
    app.project.set_text(&entry, "entity a kind place as \"同名\"\nentity b kind organization as \"同名\"\nrelation_type knows as \"认识\"\n".into()).unwrap();
    app.project.create_authoring_document(&root.join(".world/project.json"), br#"{
        "schema_version":1,"language_version":"1.10",
        "required_features":["content.entities.v1","content.relations.v1"],"maps":{},"graph_views":{}
    }"#.to_vec()).unwrap();
    app.active_file = entry;
    app.reset_views();
    app.recompile();
    (ctx, app)
}
fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
    window: u8,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                if window == 9 {
                    vec2(1040.0, 660.0)
                } else {
                    vec2(1700.0, 1400.0)
                },
            )),
            events,
            ..Default::default()
        },
        |ctx| match window {
            0 => app.entity_editor_window(ctx),
            1 => app.relation_editor_window(ctx),
            2 => app.relation_type_editor_window(ctx),
            4 => app.network_tab(ctx),
            5 => app.target_rename_window(ctx),
            6 => app.preset_editor_window(ctx),
            7 => app.review_tab(ctx),
            8 | 9 => app.reading_window(ctx),
            _ => app.content_deletion_window(ctx),
        },
    )
}

#[test]
fn pinning_two_reading_panels_keeps_source_and_undo_unchanged() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    app.open_reading(TargetRef::new("entity", "a"));
    click(&ctx, &mut app, 8, "钉住旁查");
    app.open_reading(TargetRef::new("entity", "b"));
    click(&ctx, &mut app, 8, "钉住旁查");
    assert_eq!(app.reading_panels.ids().len(), 2);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    app.reset_views();
    assert!(app.reading_panels.ids().is_empty());
}

#[test]
fn narrow_reading_panels_can_switch_and_close_without_losing_an_edit_draft() {
    let (ctx, mut app) = app();
    let first = app
        .reading_panels
        .pin(TargetRef::new("entity", "a"))
        .unwrap();
    let second = app
        .reading_panels
        .pin(TargetRef::new("entity", "b"))
        .unwrap();
    app.selected_reading_panel = Some(first);
    app.edit_entity(Some("a"));
    app.entity_editor.as_mut().unwrap().draft.description = "未提交资料".into();
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 9, "旁查 2");
    assert_eq!(app.selected_reading_panel, Some(second));
    click(&ctx, &mut app, 9, "返回源码编辑");
    assert_eq!(app.tab, super::Tab::Edit);
    click(&ctx, &mut app, 9, "编辑此对象");
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.description,
        "未提交资料"
    );
    click(&ctx, &mut app, 9, "关闭旁查");
    assert!(app.reading_panels.get(second).is_none());
    assert!(app.reading_panels.get(first).is_some());
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.description,
        "未提交资料"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn project_switch_preserves_unsubmitted_form_even_with_clean_project() {
    let (ctx, mut app) = app();
    app.project.mark_saved();
    app.edit_entity(Some("a"));
    app.entity_editor.as_mut().unwrap().draft.description = "未提交资料".into();
    app.request_action(super::Pending::Close, &ctx);
    assert!(!app.allow_close);
    assert!(app.pending.is_none());
    assert_eq!(
        app.entity_editor.as_ref().unwrap().draft.description,
        "未提交资料"
    );
}

#[test]
fn pinned_deleted_target_shows_invalid_identity_instead_of_another_object() {
    let (ctx, mut app) = app();
    let id = app
        .reading_panels
        .pin(TargetRef::new("entity", "a"))
        .unwrap();
    app.project
        .set_text(
            &app.project.entry.clone(),
            "entity b kind place as \"同名\"\n".into(),
        )
        .unwrap();
    app.recompile();
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 9);
    }
    let output = frame(&ctx, &mut app, Vec::new(), 9);
    assert!(output.shapes.iter().any(|shape| text_position(
        &shape.shape,
        "资料已失效：entity:a。可能已被删除或更改 ID。"
    )
    .is_some()));
    assert_eq!(app.reading_panels.get(id).unwrap().target.id, "a");
}
fn text_position(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text == label => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| text_position(shape, label)),
        _ => None,
    }
}
fn click(ctx: &egui::Context, app: &mut WorldeditApp, window: u8, label: &str) {
    for _ in 0..3 {
        let _ = frame(ctx, app, Vec::new(), window);
    }
    let output = frame(ctx, app, Vec::new(), window);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position(&shape.shape, label))
        .unwrap_or_else(|| panic!("按钮未显示：{label}"));
    for pressed in [true, false] {
        let _ = frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            window,
        );
    }
}
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
fn network_browsing_is_personal_until_shared_layout_is_saved() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let mut source = app.project.sources()[&entry].clone();
    source.push_str("relation_def r type knows from entity a to entity b\n");
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    app.open_network(TargetRef::new("entity", "a"));
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 4);
    }
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    assert_eq!(app.network_state.result.as_ref().unwrap().edges.len(), 1);

    app.network_view_id = "view_a".into();
    app.network_view_title = "A 的关联".into();
    click(&ctx, &mut app, 4, "保存共享布局");
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), 1);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .graph_index
        .views
        .contains_key("view_a"));
    assert_ne!(app.project.content_baseline(), baseline);
}

#[test]
fn presentation_preset_save_is_explicit_and_apply_is_personal() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let mut source = app.project.sources()[&entry].clone();
    source.push_str("relation_def r type knows from entity a to entity b\n");
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    app.open_network(TargetRef::new("entity", "a"));
    app.network_view_id = "view_a".into();
    app.network_view_title = "A 的关联".into();
    click(&ctx, &mut app, 4, "保存共享布局");

    let before_preset = app.project.content_baseline();
    let history_before = app.history.len();
    app.open_preset_editor(None);
    {
        let form = app.preset_editor.as_mut().unwrap();
        form.draft.id = "preset_a".into();
        form.draft.title = "A 的专题".into();
        form.draft.map_id = None;
        form.draft.graph_view_id = Some("view_a".into());
    }
    click(&ctx, &mut app, 6, "保存展示预设");
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), history_before + 1);
    assert_ne!(app.project.content_baseline(), before_preset);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .preset_index
        .presets
        .contains_key("preset_a"));

    let saved = app.project.content_baseline();
    let history_after_save = app.history.len();
    app.apply_presentation_preset("preset_a");
    assert_eq!(app.project.content_baseline(), saved);
    assert_eq!(app.history.len(), history_after_save);
    assert_eq!(app.network_state.focus, Some(TargetRef::new("entity", "a")));
}

#[cfg(not(debug_assertions))]
#[test]
fn network_release_profile_meets_m2_frame_and_reading_gates() {
    let (ctx, mut app) = app();
    let mut source = String::from("relation_type links as \"连接\"\n");
    for index in 0..1000 {
        source.push_str(&format!("entity e{index} kind place as \"对象 {index}\"\n"));
    }
    for index in 0..3000 {
        let from = if index < 600 { 0 } else { index % 1000 };
        let to = (index * 37 + 1) % 1000;
        source.push_str(&format!(
            "relation_def r{index} type links from entity e{from} to entity e{to}\n"
        ));
    }
    let entry = app.project.entry.clone();
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    app.open_network(TargetRef::new("entity", "e0"));

    for _ in 0..8 {
        let _ = frame(&ctx, &mut app, Vec::new(), 4);
    }
    let mut frame_ms = Vec::new();
    for _ in 0..160 {
        let started = std::time::Instant::now();
        let _ = frame(&ctx, &mut app, Vec::new(), 4);
        frame_ms.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    frame_ms.sort_by(f64::total_cmp);
    let p95_frame = frame_ms[(frame_ms.len() * 95 / 100).min(frame_ms.len() - 1)];
    let max_frame = *frame_ms.last().unwrap();

    let mut reading_ms = Vec::new();
    for index in 0..160 {
        app.open_reading(TargetRef::new("entity", &format!("e{}", index % 1000)));
        let started = std::time::Instant::now();
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1700.0, 1400.0))),
                ..Default::default()
            },
            |ctx| app.reading_window(ctx),
        );
        reading_ms.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    reading_ms.sort_by(f64::total_cmp);
    let p95_reading = reading_ms[(reading_ms.len() * 95 / 100).min(reading_ms.len() - 1)];
    let max_reading = *reading_ms.last().unwrap();

    println!(
        "WP10_PROFILE frame_p95_ms={p95_frame:.3} frame_max_ms={max_frame:.3} reading_p95_ms={p95_reading:.3} reading_max_ms={max_reading:.3}"
    );
    assert!(p95_frame <= 33.0, "网络帧 P95 {p95_frame:.3}ms 超过 33ms");
    assert!(
        p95_reading <= 200.0,
        "暖态资料切换 P95 {p95_reading:.3}ms 超过 200ms"
    );
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

#[test]
fn collaboration_review_saves_comments_and_keeps_conflicting_proposals_open() {
    let (ctx, mut app) = app();
    app.new_comment_for_anchor(worldline_core::collaboration::CommentAnchor::Object {
        target: TargetRef::new("entity", "a"),
    });
    {
        let editor = app.review.comment_editor.as_mut().unwrap();
        editor.draft.author = "甲".into();
        editor.draft.body = "请补充来源".into();
    }
    click(&ctx, &mut app, 7, "保存批注");
    assert!(app.review.comment_editor.is_none(), "{:?}", app.io_error);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .comment_index
        .comments
        .contains_key("comment_1"));

    let entry = app.project.entry.clone();
    let mut proposed = app.project.document(&entry).unwrap().to_string();
    proposed.push_str("# 提案版本\n");
    app.project.set_text(&entry, proposed).unwrap();
    app.recompile();
    app.review.author = "乙".into();
    app.review.reason = "审阅正文修改".into();
    app.review.proposal_id = "proposal_ui".into();
    click(&ctx, &mut app, 7, "保存修改提案");
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .proposal_index
        .proposals
        .contains_key("proposal_ui"));

    let mut current = app.project.document(&entry).unwrap().to_string();
    current.push_str("# 并行当前修改\n");
    app.project.set_text(&entry, current).unwrap();
    app.recompile();
    let before = app.project.content_baseline();
    click(&ctx, &mut app, 7, "采纳提案");
    assert_eq!(app.project.content_baseline(), before);
    assert_eq!(
        app.snapshot.as_ref().unwrap().proposal_index.proposals["proposal_ui"]
            .draft
            .status,
        worldline_core::collaboration::ProposalStatus::Open
    );
}
