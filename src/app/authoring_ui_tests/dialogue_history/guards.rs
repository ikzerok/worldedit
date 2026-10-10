use super::*;

fn character_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1700.0, 1400.0))),
            events,
            ..Default::default()
        },
        |ctx| app.characters_tab(ctx),
    )
}
fn rename_display(ctx: &egui::Context, app: &mut WorldeditApp, id: &str) {
    app.tab = Tab::Characters;
    app.select_character(id);
    app.character_focus.inspector_open = true;
    for _ in 0..3 {
        character_frame(ctx, app, vec![]);
    }
    let old = app.character_editor.as_ref().unwrap().draft.display.clone();
    focus_replace(ctx, egui::Id::new("character-name-input"), &old);
    character_frame(ctx, app, vec![Event::Text("更新显示名😀".into())]);
    assert_eq!(
        app.character_editor.as_ref().unwrap().draft.display,
        "更新显示名😀"
    );
    let before = app.history.len();
    let output = character_frame(ctx, app, vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|s| clipped_text_position(&s.shape, "应用人物档案", s.clip_rect))
        .unwrap_or_else(|| panic!("{}", labels(&output)));
    for pressed in [true, false] {
        character_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(app.history.len(), before + 1, "{:?}", app.io_error);
    assert_eq!(
        app.snapshot
            .as_ref()
            .unwrap()
            .result
            .analysis
            .symbols
            .characters[id]
            .display,
        "更新显示名😀"
    );
}

#[test]
fn dialogue_history_character_same_file_rejects_and_cross_file_handoff_travels() {
    // H05/H06/G03：人物表单真改 source，只有完整 original 未变的 B 可移交。
    for cross_file in [false, true] {
        for role_first in [false, true] {
            let (ctx, mut app, _cleanup) = fixture();
            let path = app.project.entry.clone();
            let original = project_files(&app.project);
            let disk_before = disk(&app);
            let id = if cross_file { "remote" } else { "traveler" };
            if role_first {
                rename_display(&ctx, &mut app, id);
            }
            stage(&ctx, &mut app, "原来的正式对白", "人物事务旁的台词😀");
            let staged = buffer(&app, &path);
            if !role_first {
                rename_display(&ctx, &mut app, id);
            }
            if !cross_file && !role_first {
                assert_buffer(&app, &staged, false);
                assert!(buffer(&app, &path)
                    .rebase_unchanged_source(&app.project)
                    .is_err());
                let before = project_files(&app.project);
                let history = (counts(&app), app.history_state.current);
                open_body(&ctx, &mut app);
                click(&ctx, &mut app, 13, "应用正文草稿");
                assert!(app.io_error.is_some());
                assert_eq!(project_files(&app.project), before);
                assert_eq!((counts(&app), app.history_state.current), history);
                assert_buffer(&app, &staged, false);
                assert_eq!(disk(&app), disk_before);
                continue;
            }
            assert_buffer(&app, &staged, true);
            apply_body(&ctx, &mut app);
            let applied = project_files(&app.project);
            assert_eq!(counts(&app), (2, 0, 1, 0));
            for _ in 0..3 {
                app.edit_undo(false);
                assert!(app.io_error.is_none(), "{:?}", app.io_error);
                open_body(&ctx, &mut app);
            }
            assert_eq!(project_files(&app.project), original);
            assert_eq!(counts(&app), (0, 2, 0, 1));
            for _ in 0..3 {
                app.edit_undo(true);
                assert!(app.io_error.is_none(), "{:?}", app.io_error);
                open_body(&ctx, &mut app);
            }
            assert_eq!(project_files(&app.project), applied);
            assert_eq!(disk(&app), disk_before);
            save_reopen(&mut app);
        }
    }
}

#[test]
fn dialogue_history_stable_id_invalidates_changed_source_only() {
    // H07/G03：稳定 ID 应用真改 source；同源两章共用旧 B，独立文件仍能应用。
    let (ctx, mut app, _cleanup) = fixture();
    let path = app.project.entry.clone();
    let remote = app.project.root.join("remote.wl");
    let disk_before = disk(&app);
    let mut other = app.project.open_source_writing_buffer(&remote).unwrap();
    other.replace_source(format!("{}// 独立来源尚可提交😀\n", other.source()));
    app.manuscript.restore_writing_buffers(&[other.clone()]);
    stage(&ctx, &mut app, "原来的正式对白", "不能覆盖新 ID 的台词😀");
    let staged = buffer(&app, &path);
    click(&ctx, &mut app, 13, "离港");
    click(&ctx, &mut app, 13, "抵达");
    assert_buffer(&app, &staged, true);
    stable_id(&ctx, &mut app);
    assert_buffer(&app, &staged, false);
    assert_buffer(&app, &other, true);
    let before = project_files(&app.project);
    let history = (counts(&app), app.history_state.current);
    open_body(&ctx, &mut app);
    let stale = frame(&ctx, &mut app, vec![], 13);
    assert!(labels(&stale).contains("STALE_DRAFT"));
    assert!(stale.shapes.iter().all(|shape| clipped_text_position(
        &shape.shape,
        "离港",
        shape.clip_rect
    )
    .is_none()));
    click(&ctx, &mut app, 13, "应用正文草稿");
    assert!(app.io_error.is_some());
    assert_eq!(project_files(&app.project), before);
    assert_eq!((counts(&app), app.history_state.current), history);
    assert_buffer(&app, &staged, false);
    assert_eq!(disk(&app), disk_before);
    search_remote_draft(&ctx, &mut app);
    assert_eq!(project_files(&app.project), before);
    assert_eq!((counts(&app), app.history_state.current), history);
    assert_buffer(&app, &staged, false);
    assert_buffer(&app, &other, true);
    click(&ctx, &mut app, 13, "源码");
    click(&ctx, &mut app, 13, "应用源码草稿（可含诊断）");
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.project.document(&remote).unwrap(), other.source());
    assert_eq!(
        app.project.document(&path).unwrap().as_bytes(),
        before[&path]
    );
    assert_buffer(&app, &staged, false);
    save_reopen(&mut app);
}

#[test]
fn dialogue_history_newer_source_generation_rejects_undo_and_identical_byte_redo() {
    // H13：较新输入即使恢复为相同字节，也不能被旧 redo 当成原草稿消费。
    for forward in [false, true] {
        let (ctx, mut app, _cleanup) = fixture();
        stage(&ctx, &mut app, "原来的正式对白", "第一份台词😀");
        apply_body(&ctx, &mut app);
        if forward {
            app.edit_undo(false);
            assert!(app.io_error.is_none());
        }
        open_body(&ctx, &mut app);
        click(&ctx, &mut app, 13, "源码");
        let old = buffer(&app, &app.project.entry);
        replace_source(&ctx, &mut app, &format!("{}// 新一代输入\n", old.source()));
        if forward {
            replace_source(&ctx, &mut app, old.source());
        }
        let newer = buffer(&app, &app.project.entry);
        assert!(newer.generation() > old.generation());
        let before = project_files(&app.project);
        let disk_before = disk(&app);
        let history = (counts(&app), app.history_state.current);
        for _ in 0..2 {
            app.edit_undo(forward);
            assert!(app.io_error.is_some());
            assert_eq!((counts(&app), app.history_state.current), history);
            assert_eq!(project_files(&app.project), before);
            assert_eq!(disk(&app), disk_before);
            assert_buffer(&app, &newer, false);
        }
    }
}

#[test]
fn dialogue_history_unstaged_fields_block_history_apply_and_close_without_consuming() {
    // H13/N07/N09：F 不是空白 B；拒绝 Undo/Redo/Apply/Close 时保留真实字段。
    for forward in [false, true] {
        let (ctx, mut app, _cleanup) = fixture();
        stage(&ctx, &mut app, "原来的正式对白", "已纳入台词😀");
        apply_body(&ctx, &mut app);
        if forward {
            app.edit_undo(false);
            assert!(app.io_error.is_none());
        }
        open_body(&ctx, &mut app);
        click(&ctx, &mut app, 13, "编辑此句");
        replace_text_area(&ctx, &mut app, 13, "已纳入台词😀", "保留未纳入的字段😀");
        let keep = buffer(&app, &app.project.entry);
        let retained = app.manuscript.runtime_drafts(&app.project.root);
        assert!(retained
            .values()
            .any(|input| input.contains("保留未纳入的字段😀")));
        let before = project_files(&app.project);
        let disk_before = disk(&app);
        let history = (counts(&app), app.history_state.current);
        assert!(app.manuscript.has_dialogue_input());
        assert!(!app.dirty_draft_names().is_empty());
        app.edit_undo(forward);
        assert!(app.io_error.is_some());
        click(&ctx, &mut app, 13, "应用正文草稿");
        app.request_action(Pending::Close, &ctx);
        assert!(!app.allow_close);
        assert!(app.draft_action.is_some());
        assert_eq!((counts(&app), app.history_state.current), history);
        assert_eq!(project_files(&app.project), before);
        assert_eq!(disk(&app), disk_before);
        assert_buffer(&app, &keep, false);
        assert_eq!(app.manuscript.runtime_drafts(&app.project.root), retained);
        assert!(app.manuscript.has_dialogue_input());
        let rendered = labels(&frame(&ctx, &mut app, vec![], 13));
        assert!(rendered.contains("保留未纳入的字段😀"), "{rendered}");
    }
}
