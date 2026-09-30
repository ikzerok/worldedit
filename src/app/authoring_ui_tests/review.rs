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
    scroll_from_visible_anchor_to(&ctx, &mut app, 16, "协作审阅", "重新比较提案");
    click(&ctx, &mut app, 16, "重新比较提案");
    let rendered = scroll_review_to(&ctx, &mut app, "基底");
    assert!(rendered.contains("基底"), "{rendered}");
    assert!(rendered.contains("当前"), "{rendered}");
    assert!(rendered.contains("提议"), "{rendered}");
    assert!(rendered.contains("提议修改"), "{rendered}");
    click(&ctx, &mut app, 16, "提议");
    assert_eq!(app.review.preview_side, 2, "必须实际切到提议侧");
    scroll_review_to(&ctx, &mut app, "新增地点");
    let output = frame(&ctx, &mut app, Vec::new(), 16);
    let mut proposed_view = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut proposed_view);
    }
    assert!(proposed_view.contains("新增地点"), "{proposed_view}");
    let history_before_apply = app.history.len();
    scroll_review_to(&ctx, &mut app, "采纳提案");
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

fn scroll_review_to(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) -> String {
    let mut rendered = String::new();
    for delta in [-60.0, 60.0] {
        for _ in 0..160 {
            for _ in 0..8 {
                let _ = frame(ctx, app, Vec::new(), 16);
            }
            let output = frame(ctx, app, Vec::new(), 16);
            rendered.clear();
            for shape in &output.shapes {
                collect_text(&shape.shape, &mut rendered);
            }
            if visible_text_position(&output, label).is_some() {
                return rendered;
            }
            let point = pos2(
                ctx.screen_rect().width() * 0.25,
                ctx.screen_rect().height() * 0.5,
            );
            let _ = frame(
                ctx,
                app,
                vec![
                    Event::PointerMoved(point),
                    Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        delta: vec2(0.0, delta),
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                16,
            );
        }
    }
    panic!("审阅控件滚动后不可达：{label}；{rendered}");
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

#[test]
fn conflicting_zero_write_preview_is_explicit_and_close_preserves_drafts_and_stale_guard() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let entry = app.project.entry.clone();
    let base = app.project.document(&entry).unwrap().to_owned();
    let proposed = base.replacen("同名", "提议地名", 1);
    app.project.set_text(&entry, proposed.clone()).unwrap();
    app.recompile();
    app.review.author = "乙".into();
    app.review.reason = "文案同字段冲突".into();
    app.review.proposal_id = "summary_conflict".into();
    click(&ctx, &mut app, 7, "保存修改提案");
    app.project
        .set_text(&entry, base.replacen("同名", "当前地名", 1))
        .unwrap();
    app.recompile();
    click(&ctx, &mut app, 7, "重新比较提案");
    let proposal =
        &app.snapshot.as_ref().unwrap().proposal_index.proposals["summary_conflict"].draft;
    let preview = worldline_core::collaboration::preview_proposal(&app.project, proposal).unwrap();
    assert_eq!(
        preview.content_files(),
        0,
        "core计数是当前合并结果，不是三方版本差异"
    );
    assert_eq!(preview.conflicts.len(), 1);
    let rendered = rendered_text_in_window(&ctx, &mut app, 7, "提案三方预览");
    assert!(
        rendered.contains("当前合并结果拟写入：内容 0 个文件"),
        "{rendered}"
    );
    assert!(rendered.contains("待解决冲突 · 暂不可应用"), "{rendered}");
    assert!(
        !rendered.contains("content · world.wl · 无变化"),
        "{rendered}"
    );
    let before = app.project.content_baseline();
    let history = app.history.len();
    click(&ctx, &mut app, 7, "关闭预览");
    assert!(app.review.selected_proposal.is_none());
    assert!(app.review.preview.is_some(), "关闭不能清掉过期检查的原基线");
    assert_eq!(app.project.content_baseline(), before);
    assert_eq!(app.history.len(), history);
    click(&ctx, &mut app, 7, "待审阅 · 乙 · 文案同字段冲突");
    click(&ctx, &mut app, 7, "采用提议");
    let rendered = rendered_text_in_window(&ctx, &mut app, 7, "解决草稿已齐");
    assert!(rendered.contains("仍待明确采纳"), "{rendered}");
    assert_eq!(app.project.content_baseline(), before);
    click(&ctx, &mut app, 7, "关闭预览");
    let newer = base.replacen("同名", "后来地名", 1);
    app.project.set_text(&entry, newer.clone()).unwrap();
    app.recompile();
    let changed = app.project.content_baseline();
    click(&ctx, &mut app, 7, "待审阅 · 乙 · 文案同字段冲突");
    let rendered = rendered_text_in_window(&ctx, &mut app, 7, "当前差异已过期");
    assert!(rendered.contains("采纳已禁用"), "{rendered}");
    click(&ctx, &mut app, 7, "采纳提案");
    assert_eq!(app.project.content_baseline(), changed);
    assert_eq!(app.history.len(), history);
    click(&ctx, &mut app, 7, "重新比较提案");
    let rendered = rendered_text_in_window(&ctx, &mut app, 7, "解决草稿已齐");
    assert!(
        rendered.contains("仍待明确采纳"),
        "关闭重开与重新比较应保留解决草稿：{rendered}"
    );
    click(&ctx, &mut app, 7, "采纳提案");
    assert_eq!(app.project.document(&entry).unwrap(), proposed);
    assert_eq!(app.history.len(), history + 1);
    app.undo(false);
    assert_eq!(app.project.document(&entry).unwrap(), newer);
    assert_eq!(
        app.snapshot.as_ref().unwrap().proposal_index.proposals["summary_conflict"]
            .draft
            .status,
        worldline_core::collaboration::ProposalStatus::Open
    );
}
