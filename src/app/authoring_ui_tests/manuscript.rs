use super::*;

#[test]
fn manuscript_reordering_uses_one_core_command_and_undo_keeps_story_semantics() {
    let (ctx, mut app) = manuscript_app();
    let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
    let content_baseline = app.project.content_baseline();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());

    click(&ctx, &mut app, 13, "下移");
    assert_eq!(
        app.project
            .manuscript_index("novel")
            .unwrap()
            .page(0, 10)
            .chapters[0]
            .id,
        "opening"
    );
    assert!(app.history.is_empty(), "草稿重排尚未应用");
    click(&ctx, &mut app, 13, "应用书稿");

    let reordered = app.project.manuscript_index("novel").unwrap();
    assert_eq!(reordered.page(0, 10).chapters[0].id, "departure");
    assert_eq!(app.history.len(), 1);
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
    assert_eq!(
        app.snapshot
            .as_ref()
            .unwrap()
            .result
            .program
            .events
            .iter()
            .map(|event| event.name.as_str())
            .collect::<Vec<_>>(),
        ["arrival", "departure"]
    );

    app.undo(false);
    assert_eq!(
        app.project
            .manuscript_index("novel")
            .unwrap()
            .page(0, 10)
            .chapters[0]
            .id,
        "opening"
    );
    assert_eq!(app.project.content_baseline(), content_baseline);
}

#[test]
fn manuscript_preview_and_stats_come_from_core_projection() {
    let (ctx, mut app) = manuscript_app();
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("汉字 8 · 词数 8"), "{rendered}");
    assert!(rendered.contains("甲乙"), "{rendered}");
    assert!(rendered.contains("灯塔亮起"), "{rendered}");
    assert!(
        !rendered.contains("-> END"),
        "控制流不能混入阅读正文：{rendered}"
    );
    assert!(rendered.contains("书稿只决定阅读顺序"), "{rendered}");
    assert!(
        rendered.contains("独立于世界时间与事件控制流"),
        "{rendered}"
    );
    click(&ctx, &mut app, 13, "卡片");
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let mut cards = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut cards);
    }
    assert!(cards.contains("查看章节"), "{cards}");
    click(&ctx, &mut app, 13, "列表");
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let mut list = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut list);
    }
    assert!(list.contains("抵达"), "{list}");
    click(&ctx, &mut app, 13, "章节树");
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position_contains(&shape.shape, "灯塔亮起"))
        .unwrap();
    for _ in 0..8 {
        let _ = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(point),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: vec2(0.0, -12.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            13,
        );
    }
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let mut scrolled = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut scrolled);
    }
    assert!(
        scrolled.contains("同名地点资料"),
        "书稿长预览应可滚动：{scrolled}"
    );
}
#[test]
fn manuscript_workbench_scrolls_to_preview_in_short_viewport() {
    let (ctx, mut app) = manuscript_app();
    let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0));
    let visible = |output: &egui::FullOutput, needle: &str| {
        output.shapes.iter().find_map(|clipped| {
            let point = text_position_contains(&clipped.shape, needle)?;
            (screen.contains(point) && clipped.clip_rect.contains(point)).then_some(point)
        })
    };
    let mut output = frame(&ctx, &mut app, Vec::new(), 31);
    for field in ["状态", "字数目标", "event:arrival"] {
        assert!(
            visible(&output, field).is_some(),
            "chapter field must be visible in a short viewport: {field}"
        );
    }
    let scroll_point = visible(&output, "稳定 ID").unwrap();
    for _ in 0..32 {
        if visible(&output, "按书稿章节顺序展示 core 编译的静态文本").is_some() {
            break;
        }
        output = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(scroll_point),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -90.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            31,
        );
    }
    assert!(
        visible(&output, "按书稿章节顺序展示 core 编译的静态文本").is_some(),
        "reading preview must be reachable by scrolling in a short viewport"
    );
}
#[test]
fn manuscript_detail_fields_remain_reachable_in_narrow_viewport() {
    fn text_bounds(shape: &egui::Shape, needle: &str) -> Option<Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.job.text.contains(needle) => {
                Some(Rect::from_min_size(
                    text.pos + text.galley.rect.min.to_vec2(),
                    text.galley.rect.size(),
                ))
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| text_bounds(shape, needle)),
            _ => None,
        }
    }
    fn exact_text_position(shape: &egui::Shape, needle: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.job.text.trim() == needle => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            egui::Shape::Vec(shapes) => shapes
                .iter()
                .find_map(|shape| exact_text_position(shape, needle)),
            _ => None,
        }
    }
    let (ctx, mut app) = manuscript_app();
    let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let bounds = |output: &egui::FullOutput, needle: &str| {
        output
            .shapes
            .iter()
            .find_map(|clipped| text_bounds(&clipped.shape, needle))
    };
    let mut output = frame(&ctx, &mut app, Vec::new(), 32);
    let chapter_heading = output
        .shapes
        .iter()
        .find_map(|clipped| exact_text_position(&clipped.shape, "章节"))
        .expect("chapter list heading");
    let detail_heading = output
        .shapes
        .iter()
        .find_map(|clipped| exact_text_position(&clipped.shape, "编排与来源"))
        .expect("chapter detail heading");
    assert!(
        detail_heading.y > chapter_heading.y + 40.0,
        "narrow layout should stack chapter selection above its details"
    );

    for _ in 0..16 {
        if bounds(&output, "event:arrival")
            .is_some_and(|rect| rect.top() >= 60.0 && rect.bottom() <= 540.0)
        {
            break;
        }
        output = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos2(700.0, 500.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -90.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            32,
        );
    }
    for _ in 0..16 {
        if bounds(&output, "event:arrival").is_some_and(|rect| screen.contains_rect(rect)) {
            break;
        }
        output = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos2(700.0, 500.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(-90.0, 0.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            32,
        );
    }
    assert!(
        bounds(&output, "event:arrival").is_some_and(|rect| screen.contains_rect(rect)),
        "chapter target text must fit inside the narrow viewport after scrolling"
    );
    for field in ["状态", "字数目标"] {
        assert!(
            bounds(&output, field).is_some_and(|rect| screen.contains_rect(rect)),
            "manuscript field must fit inside the narrow viewport: {field}"
        );
    }
}

#[test]
fn manuscript_body_editor_keeps_unsubmitted_input_and_applies_through_egui() {
    let (ctx, mut app) = manuscript_app();
    let baseline = app.project.content_baseline();
    let replacement = concat!(
        "character traveler as \"旅人\"\n",
        "event arrival as \"抵达\"\n",
        "  全新正文。\n",
        "  -> END\n",
        "event departure as \"离港\"\n",
        "  远航。\n",
        "  -> END\n",
    );
    replace_manuscript_source(&ctx, &mut app, replacement);
    assert_eq!(app.project.content_baseline(), baseline, "输入仍只是草稿");
    click(&ctx, &mut app, 13, "应用正文草稿");
    assert!(app
        .project
        .document(&app.active_file)
        .unwrap()
        .contains("全新正文。"));
    assert_eq!(app.history.len(), 1);
    app.undo(false);
    assert!(app
        .project
        .document(&app.active_file)
        .unwrap()
        .contains("甲乙"));
}

#[test]
fn stale_manuscript_body_keeps_input_and_does_not_overwrite_new_source() {
    let (ctx, mut app) = manuscript_app();
    let replacement = concat!(
        "character traveler as \"旅人\"\n",
        "event arrival as \"抵达\"\n",
        "  保留在草稿的文字。\n",
        "  -> END\n",
    );
    replace_manuscript_source(&ctx, &mut app, replacement);
    let externally_changed = "event arrival as \"抵达\"\n  外部修改。\n  -> END\n";
    app.project
        .set_text(&app.active_file.clone(), externally_changed.into())
        .unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();

    click(&ctx, &mut app, 13, "应用正文草稿");
    assert_eq!(
        app.project.document(&app.active_file).unwrap(),
        externally_changed
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    assert!(app
        .io_error
        .as_deref()
        .is_some_and(|error| error.contains("过期")));
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    assert!(
        output
            .shapes
            .iter()
            .any(|shape| text_position_contains(&shape.shape, "保留在草稿的文字").is_some()),
        "过期时保留编辑器输入"
    );
}

#[test]
fn manuscript_target_picker_disambiguates_same_display_names() {
    let (ctx, mut app) = manuscript_app();
    click(&ctx, &mut app, 13, "event:arrival");
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("同名 · entity:a"), "{rendered}");
    assert!(rendered.contains("同名 · entity:b"), "{rendered}");
}

#[test]
fn manuscript_missing_reference_can_be_repaired_through_candidate_picker() {
    let (ctx, mut app) = manuscript_app();
    let path = app.project.root.join(".world/manuscripts/novel.json");
    let mut book: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&path).unwrap().bytes()).unwrap();
    book["entries"][0]["target_ref"]["id"] = "deleted_event".into();
    app.project
        .set_authoring_document(&path, serde_json::to_vec(&book).unwrap())
        .unwrap();
    app.recompile();

    click(&ctx, &mut app, 13, "event:deleted_event");
    click(&ctx, &mut app, 13, "抵达 · event:arrival");
    click(&ctx, &mut app, 13, "应用书稿");

    let chapter = &app
        .project
        .manuscript_index("novel")
        .unwrap()
        .page(0, 10)
        .chapters[0];
    assert_eq!(chapter.target_ref.as_ref().unwrap().id, "arrival");
    assert_eq!(
        chapter.source.as_ref().unwrap().status,
        worldline_core::ManuscriptReferenceStatus::Resolved
    );
    assert_eq!(app.history.len(), 1);
}

#[test]
fn manuscript_creation_insert_save_reopen_and_full_reader_preview_use_core() {
    let (ctx, mut app) = app();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("worldedit-manuscript-e2e-{unique}"));
    app.project = Project::new(&root);
    app.active_file = app.project.entry.clone();
    let entry = app.project.entry.clone();
    app.project.documents.retain(|path, _| path == &entry);
    app.project
        .set_text(
            &app.active_file.clone(),
            "event opening as \"开篇\"\n  海雾散开。\n  -> END\n".into(),
        )
        .unwrap();
    app.project
        .create_authoring_document(
            &root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.10","entry":"world.wl","required_features":["presentation.manuscripts.v1"],"maps":{},"graph_views":{},"manuscripts":{}}"#.to_vec(),
        )
        .unwrap();
    app.reset_views();
    app.recompile();

    enter_text_at_placeholder(&ctx, &mut app, "例如 novel", "novel");
    enter_text_at_placeholder(&ctx, &mut app, "书稿名称", "雾港序章");
    click(&ctx, &mut app, 13, "创建并打开书稿");
    assert!(app.project.manuscript_indices().contains_key("novel"));
    assert_eq!(app.history.len(), 1);

    click(&ctx, &mut app, 13, "插入分节");
    click(&ctx, &mut app, 13, "插入章节");
    click(&ctx, &mut app, 13, "应用书稿");
    let chapter = &app
        .project
        .manuscript_index("novel")
        .unwrap()
        .page(0, 10)
        .chapters[0];
    assert_eq!(chapter.target_ref.as_ref().unwrap().id, "opening");
    assert_eq!(chapter.section_path, ["section"]);
    assert_eq!(
        chapter.source.as_ref().unwrap().status,
        worldline_core::ManuscriptReferenceStatus::Resolved
    );
    assert_eq!(app.history.len(), 2);

    app.project.save().unwrap();
    app.project = Project::open(&root).unwrap();
    app.reset_views();
    app.recompile();
    let reopened = app.project.manuscript_index("novel").unwrap();
    assert_eq!(reopened.title.as_deref(), Some("雾港序章"));
    assert_eq!(reopened.page(0, 10).chapters[0].id, "chapter");
    assert_eq!(reopened.page(0, 10).chapters[0].section_path, ["section"]);
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("海雾散开"), "{rendered}");
}

#[test]
fn sidebar_manuscript_tab_remains_reachable_in_short_viewport() {
    let (ctx, mut app) = app();
    let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
    let visible = |output: &egui::FullOutput, label: &str| {
        visible_text_position(output, label).filter(|point| screen.contains(*point))
    };
    let mut output = frame(&ctx, &mut app, Vec::new(), 32);
    assert!(visible(&output, "时间线").is_some());

    for _ in 0..16 {
        if visible(&output, "书稿工作台").is_some() {
            break;
        }
        output = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos2(120.0, 350.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -90.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            32,
        );
    }
    assert!(
        visible(&output, "书稿工作台").is_some(),
        "scrolling the sidebar must reveal the manuscript tab"
    );
    click(&ctx, &mut app, 32, "书稿工作台");
    assert_eq!(app.tab, super::Tab::Manuscript);

    for _ in 0..16 {
        if visible(&output, "事件关系图").is_some() {
            break;
        }
        output = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos2(120.0, 350.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, 90.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            32,
        );
    }
    assert!(
        visible(&output, "事件关系图").is_some(),
        "scrolling back must preserve access to the other navigation tabs"
    );
    click(&ctx, &mut app, 32, "事件关系图");
    assert_eq!(app.tab, super::Tab::Graph);
}
