use super::*;
use std::time::{Duration, Instant};

fn poll(ctx: &egui::Context, app: &mut WorldeditApp) {
    app.last_refresh = Instant::now() - Duration::from_secs(2);
    // Runs the real native App::update scan/refresh branch, without creating an OS window.
    app.tab = Tab::Edit;
    let mut native_frame = eframe::Frame::_new_kittest();
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1700.0, 1400.0))),
            ..Default::default()
        },
        |ctx| eframe::App::update(app, ctx, &mut native_frame),
    );
}

#[test]
fn dialogue_history_undo_save_own_poll_keeps_exact_buffer_and_redo() {
    // H01/H11：Project 撤销恢复完整 B；保存/自身轮询不能清除它或 redo。
    let (ctx, mut app, _cleanup) = fixture();
    poll(&ctx, &mut app);
    stage(&ctx, &mut app, "原来的正式对白", "保存后重做😀");
    let staged = buffer(&app, &app.project.entry);
    apply_body(&ctx, &mut app);
    let applied = project_files(&app.project);
    app.edit_undo(false);
    assert_buffer(&app, &staged, true);
    save_reopen(&mut app);
    let before = (
        counts(&app),
        app.history_state.current,
        app.version,
        app.project.content_baseline(),
    );
    let saved = disk(&app);
    poll(&ctx, &mut app);
    assert_eq!(
        (
            counts(&app),
            app.history_state.current,
            app.version,
            app.project.content_baseline()
        ),
        before
    );
    assert_eq!(disk(&app), saved);
    assert_buffer(&app, &staged, true);
    app.edit_undo(true);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(project_files(&app.project), applied);
    save_reopen(&mut app);
}

#[test]
fn dialogue_history_saved_locale_tombstone_poll_keeps_redo_and_staged_dialogue() {
    // H12：两条真实 Project Undo 删除已保存的 locale；missing saved baseline 仍可重做。
    for imported in [false, true] {
        let (ctx, mut app, _cleanup) = fixture();
        poll(&ctx, &mut app);
        let original = project_files(&app.project);
        stage(&ctx, &mut app, "原来的正式对白", "删除译文后重做😀");
        let staged = buffer(&app, &app.project.entry);
        if imported {
            import_locale(&ctx, &mut app);
        } else {
            typed_locale(&ctx, &mut app);
        }
        apply_body(&ctx, &mut app);
        let applied = project_files(&app.project);
        save_reopen(&mut app);
        assert!(sidecar(&app).exists());
        poll(&ctx, &mut app);
        for _ in 0..2 {
            app.edit_undo(false);
            assert!(app.io_error.is_none(), "{:?}", app.io_error);
        }
        assert_eq!(project_files(&app.project), original);
        assert_eq!(counts(&app), (0, 2, 1, 0));
        assert_buffer(&app, &staged, true);
        assert!(app
            .project
            .authoring_document(&sidecar(&app))
            .unwrap()
            .is_deleted());
        save_reopen(&mut app);
        assert!(!sidecar(&app).exists());
        let before = (counts(&app), app.history_state.current, app.version);
        poll(&ctx, &mut app);
        assert_eq!(
            (counts(&app), app.history_state.current, app.version),
            before
        );
        assert_buffer(&app, &staged, true);
        for _ in 0..2 {
            app.edit_undo(true);
            assert!(app.io_error.is_none(), "{:?}", app.io_error);
        }
        assert_eq!(project_files(&app.project), applied);
        assert_translation(&app);
        save_reopen(&mut app);
    }
}

#[test]
fn dialogue_history_real_external_poll_retains_staged_buffer_and_unstaged_fields() {
    // H14/G06/N12：外改另一源文件，B 与尚未 stage 的 F 都不能被 recompile 清掉。
    let (ctx, mut app, _cleanup) = fixture();
    poll(&ctx, &mut app);
    stage(&ctx, &mut app, "原来的正式对白", "已应用台词");
    apply_body(&ctx, &mut app);
    save_reopen(&mut app);
    poll(&ctx, &mut app);
    stage(&ctx, &mut app, "已应用台词", "待应用的台词😀");
    let staged = buffer(&app, &app.project.entry);
    click(&ctx, &mut app, 13, "编辑此句");
    replace_text_area(&ctx, &mut app, 13, "待应用的台词😀", "未纳入的字段😀");
    assert!(app.manuscript.has_dialogue_input());
    let retained = app.manuscript.runtime_drafts(&app.project.root);
    assert!(retained
        .values()
        .any(|input| input.contains("未纳入的字段😀")));
    let remote = app.project.root.join("remote.wl");
    let external = "character remote as \"真正外改\"\n// v034 专用外部字节\n";
    fs::write(&remote, external).unwrap();
    poll(&ctx, &mut app);
    assert_eq!(counts(&app), (0, 0, 0, 0));
    assert_eq!(app.project.document(&remote).unwrap(), external);
    assert_eq!(fs::read_to_string(&remote).unwrap(), external);
    assert_buffer(&app, &staged, false);
    assert_eq!(app.manuscript.runtime_drafts(&app.project.root), retained);
    assert!(app.manuscript.has_dialogue_input());
    open_body(&ctx, &mut app);
    let rendered = labels(&frame(&ctx, &mut app, vec![], 13));
    assert!(rendered.contains("未纳入的字段😀"), "{rendered}");
    let expected: serde_json::Value = serde_json::from_str(
        retained
            .iter()
            .find(|(key, _)| key.starts_with("未插入对白 · "))
            .unwrap()
            .1,
    )
    .unwrap();
    let copied = click_body_output(&ctx, &mut app, "复制这份对白输入");
    let actual = copied
        .platform_output
        .commands
        .iter()
        .find_map(|command| match command {
            egui::OutputCommand::CopyText(text) => {
                Some(serde_json::from_str::<serde_json::Value>(text).unwrap())
            }
            _ => None,
        })
        .expect("真实救援按钮应交付完整可恢复 JSON");
    assert_eq!(actual, expected);
    assert_eq!(app.manuscript.runtime_drafts(&app.project.root), retained);
    assert_buffer(&app, &staged, false);
    let before = project_files(&app.project);
    for forward in [false, true] {
        app.edit_undo(forward);
        assert_eq!(counts(&app), (0, 0, 0, 0));
        assert_eq!(project_files(&app.project), before);
        assert_buffer(&app, &staged, false);
        assert_eq!(app.manuscript.runtime_drafts(&app.project.root), retained);
        assert!(app.manuscript.has_dialogue_input());
    }
}
