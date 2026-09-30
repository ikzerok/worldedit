use super::*;

fn rendered(output: &egui::FullOutput) -> String {
    let mut text = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut text);
    }
    text
}

#[test]
fn manuscript_opens_selected_body_and_cards_show_author_metadata() {
    let (ctx, mut app) = manuscript_app();
    let baseline = app.project.content_baseline();
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let text = rendered(&output);
    assert!(text.contains("甲乙"), "{text}");
    assert!(text.contains("写作"), "{text}");
    assert!(
        !text.contains("character traveler as"),
        "默认不展示整文件头部"
    );
    assert!(text.contains("旅人来到港口"));
    assert!(text.contains("8 词（静态）"));
    assert!(text.contains("目标：100"));
    click(&ctx, &mut app, 13, "卡片");
    let text = rendered(&frame(&ctx, &mut app, Vec::new(), 13));
    assert!(text.contains("查看章节"));
    assert!(text.contains("旅人来到港口"));
    assert!(text.contains("draft"));
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn manuscript_reordering_uses_one_core_command_and_undo_keeps_story_semantics() {
    let (ctx, mut app) = manuscript_app();
    let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 13, "编排与来源");
    click(&ctx, &mut app, 13, "下移");
    assert_eq!(app.project.content_baseline(), baseline);
    click(&ctx, &mut app, 13, "应用书稿");
    assert_eq!(
        app.project
            .manuscript_index("novel")
            .unwrap()
            .page(0, 10)
            .chapters[0]
            .id,
        "departure"
    );
    assert_eq!(app.history.len(), 1);
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
    app.undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn manuscript_prose_editor_applies_only_target_and_undo_restores() {
    let (ctx, mut app) = manuscript_app();
    let original = app.project.document(&app.active_file).unwrap().to_owned();
    replace_text_area(
        &ctx,
        &mut app,
        13,
        "甲乙 [[character:traveler|林澈]]",
        "新正文 [[character:traveler|林澈]]",
    );
    assert_eq!(app.project.document(&app.active_file).unwrap(), original);
    click(&ctx, &mut app, 13, "应用正文草稿");
    assert_eq!(
        app.project.document(&app.active_file).unwrap(),
        original.replace("甲乙", "新正文")
    );
    assert_eq!(app.history.len(), 1);
    app.undo(false);
    assert_eq!(app.project.document(&app.active_file).unwrap(), original);
}

#[test]
fn manuscript_full_source_draft_survives_invalid_input_and_applies_explicitly() {
    let (ctx, mut app) = manuscript_app();
    let baseline = app.project.content_baseline();
    let replacement = "event arrival\n  if (\n    尚未完成的正文。\n";
    replace_manuscript_source(&ctx, &mut app, replacement);
    assert_eq!(app.project.content_baseline(), baseline);
    click(&ctx, &mut app, 13, "写作");
    let text = rendered(&frame(&ctx, &mut app, Vec::new(), 13));
    assert!(text.contains("草稿暂不能解析"), "{text}");
    click(&ctx, &mut app, 13, "源码");
    assert!(rendered(&frame(&ctx, &mut app, Vec::new(), 13)).contains("尚未完成"));
    click(&ctx, &mut app, 13, "应用源码草稿（可含诊断）");
    assert_eq!(app.project.document(&app.active_file).unwrap(), replacement);
    assert_eq!(app.history.len(), 1);
    assert!(app.snapshot.as_ref().unwrap().result.has_errors());
    app.undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn stale_manuscript_source_draft_never_overwrites_new_source() {
    let (ctx, mut app) = manuscript_app();
    replace_manuscript_source(&ctx, &mut app, "event arrival\n  草稿保留。\n  -> END\n");
    let externally_changed = "event arrival\n  外部缓冲。\n  -> END\n";
    app.project
        .set_text(&app.active_file.clone(), externally_changed.into())
        .unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 13, "应用源码草稿（可含诊断）");
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
    assert!(rendered(&frame(&ctx, &mut app, Vec::new(), 13)).contains("草稿保留"));
}

#[test]
fn manuscript_chapter_switch_uses_same_file_draft() {
    let (ctx, mut app) = manuscript_app();
    replace_text_area(
        &ctx,
        &mut app,
        13,
        "甲乙 [[character:traveler|林澈]]",
        "仍在草稿的文字",
    );
    click(&ctx, &mut app, 13, "离港");
    assert!(rendered(&frame(&ctx, &mut app, Vec::new(), 13)).contains("远航"));
    click(&ctx, &mut app, 13, "抵达");
    assert!(rendered(&frame(&ctx, &mut app, Vec::new(), 13)).contains("仍在草稿的文字"));
    click(&ctx, &mut app, 13, "丢弃此文件草稿");
    click(&ctx, &mut app, 13, "取消丢弃");
    assert!(rendered(&frame(&ctx, &mut app, Vec::new(), 13)).contains("仍在草稿的文字"));
    click(&ctx, &mut app, 13, "丢弃此文件草稿");
    click(&ctx, &mut app, 13, "确认丢弃正文草稿");
    assert!(!rendered(&frame(&ctx, &mut app, Vec::new(), 13)).contains("仍在草稿的文字"));
    assert!(app.history.is_empty());
}

#[test]
fn manuscript_delete_arrangement_requires_confirmation_and_keeps_source() {
    let (ctx, mut app) = manuscript_app();
    let source = app.project.document(&app.active_file).unwrap().to_owned();
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 13, "编排与来源");
    click(&ctx, &mut app, 13, "删除编排项");
    click(&ctx, &mut app, 13, "取消删除");
    assert_eq!(app.project.content_baseline(), baseline);
    click(&ctx, &mut app, 13, "删除编排项");
    click(&ctx, &mut app, 13, "确认只删除编排");
    click(&ctx, &mut app, 13, "应用书稿");
    assert!(!app
        .project
        .manuscript_index("novel")
        .unwrap()
        .entries
        .iter()
        .any(|entry| entry.id == "opening"));
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    app.undo(false);
    assert!(app
        .project
        .manuscript_index("novel")
        .unwrap()
        .entries
        .iter()
        .any(|entry| entry.id == "opening"));
}

#[test]
fn new_chapter_never_implicitly_selects_first_source() {
    let (ctx, mut app) = manuscript_app();
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 13, "插入章节");
    let text = rendered(&frame(&ctx, &mut app, Vec::new(), 13));
    assert!(text.contains("请选择"), "{text}");
    assert!(text.contains("尚未选择来源"), "{text}");
    click(&ctx, &mut app, 13, "应用书稿");
    assert_eq!(app.project.content_baseline(), baseline);
    click(&ctx, &mut app, 13, "恢复书稿草稿");
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn manuscript_explicit_source_selection_applies_saves_and_reopens() {
    let (ctx, mut app) = manuscript_app();
    click(&ctx, &mut app, 13, "插入章节");
    click(&ctx, &mut app, 13, "请选择");
    enter_text_at_placeholder(&ctx, &mut app, "搜索名称、类型、ID或来源", "arrival");
    click_containing(&ctx, &mut app, 13, "抵达 · 事件:arrival");
    click(&ctx, &mut app, 13, "应用书稿");
    assert!(app
        .project
        .manuscript_index("novel")
        .unwrap()
        .entries
        .iter()
        .any(|entry| entry.id == "chapter"
            && entry
                .target_ref
                .as_ref()
                .is_some_and(|target| target.id == "arrival")));
    app.project.save().unwrap();
    let reopened = Project::open(&app.project.root).unwrap();
    assert!(reopened
        .manuscript_index("novel")
        .unwrap()
        .entries
        .iter()
        .any(|entry| entry.id == "chapter"));
    assert_eq!(
        reopened.document(&reopened.entry).unwrap(),
        app.project.document(&app.active_file).unwrap()
    );
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn manuscript_moves_chapter_to_explicit_section_without_touching_source() {
    let (ctx, mut app) = manuscript_app();
    let source = app.project.document(&app.active_file).unwrap().to_owned();
    let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
    click(&ctx, &mut app, 13, "插入分节");
    click(&ctx, &mut app, 13, "应用书稿");
    click(&ctx, &mut app, 13, "抵达");
    click(&ctx, &mut app, 13, "编排与来源");
    click(&ctx, &mut app, 13, "移动到分节：根目录");
    click(&ctx, &mut app, 13, "新分节 · section");
    click(&ctx, &mut app, 13, "应用书稿");
    let page = app.project.manuscript_index("novel").unwrap().page(0, 100);
    assert_eq!(
        page.chapters
            .iter()
            .find(|entry| entry.id == "opening")
            .unwrap()
            .section_path,
        ["section"]
    );
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
    assert_eq!(app.history.len(), 2);
}

#[test]
fn manuscript_status_filter_is_personal_and_clear_restores_outline() {
    let (ctx, mut app) = manuscript_app();
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 13, "阅读预览");
    enter_text_at_placeholder(&ctx, &mut app, "筛选状态，如 draft", "planned");
    let text = rendered(&frame(&ctx, &mut app, Vec::new(), 13));
    assert!(
        !text.contains("旅人来到港口"),
        "被筛掉章节的卡片摘要不显示：{text}"
    );
    click(&ctx, &mut app, 13, "清除筛选");
    assert!(rendered(&frame(&ctx, &mut app, Vec::new(), 13)).contains("旅人来到港口"));
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn narrow_manuscript_keeps_body_and_metadata_reachable() {
    let (ctx, mut app) = manuscript_app();
    let output = frame(&ctx, &mut app, Vec::new(), 32);
    assert!(visible_text_position(&output, "甲乙").is_some());
    assert!(visible_text_position(&output, "编排与来源").is_some());
    click(&ctx, &mut app, 32, "编排与来源");
    let mut output = frame(&ctx, &mut app, Vec::new(), 32);
    for _ in 0..16 {
        if visible_text_position(&output, "字数目标").is_some() {
            break;
        }
        output = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos2(700.0, 450.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -60.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            32,
        );
    }
    assert!(
        visible_text_position(&output, "状态").is_some(),
        "{}",
        rendered(&output)
    );
    assert!(
        visible_text_position(&output, "字数目标").is_some(),
        "{}",
        rendered(&output)
    );
}

#[test]
fn manuscript_session_records_stable_ids_without_source_or_writes() {
    let (ctx, mut app) = manuscript_app();
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 13, "离港");
    let session = app.manuscript_session();
    assert_eq!(session.manuscript_id.as_deref(), Some("novel"));
    assert_eq!(session.selected_id.as_deref(), Some("departure"));
    app.reset_views();
    app.restore_manuscript_session(session);
    let _ = frame(&ctx, &mut app, Vec::new(), 13);
    assert_eq!(
        app.manuscript_session().selected_id.as_deref(),
        Some("departure")
    );
    assert_eq!(app.project.content_baseline(), baseline);
}
#[test]
fn sidebar_manuscript_and_collapsed_structure_group_remain_reachable() {
    let (ctx, mut app) = app();
    let mut output = frame(&ctx, &mut app, Vec::new(), 32);
    assert!(visible_text_position(&output, "书稿工作台").is_some());
    click(&ctx, &mut app, 32, "书稿工作台");
    assert_eq!(app.tab, super::Tab::Manuscript);
    for _ in 0..20 {
        if visible_text_position(&output, "结构与审阅").is_some() {
            break;
        }
        output = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos2(120.0, 350.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -60.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            32,
        );
    }
    click(&ctx, &mut app, 32, "结构与审阅");
    for _ in 0..20 {
        output = frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos2(120.0, 350.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -60.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            32,
        );
        if visible_text_position(&output, "事件关系图").is_some() {
            break;
        }
    }
    click(&ctx, &mut app, 32, "事件关系图");
    assert_eq!(app.tab, super::Tab::Graph);
}

#[test]
fn long_project_paths_do_not_expand_sidebar_beyond_its_painted_width() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let folder = app
        .project
        .root
        .join(".world/markdown-imports/a_very_long_import_namespace_0123456789abcdef/sources");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("a_very_long_source_file_name_0123456789abcdef.wl"),
        "entity imported kind place\n",
    )
    .unwrap();
    app.project.refresh().unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    for _ in 0..30 {
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1188.0, 848.0))),
                ..Default::default()
            },
            |ctx| {
                app.sidebar(ctx);
                assert!(
                    ctx.available_rect().left() <= 320.0,
                    "长目录不能把侧栏撑出320px绘制范围：{:?}",
                    ctx.available_rect()
                );
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label("主内容");
                });
            },
        );
    }
    // 更高视口中实际水平滚动到长路径末端，不截断或隐藏文件名。
    let mut end_before = None;
    let mut end_after = None;
    for pass in 0..15 {
        let output = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1188.0, 1800.0))),
                events: if let Some((_, y)) = end_before {
                    vec![
                        Event::PointerMoved(pos2(150.0, y)),
                        Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: vec2(-100.0, 0.0),
                            modifiers: egui::Modifiers::NONE,
                        },
                    ]
                } else {
                    Vec::new()
                },
                ..Default::default()
            },
            |ctx| {
                app.sidebar(ctx);
                egui::CentralPanel::default().show(ctx, |_| {});
            },
        );
        for shape in &output.shapes {
            if let egui::Shape::Text(text) = &shape.shape {
                if text.galley.job.text.ends_with("/sources") {
                    let end = text.pos + text.galley.rect.right_center().to_vec2();
                    if pass == 3 {
                        end_before = Some((end.x, end.y));
                    }
                    if pass > 3 && shape.clip_rect.contains(end) {
                        end_after = Some(end.x);
                    }
                }
            }
        }
    }
    assert!(end_before.is_some(), "长目录全文仍应渲染");
    assert!(
        end_after.is_some_and(|end| end < end_before.unwrap().0),
        "水平滚动应让路径末端进入裁剪区"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn manuscript_back_keeps_prose_mode_after_last_focus_was_source() {
    let (ctx, mut app) = manuscript_app();
    app.tab = Tab::Manuscript;
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 13, "源码");
    let id = egui::Id::new(("writing-source", &app.active_file, "event", "arrival"));
    ctx.memory_mut(|m| m.request_focus(id));
    let mut state = egui::TextEdit::load_state(&ctx, id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(3),
        )));
    state.store(&ctx, id);
    let _ = frame(&ctx, &mut app, Vec::new(), 13);
    assert_eq!(
        app.manuscript_session().cursor.as_ref().unwrap().mode,
        "source"
    );
    click(&ctx, &mut app, 13, "写作"); // deliberately do not focus a prose editor
    assert_eq!(
        app.manuscript_session().mode,
        crate::app::writing_workspace::Mode::Prose
    );
    assert!(app.manuscript_session().cursor.is_none());
    let person = app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .object(&TargetRef::new("character", "traveler"))
        .unwrap()
        .clone();
    app.navigate_object(&person);
    assert_eq!(app.tab, Tab::Characters);
    app.author_back(&ctx);
    assert_eq!(app.tab, Tab::Manuscript);
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    let text = rendered(&output);
    assert_eq!(
        app.manuscript_session().selected_id.as_deref(),
        Some("opening")
    );
    assert_eq!(
        app.manuscript_session().mode,
        crate::app::writing_workspace::Mode::Prose
    );
    assert!(!text.contains("character traveler as"), "{text}");
    assert_eq!(app.project.content_baseline(), baseline);
}
#[test]
fn manuscript_session_restores_explicit_source_mode_without_cursor() {
    let (ctx, mut app) = manuscript_app();
    let _ = frame(&ctx, &mut app, Vec::new(), 13);
    let mut session = app.manuscript_session();
    session.mode = crate::app::writing_workspace::Mode::Source;
    session.cursor = None;
    let session = serde_json::from_str(&serde_json::to_string(&session).unwrap()).unwrap();
    app.reset_views();
    app.restore_manuscript_session(session);
    let output = frame(&ctx, &mut app, Vec::new(), 13);
    assert_eq!(
        app.manuscript_session().mode,
        crate::app::writing_workspace::Mode::Source
    );
    assert!(rendered(&output).contains("character traveler as"));
}
