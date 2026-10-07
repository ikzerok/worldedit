use super::*;

const SIZE: egui::Vec2 = vec2(1600.0, 1500.0);

fn metadata_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    events: Vec<Event>,
) -> egui::FullOutput {
    let mut raw = RawInput {
        screen_rect: Some(Rect::from_min_size(
            pos2(0.0, 0.0),
            size / ctx.zoom_factor(),
        )),
        events,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest());
        // end_pass swaps egui's widget lists: sample after update while still
        // inside this pass, rather than reading the previous pass after run.
        for role in ["book", "title", "summary", "status", "goal"] {
            let id = field_id(app, role);
            if let Some(response) = ctx.read_response(id) {
                ctx.data_mut(|data| {
                    data.insert_temp(id.with("guard-frame-enabled"), response.enabled())
                });
            }
        }
    })
}

fn field_enabled(ctx: &egui::Context, id: egui::Id) -> bool {
    ctx.data(|data| data.get_temp::<bool>(id.with("guard-frame-enabled")))
        .unwrap()
}

fn field_id(app: &WorldeditApp, role: &str) -> egui::Id {
    if role == "book" {
        egui::Id::new(("manuscript-book-title", &app.project.root, "book"))
    } else {
        egui::Id::new((
            "manuscript-entry-field",
            &app.project.root,
            "book",
            "chapter",
            role,
        ))
    }
}

fn field_value<'a>(app: &'a WorldeditApp, role: &str) -> &'a str {
    let draft = &app.manuscript.books["book"].draft;
    let entry = &draft.entries[0];
    match role {
        "book" => &draft.title,
        "title" => &entry.title,
        "summary" => entry.summary.as_deref().unwrap_or_default(),
        "status" => entry.status.as_deref().unwrap_or_default(),
        "goal" => entry.goal.as_deref().unwrap_or_default(),
        _ => unreachable!(),
    }
}

fn focused_field(role: &str) -> (egui::Context, WorldeditApp, egui::Id) {
    let (ctx, mut app) = blank();
    retention::two_chapters(&ctx, &mut app, false);
    click(&ctx, &mut app, "潮汐初起");
    app.personal.settings.reduce_motion = true;
    app.manuscript.reader_open = false;
    app_click(&ctx, &mut app, SIZE, "编排与来源");
    settle_app(&ctx, &mut app, SIZE);
    let id = field_id(&app, role);
    let response = ctx.read_response(id).unwrap();
    assert!(
        response.enabled() && response.interact_rect.contains_rect(response.rect),
        "{role}"
    );
    let position = response.rect.center();
    for pressed in [true, false] {
        metadata_frame(
            &ctx,
            &mut app,
            SIZE,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id), "{role}");
    (ctx, app, id)
}

fn composing_field(role: &str) -> (egui::Context, WorldeditApp, egui::Id) {
    let (ctx, mut app, id) = focused_field(role);
    metadata_frame(
        &ctx,
        &mut app,
        SIZE,
        vec![
            Event::Ime(egui::ImeEvent::Enabled),
            Event::Ime(egui::ImeEvent::Preedit("尚在组合".into())),
        ],
    );
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id), "{role}");
    assert!(field_value(&app, role).contains("尚在组合"), "{role}");
    (ctx, app, id)
}

#[test]
fn unchanged_chapter_preedit_is_not_rebased_away_before_guarded_commit() {
    let (ctx, mut app, id) = focused_field("title");
    let original = field_value(&app, "title").to_owned();
    for pressed in [true, false] {
        metadata_frame(
            &ctx,
            &mut app,
            SIZE,
            vec![Event::Key {
                key: egui::Key::A,
                physical_key: Some(egui::Key::A),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
        );
    }
    metadata_frame(
        &ctx,
        &mut app,
        SIZE,
        vec![
            Event::Ime(egui::ImeEvent::Enabled),
            Event::Ime(egui::ImeEvent::Preedit(original.clone())),
        ],
    );
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    assert_eq!(field_value(&app, "title"), original);
    assert!(
        !app.manuscript.books["book"].changed,
        "等值Preedit应真实保持clean前提"
    );
    refresh_guard(&mut app, "readonly");
    metadata_frame(
        &ctx,
        &mut app,
        SIZE,
        vec![Event::Ime(egui::ImeEvent::Commit(
            "等值后的完整提交".into(),
        ))],
    );
    let entry = app.manuscript.books["book"]
        .draft
        .entries
        .iter()
        .find(|entry| entry.id == "chapter")
        .expect("组合中的原章节不可被clean rebase移除");
    assert_eq!(entry.title, "等值后的完整提交");
}

#[test]
fn guarded_commit_does_not_allow_same_frame_normal_text_or_paste() {
    for guard in ["readonly", "recovery"] {
        let (ctx, mut app, _) = composing_field("book");
        refresh_guard(&mut app, guard);
        metadata_frame(
            &ctx,
            &mut app,
            SIZE,
            vec![
                Event::Ime(egui::ImeEvent::Commit("完整提交".into())),
                Event::Text("旁路键入".into()),
                Event::Paste("旁路粘贴".into()),
            ],
        );
        let value = field_value(&app, "book");
        assert!(value.contains("完整提交"));
        assert!(
            !value.contains("旁路"),
            "{guard}: 只允许完成原组合，不能绕过保护继续写入：{value}"
        );
    }
}

fn refresh_guard(app: &mut WorldeditApp, guard: &str) {
    if guard == "readonly" {
        let path = app.project.root.join(".world/manuscripts/book.json");
        let mut document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        document["schema_version"] = 99.into();
        std::fs::write(path, serde_json::to_vec(&document).unwrap()).unwrap();
    } else {
        // Existing recovery test protocol: the journal expects absence but a
        // real third-party world.wl exists, so core must preserve it as conflict.
        let directory = app
            .project
            .root
            .join(".world/.transactions/metadata-input-conflict");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("journal.json"), br#"{"version":1,"status":"applying","files":[{"path":"world.wl","before":null,"after":null,"payload":null}]}"#).unwrap();
    }
    app.project.refresh().unwrap();
    if guard == "readonly" {
        assert!(app.project.manuscript_index("book").unwrap().read_only);
    } else {
        assert_eq!(
            app.project.recovery_conflicts(),
            std::slice::from_ref(&app.project.entry)
        );
    }
    app.recompile();
}

#[test]
fn metadata_guard_matrix_keeps_commit_but_rejects_later_typing_and_actions() {
    for guard in ["readonly", "recovery"] {
        for role in ["book", "title", "summary", "status", "goal"] {
            let (ctx, mut app, id) = composing_field(role);
            let sources = app.project.sources();
            let source_disk = std::fs::read(&app.project.entry).unwrap();
            let other = app.manuscript.books["book"].draft.entries[1].clone();
            let history = app.history.len();
            refresh_guard(&mut app, guard);
            let path = app.project.root.join(".world/manuscripts/book.json");
            let disk = std::fs::read(&path).unwrap();
            let baseline = app.project.content_baseline();
            // Deliberately no settle/layout frame after the guard changes.
            metadata_frame(
                &ctx,
                &mut app,
                SIZE,
                vec![Event::Ime(egui::ImeEvent::Commit("完整提交".into()))],
            );
            assert_eq!(
                ctx.memory(|memory| memory.focused()),
                Some(id),
                "{guard}/{role}"
            );
            let value = field_value(&app, role);
            assert_eq!(
                value.matches("完整提交").count(),
                1,
                "{guard}/{role}: {value}"
            );
            assert!(!value.contains("尚在组合"), "{guard}/{role}: {value}");
            assert_eq!(app.manuscript.books["book"].draft.entries[1], other);
            let retained = app.manuscript.books["book"].draft.clone();

            metadata_frame(
                &ctx,
                &mut app,
                SIZE,
                vec![
                    Event::Text("不得续写".into()),
                    Event::Paste("不得粘贴".into()),
                ],
            );
            assert_eq!(
                app.manuscript.books["book"].draft, retained,
                "{guard}/{role}: Commit后的接收例外必须结束"
            );
            for field in ["book", "title", "summary", "status", "goal"] {
                assert!(
                    !field_enabled(&ctx, field_id(&app, field)),
                    "{guard}/{role}/{field}"
                );
            }
            for label in ["下移", "删除编排项", "应用书稿"] {
                app_click(&ctx, &mut app, SIZE, label);
                assert_eq!(
                    app.manuscript.books["book"].draft, retained,
                    "{guard}/{role}/{label}"
                );
            }
            assert_eq!(app.history.len(), history);
            assert_eq!(app.project.sources(), sources);
            assert_eq!(app.project.content_baseline(), baseline);
            assert_eq!(std::fs::read(&path).unwrap(), disk);
            assert_eq!(app.project.authoring_document(&path).unwrap().bytes(), disk);
            assert_eq!(std::fs::read(&app.project.entry).unwrap(), source_disk);
        }
    }
}

#[test]
fn metadata_receiver_survives_guarded_wait_and_disabled_keeps_uncommitted_input() {
    for guard in ["readonly", "recovery"] {
        let (ctx, mut app, id) = composing_field("summary");
        refresh_guard(&mut app, guard);
        let baseline = app.project.content_baseline();
        // The same receiver stays enabled while composition is still active,
        // even when no IME event happens in the intervening redraw.
        metadata_frame(&ctx, &mut app, SIZE, vec![]);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
        assert!(field_enabled(&ctx, id));
        assert!(!field_enabled(&ctx, field_id(&app, "book")));
        metadata_frame(
            &ctx,
            &mut app,
            SIZE,
            vec![Event::Ime(egui::ImeEvent::Preedit("仍可保留的候选".into()))],
        );
        metadata_frame(
            &ctx,
            &mut app,
            SIZE,
            vec![Event::Ime(egui::ImeEvent::Disabled)],
        );
        let retained = app.manuscript.books["book"].draft.clone();
        assert!(field_value(&app, "summary").contains("仍可保留的候选"));
        metadata_frame(
            &ctx,
            &mut app,
            SIZE,
            vec![Event::Text("不能继续写入".into())],
        );
        assert_eq!(app.manuscript.books["book"].draft, retained);
        assert!(!field_enabled(&ctx, id));
        assert_eq!(app.project.content_baseline(), baseline);
    }
}

#[test]
fn readonly_refresh_during_book_title_preedit_preserves_same_frame_commit() {
    let (ctx, mut app) = blank();
    retention::two_chapters(&ctx, &mut app, false);
    click(&ctx, &mut app, "潮汐初起");
    app.personal.settings.reduce_motion = true;
    app.manuscript.reader_open = false;
    let size = vec2(1600.0, 1500.0);
    app_click(&ctx, &mut app, size, "编排与来源");
    settle_app(&ctx, &mut app, size);
    let id = egui::Id::new(("manuscript-book-title", &app.project.root, "book"));
    let response = ctx.read_response(id).unwrap();
    assert!(response.interact_rect.contains_rect(response.rect));
    let position = response.rect.center();
    for pressed in [true, false] {
        metadata_frame(
            &ctx,
            &mut app,
            size,
            vec![
                Event::PointerMoved(position),
                Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    let other = app.manuscript.books["book"].draft.entries[1].clone();
    let sources = app.project.sources();
    metadata_frame(
        &ctx,
        &mut app,
        size,
        vec![
            Event::Ime(egui::ImeEvent::Enabled),
            Event::Ime(egui::ImeEvent::Preedit("尚在组合".into())),
        ],
    );
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    assert!(app.manuscript.books["book"]
        .draft
        .title
        .contains("尚在组合"));

    let path = app.project.root.join(".world/manuscripts/book.json");
    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    document["schema_version"] = 99.into();
    let disk = serde_json::to_vec(&document).unwrap();
    std::fs::write(&path, &disk).unwrap();
    assert!(app.project.refresh().unwrap().is_empty());
    assert!(app.project.manuscript_index("book").unwrap().read_only);
    app.recompile();
    let baseline = app.project.content_baseline();
    // No layout/settle frame between external refresh and the actual Commit.
    metadata_frame(
        &ctx,
        &mut app,
        size,
        vec![Event::Ime(egui::ImeEvent::Commit("完整提交".into()))],
    );
    let local = &app.manuscript.books["book"].draft;
    assert!(
        local.title.contains("完整提交") && !local.title.contains("尚在组合"),
        "Commit 必须完整留在原书名稿：{}",
        local.title
    );
    assert_eq!(local.entries[1], other);
    assert_eq!(app.project.sources(), sources);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(std::fs::read(&path).unwrap(), disk);
    assert_eq!(app.project.authoring_document(&path).unwrap().bytes(), disk);
}
