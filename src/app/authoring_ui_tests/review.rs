use super::*;

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
    let output = frame(&ctx, &mut app, Vec::new(), 7);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("当前差异已过期"), "{rendered}");
    click(&ctx, &mut app, 7, "重新比较提案");
    let output = frame(&ctx, &mut app, Vec::new(), 7);
    let mut compared = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut compared);
    }
    assert!(compared.contains("变化："), "{compared}");
    assert!(compared.contains("基底"), "{compared}");
    assert!(compared.contains("当前"), "{compared}");
    assert!(compared.contains("提议"), "{compared}");
    click(&ctx, &mut app, 7, "采纳提案");
    assert_eq!(app.project.content_baseline(), before);
    assert_eq!(
        app.snapshot.as_ref().unwrap().proposal_index.proposals["proposal_ui"]
            .draft
            .status,
        worldline_core::collaboration::ProposalStatus::Open
    );
}

#[test]
fn narrow_review_switches_between_selectable_three_way_text() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    app.recompile();
    let entry = app.project.entry.clone();
    let original = app.project.document(&entry).unwrap().to_string();
    let mut proposed = original.clone();
    proposed.push_str("entity c kind place as \"新增地点\"\n");
    app.project.set_text(&entry, proposed).unwrap();
    app.recompile();
    app.review.author = "乙".into();
    app.review.reason = "检查窄屏差异".into();
    click(&ctx, &mut app, 7, "保存修改提案");
    app.project.set_text(&entry, original.clone()).unwrap();
    app.recompile();
    click(&ctx, &mut app, 16, "重新比较提案");
    for _ in 0..4 {
        let _ = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos2(200.0, 450.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: vec2(0.0, -8.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            16,
        );
    }
    let output = frame(&ctx, &mut app, Vec::new(), 16);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("基底"), "{rendered}");
    assert!(rendered.contains("当前"), "{rendered}");
    assert!(rendered.contains("提议"), "{rendered}");
    assert!(rendered.contains("提议修改"), "{rendered}");
    click(&ctx, &mut app, 16, "提议");
    let output = frame(&ctx, &mut app, Vec::new(), 16);
    let mut proposed_view = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut proposed_view);
    }
    assert!(proposed_view.contains("新增地点"), "{proposed_view}");
    let history_before_apply = app.history.len();
    click(&ctx, &mut app, 16, "采纳提案");
    assert!(
        app.project.document(&entry).unwrap().contains("新增地点"),
        "{:?}",
        app.io_error
    );
    assert_eq!(app.history.len(), history_before_apply + 1);
    app.undo(false);
    assert_eq!(app.project.document(&entry).unwrap(), original);
}

fn prepare_proposal_title_conflict(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
) -> (std::path::PathBuf, String) {
    let manifest_path = app.project.root.join(".world/project.json");
    let mut manifest: serde_json::Value = serde_json::from_slice(
        app.project
            .authoring_document(&manifest_path)
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["required_features"]
        .as_array_mut()
        .unwrap()
        .push("presentation.maps.v1".into());
    manifest["maps"]["review"] = ".world/maps/review.json".into();
    app.project
        .set_authoring_document(&manifest_path, serde_json::to_vec(&manifest).unwrap())
        .unwrap();

    let map_path = app.project.root.join(".world/maps/review.json");
    let mut base = serde_json::json!({
        "schema_version": 1,
        "id": "review",
        "title": "基底",
        "raster_layers": [],
        "canvas": {"width": 1000, "height": 800, "unit": "normalized"},
        "layer_order": [],
        "layers": {},
        "placements": {},
        "extensions": {}
    });
    base["extensions"]["retired"] = "旧".into();
    let base_text = serde_json::to_string(&base).unwrap();
    app.project
        .create_authoring_document(&map_path, base_text.as_bytes().to_vec())
        .unwrap();
    app.project.save().unwrap();
    app.recompile();

    let mut proposed = base.clone();
    proposed["title"] = "提议".into();
    proposed["extensions"]
        .as_object_mut()
        .unwrap()
        .remove("retired");
    proposed["extensions"]["added"] = "新增".into();
    let proposed_text = serde_json::to_string(&proposed).unwrap();
    app.project
        .set_authoring_document(&map_path, proposed_text.as_bytes().to_vec())
        .unwrap();
    app.recompile();
    app.review.author = "审阅者".into();
    app.review.reason = "解决同字段冲突".into();
    app.review.proposal_id = "proposal_resolution".into();
    click(ctx, app, 7, "保存修改提案");

    let mut current = base;
    current["title"] = "当前".into();
    app.project
        .set_authoring_document(&map_path, serde_json::to_vec(&current).unwrap())
        .unwrap();
    app.recompile();
    click(ctx, app, 7, "重新比较提案");
    (map_path, proposed_text)
}

#[test]
fn proposal_conflict_can_be_resolved_from_a_side_and_undone() {
    let (ctx, mut app) = app();
    let (map_path, proposed_text) = prepare_proposal_title_conflict(&ctx, &mut app);
    let proposal =
        &app.snapshot.as_ref().unwrap().proposal_index.proposals["proposal_resolution"].draft;
    let preview = worldline_core::collaboration::preview_proposal(&app.project, proposal).unwrap();
    assert_eq!(preview.conflicts.len(), 1, "{:?}", preview.conflicts);
    let output = frame(&ctx, &mut app, Vec::new(), 7);
    let mut markers = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut markers);
    }
    assert!(markers.contains("当前修改 · 提议修改"), "{markers}");
    assert!(markers.contains("当前未改 · 提议新增"), "{markers}");
    assert!(markers.contains("当前未改 · 提议删除"), "{markers}");

    click(&ctx, &mut app, 7, "采纳提案");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            app.project.authoring_document(&map_path).unwrap().bytes()
        )
        .unwrap()["title"],
        "当前"
    );

    click(&ctx, &mut app, 7, "采用提议");
    let history_before_apply = app.history.len();
    click(&ctx, &mut app, 7, "采纳提案");
    let resolved: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&map_path).unwrap().bytes()).unwrap();
    assert_eq!(resolved["title"], "提议", "{:?}", app.io_error);
    assert_eq!(app.history.len(), history_before_apply + 1);
    let proposal = &app.snapshot.as_ref().unwrap().proposal_index.proposals["proposal_resolution"];
    assert_eq!(
        proposal.draft.status,
        worldline_core::collaboration::ProposalStatus::Accepted
    );
    assert_eq!(
        proposal.draft.changes[0].proposed.as_deref(),
        Some(proposed_text.as_str())
    );

    app.undo(false);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            app.project.authoring_document(&map_path).unwrap().bytes()
        )
        .unwrap()["title"],
        "当前"
    );
    assert_eq!(
        app.snapshot.as_ref().unwrap().proposal_index.proposals["proposal_resolution"]
            .draft
            .status,
        worldline_core::collaboration::ProposalStatus::Open
    );
    click(&ctx, &mut app, 7, "采纳提案");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            app.project.authoring_document(&map_path).unwrap().bytes()
        )
        .unwrap()["title"],
        "当前",
        "an accepted proposal's old resolution must not be silently reused after undo"
    );
}

#[test]
fn proposal_conflict_can_be_edited_before_core_application() {
    let (ctx, mut app) = app();
    let (map_path, _) = prepare_proposal_title_conflict(&ctx, &mut app);
    replace_text_area(&ctx, &mut app, 7, "编辑 JSON 值", "\"已解决\"");
    let mut updated_current: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&map_path).unwrap().bytes()).unwrap();
    updated_current["title"] = "当前更新".into();
    app.project
        .set_authoring_document(&map_path, serde_json::to_vec(&updated_current).unwrap())
        .unwrap();
    app.recompile();
    let output = frame(&ctx, &mut app, Vec::new(), 7);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("当前差异已过期"), "{rendered}");
    click(&ctx, &mut app, 7, "采纳提案");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            app.project.authoring_document(&map_path).unwrap().bytes()
        )
        .unwrap()["title"],
        "当前更新"
    );
    click(&ctx, &mut app, 7, "重新比较提案");
    click(&ctx, &mut app, 7, "采纳提案");
    let resolved: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&map_path).unwrap().bytes()).unwrap();
    assert_eq!(resolved["title"], "已解决", "{:?}", app.io_error);
    assert_eq!(
        app.snapshot.as_ref().unwrap().proposal_index.proposals["proposal_resolution"]
            .draft
            .status,
        worldline_core::collaboration::ProposalStatus::Accepted
    );
}
