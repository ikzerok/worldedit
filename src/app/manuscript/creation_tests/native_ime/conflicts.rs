use super::*;

fn focus_mode_input(ctx: &egui::Context, app: &mut WorldeditApp, mode: &str) {
    if mode != "写作" {
        native_click(ctx, app, mode, None);
    }
    native_settle(ctx, app);
    let (target, path) = app.manuscript.active_writing_target().unwrap();
    let id = match mode {
        "结构" => egui::Id::new(("writing-structure", &path, &target.kind, &target.id)),
        "源码" => egui::Id::new(("writing-source", &path, &target.kind, &target.id)),
        _ => {
            let projection = app
                .project
                .project_writing_buffer(&app.manuscript.writing_buffers[&path], &target)
                .unwrap();
            let offset = projection
                .empty_prose_slot
                .as_ref()
                .map(|slot| slot.offset())
                .or_else(|| {
                    projection
                        .blocks
                        .iter()
                        .find(|block| {
                            block.kind == worldline_core::manuscript::WritingBlockKind::Prose
                        })
                        .map(|block| block.range.start)
                })
                .unwrap();
            egui::Id::new(("writing-prose", &path, &target.kind, &target.id, offset))
        }
    };
    let rect = ctx.read_response(id).unwrap().rect;
    let pos = rect.min + vec2(16.0, 8.0);
    for pressed in [true, false] {
        native_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert!(ctx.memory(|memory| memory.has_focus(id)), "{mode}");
}

#[test]
fn all_writing_modes_keep_failed_commit_separate_from_later_typing_and_new_composition() {
    for mode in ["写作", "结构", "源码"] {
        let (ctx, mut app) = blank();
        native_body(&ctx, &mut app, true);
        focus_mode_input(&ctx, &mut app, mode);
        native_frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Enabled)]);
        let path = app.project.entry.clone();
        let old = app.manuscript.writing_buffers[&path].source().to_owned();
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Preedit("第一候选".into()))],
        );
        assert_eq!(app.manuscript.writing_buffers[&path].source(), old);
        assert!(
            app.manuscript
                .runtime_drafts(&app.project.root)
                .values()
                .any(|text| text.contains("第一候选")),
            "{mode}: preedit must be accepted"
        );
        let changed = old.replace("剪贴板正文。", "外部新稿。");
        app.manuscript
            .writing_buffers
            .get_mut(&path)
            .unwrap()
            .replace_source(changed.clone());
        native_settle(&ctx, &mut app);
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Commit("第一次提交".into()))],
        );
        assert_eq!(app.manuscript.writing_buffers[&path].source(), changed);
        native_settle(&ctx, &mut app);
        native_frame(&ctx, &mut app, vec![Event::Text("续".into())]);
        assert_eq!(
            app.manuscript.writing_buffers[&path]
                .source()
                .replace("续", ""),
            changed,
            "{mode}"
        );
        assert!(
            app.manuscript
                .runtime_drafts(&app.project.root)
                .values()
                .any(|text| text.contains("第一次提交")),
            "{mode}"
        );
        if mode == "写作" {
            native_frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Enabled)]);
            native_frame(
                &ctx,
                &mut app,
                vec![Event::Ime(egui::ImeEvent::Preedit("第二候选".into()))],
            );
            let newer = format!(
                "{}\n// 第二次外改\n",
                app.manuscript.writing_buffers[&path].source()
            );
            app.manuscript
                .writing_buffers
                .get_mut(&path)
                .unwrap()
                .replace_source(newer.clone());
            native_settle(&ctx, &mut app);
            native_frame(
                &ctx,
                &mut app,
                vec![Event::Ime(egui::ImeEvent::Commit("第二次提交".into()))],
            );
            let drafts = app
                .manuscript
                .writing_view
                .retained_runtime_drafts(&app.project.root);
            assert!(drafts.values().any(|text| text.contains("第一次提交")));
            assert!(drafts.values().any(|text| text.contains("第二次提交")));
            assert_eq!(app.manuscript.writing_buffers[&path].source(), newer);
        }
    }
}

#[test]
fn another_chapter_target_or_readonly_change_does_not_remove_the_pending_editor() {
    for change in ["delete", "retarget", "readonly"] {
        for mode in ["写作", "结构", "源码"] {
            let (ctx, mut app) = blank();
            retention::two_chapters(&ctx, &mut app, false);
            click(&ctx, &mut app, "潮汐初起");
            app.personal.settings.focus = true;
            app.manuscript.reader_open = false;
            native_settle(&ctx, &mut app);
            focus_mode_input(&ctx, &mut app, mode);
            native_frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Enabled)]);
            let focus = ctx.memory(|memory| memory.focused()).unwrap();
            native_frame(
                &ctx,
                &mut app,
                vec![Event::Ime(egui::ImeEvent::Preedit("A待提交".into()))],
            );
            let source_before = app.project.sources();
            let index = app.project.manuscript_index("book").unwrap();
            let path = app
                .project
                .authoring_documents
                .iter()
                .find(|(_, document)| document.bytes() == index.source_bytes())
                .map(|(path, _)| path.clone())
                .unwrap();
            let mut document: serde_json::Value =
                serde_json::from_slice(app.project.authoring_document(&path).unwrap().bytes())
                    .unwrap();
            match change {
                "delete" => {
                    document["entries"].as_array_mut().unwrap().remove(0);
                }
                "retarget" => {
                    document["entries"][0]["target_ref"] =
                        document["entries"][1]["target_ref"].clone();
                }
                _ => {
                    document["schema_version"] = serde_json::json!(99);
                }
            }
            if change == "readonly" {
                // 不让作者 API 写入不支持的格式；模拟已保存文件的真实外部变化。
                std::fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
                assert!(app.project.refresh().unwrap().is_empty());
                assert!(app.project.manuscript_index("book").unwrap().read_only);
            } else {
                app.project
                    .set_authoring_document(&path, serde_json::to_vec(&document).unwrap())
                    .unwrap();
            }
            app.recompile();
            let baseline = app.project.content_baseline();
            // 不预热新界面：外变与 Commit 紧邻，不能借测量空帧隐藏丢输入。
            native_frame(
                &ctx,
                &mut app,
                vec![Event::Ime(egui::ImeEvent::Commit("A完整提交".into()))],
            );
            assert!(
                ctx.memory(|memory| memory.has_focus(focus)),
                "{change}/{mode}"
            );
            let drafts = app.manuscript.runtime_drafts(&app.project.root);
            assert!(
                drafts.values().any(|text| text.contains("A完整提交")),
                "{change}/{mode}: {drafts:?}"
            );
            assert!(
                !drafts.values().any(|text| text.contains("A待提交")),
                "{change}/{mode}: {drafts:?}"
            );
            assert_eq!(app.project.content_baseline(), baseline);
            assert_eq!(app.project.sources(), source_before);
            assert!(app
                .manuscript
                .writing_buffers
                .values()
                .all(|buffer| !buffer.source().contains("A完整提交")));
            std::fs::remove_dir_all(&app.project.root).unwrap();
        }
    }
}

#[test]
fn focus_loss_or_touch_cancel_finishes_a_blocked_gesture_without_swallowing_the_next_click() {
    for touch in [false, true] {
        let (ctx, mut app) = blank();
        native_body(&ctx, &mut app, true);
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Preedit("候选".into()))],
        );
        let pos = native_position(&ctx, &mut app, "外观");
        native_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        native_frame(
            &ctx,
            &mut app,
            vec![if touch {
                Event::Touch {
                    device_id: egui::TouchDeviceId(1),
                    id: egui::TouchId(1),
                    phase: egui::TouchPhase::Cancel,
                    pos,
                    force: None,
                }
            } else {
                Event::WindowFocused(false)
            }],
        );
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Commit("提交".into()))],
        );
        native_frame(
            &ctx,
            &mut app,
            vec![
                Event::Ime(egui::ImeEvent::Disabled),
                Event::WindowFocused(true),
            ],
        );
        // 即使返焦才补发旧 release，也不能复活原手势。
        native_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(!app.personal.preferences_open);
        native_click(&ctx, &mut app, "外观", None);
        assert!(app.personal.preferences_open, "touch={touch}");
    }
}

#[test]
fn empty_slot_conflict_is_not_retried_as_old_text_and_second_failure_keeps_both_attempts() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    app.project.save().unwrap();
    app.personal.settings.focus = true;
    app.manuscript.reader_open = false;
    native_click(&ctx, &mut app, "从这里写下第一段……", None);
    native_frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Enabled)]);
    native_frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Preedit("空候选".into()))],
    );
    let path = app.project.entry.clone();
    let disk = std::fs::read(&path).unwrap();
    let changed = format!(
        "{}\n// 外部尾注\n",
        app.manuscript.writing_buffers[&path].source()
    );
    app.manuscript
        .writing_buffers
        .get_mut(&path)
        .unwrap()
        .replace_source(changed.clone());
    native_frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Commit("空槽完整提交".into()))],
    );
    assert_eq!(app.manuscript.writing_buffers[&path].source(), changed);
    native_settle(&ctx, &mut app);
    std::fs::write(&path, [disk.as_slice(), b"\n// disk changed\n"].concat()).unwrap();
    native_frame(&ctx, &mut app, vec![Event::Text("新输入".into())]);
    let retained = app
        .manuscript
        .writing_view
        .retained_runtime_drafts(&app.project.root);
    assert!(
        retained.values().any(|text| text.contains("空槽完整提交")),
        "{retained:?}"
    );
    assert!(
        retained.values().any(|text| text.contains("新输入")),
        "{retained:?}"
    );
    assert_eq!(app.manuscript.writing_buffers[&path].source(), changed);
    std::fs::write(&path, &disk).unwrap();
    native_frame(&ctx, &mut app, vec![Event::Text("续".into())]);
    let body = app.manuscript.writing_buffers[&path].source();
    assert!(body.contains("续"));
    assert!(!body.contains("空槽完整提交") && !body.contains("新输入"));
    let retained = app
        .manuscript
        .writing_view
        .retained_runtime_drafts(&app.project.root);
    assert!(retained.values().any(|text| text.contains("空槽完整提交")));
    assert!(retained.values().any(|text| text.contains("新输入")));
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn a_plain_empty_slot_disk_failure_never_auto_refills_or_clears_the_failed_text() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    app.project.save().unwrap();
    app.personal.settings.focus = true;
    app.manuscript.reader_open = false;
    native_click(&ctx, &mut app, "从这里写下第一段……", None);
    let path = app.project.entry.clone();
    let original = std::fs::read(&path).unwrap();
    std::fs::write(
        &path,
        [original.as_slice(), b"\n// outside edit\n"].concat(),
    )
    .unwrap();
    native_frame(&ctx, &mut app, vec![Event::Text("失败甲稿".into())]);
    assert!(!app.manuscript.writing_buffers[&path].is_changed());
    std::fs::write(&path, &original).unwrap();
    native_frame(&ctx, &mut app, vec![Event::Text("新乙稿".into())]);
    let source = app.manuscript.writing_buffers[&path].source();
    assert!(source.contains("新乙稿"));
    assert!(!source.contains("失败甲稿"));
    assert!(app
        .manuscript
        .writing_view
        .retained_runtime_drafts(&app.project.root)
        .values()
        .any(|text| text.contains("失败甲稿")));
    assert_eq!(app.project.document(&path).unwrap().as_bytes(), original);
    std::fs::remove_dir_all(&app.project.root).unwrap();
}
