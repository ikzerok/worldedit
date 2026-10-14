//! 普通 Text 的显式逐句编辑与转换，保留原正文直接写作和共享历史。
use super::*;

fn close_clean_navigation_form(ctx: &egui::Context, app: &mut WorldeditApp) {
    // 目录单击只选章节；显式进入写作，建立已聚焦的无修改首句，再测试行菜单入口。
    click(ctx, app, 13, "写作");
    frame(ctx, app, vec![], 13);
    assert!(!app.manuscript.has_dialogue_input());
    let baseline = app.project.content_baseline();
    let sources = app.project.sources();
    let disk = |app: &WorldeditApp| {
        worldline_core::file_access::workspace_files(&app.project.root)
            .unwrap()
            .into_iter()
            .map(|path| {
                let bytes = std::fs::read(&path).unwrap();
                (path, bytes)
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let before_disk = disk(app);
    let buffers: Vec<_> = app
        .manuscript
        .writing_buffers()
        .iter()
        .map(|buffer| buffer.identity())
        .collect();
    let history = (
        app.history.len(),
        app.redo.len(),
        app.search_state.undo.len(),
        app.search_state.redo.len(),
    );
    click(ctx, app, 13, "取消此句输入");
    assert!(!app.manuscript.has_dialogue_input());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.sources(), sources);
    assert_eq!(disk(app), before_disk);
    assert_eq!(
        app.manuscript
            .writing_buffers()
            .iter()
            .map(|buffer| buffer.identity())
            .collect::<Vec<_>>(),
        buffers
    );
    assert_eq!(
        (
            app.history.len(),
            app.redo.len(),
            app.search_state.undo.len(),
            app.search_state.redo.len()
        ),
        history
    );
}

#[test]
fn dialogue_modes_plain_text_updates_applies_undoes_and_returns_to_direct_body() {
    let (ctx, mut app) = dialogue::fixture();
    click(&ctx, &mut app, 13, "离港");
    close_clean_navigation_form(&ctx, &mut app);
    let path = app.active_file.clone();
    let original = app.project.document(&path).unwrap().to_owned();
    click(&ctx, &mut app, 13, "编辑此句");
    replace_text_area(&ctx, &mut app, 13, "远航。", "普通旁白也能逐句修改。");
    assert!(app.manuscript.has_dialogue_input());
    assert_eq!(app.project.document(&path).unwrap(), original);
    click(&ctx, &mut app, 13, "预览语句变更");
    click(&ctx, &mut app, 13, "纳入正文草稿");
    assert!(!app.manuscript.has_dialogue_input());
    assert!(app
        .manuscript
        .writing_buffers()
        .iter()
        .any(|buffer| buffer.source().contains("普通旁白也能逐句修改。")));
    assert_eq!(app.project.document(&path).unwrap(), original);
    click(&ctx, &mut app, 13, "应用正文草稿");
    assert!(app
        .project
        .document(&path)
        .unwrap()
        .contains("普通旁白也能逐句修改。"));
    app.edit_undo(false);
    assert_eq!(app.project.document(&path).unwrap(), original);
    app.edit_undo(false);
    assert!(app
        .manuscript
        .writing_buffers()
        .iter()
        .all(|buffer| !buffer.source().contains("普通旁白也能逐句修改。")));
    app.edit_undo(true);
    app.edit_undo(true);
    assert!(app
        .project
        .document(&path)
        .unwrap()
        .contains("普通旁白也能逐句修改。"));
    super::dialogue::toggle_mode(&ctx, &mut app);
    replace_text_area(
        &ctx,
        &mut app,
        13,
        "普通旁白也能逐句修改。",
        "退出后直接写普通正文。",
    );
    assert!(!app.manuscript.has_dialogue_input());
    assert!(app
        .manuscript
        .writing_buffers()
        .iter()
        .any(|buffer| buffer.source().contains("退出后直接写普通正文。")));
    click(&ctx, &mut app, 13, "应用正文草稿");
    assert!(app
        .project
        .document(&path)
        .unwrap()
        .contains("退出后直接写普通正文。"));
    std::fs::remove_dir_all(app.project.root).unwrap();
}

#[test]
fn dialogue_modes_plain_text_converts_to_formal_role_and_back_without_a_second_source() {
    let (ctx, mut app) = dialogue::fixture();
    click(&ctx, &mut app, 13, "离港");
    close_clean_navigation_form(&ctx, &mut app);
    let path = app.active_file.clone();
    let original = app.project.document(&path).unwrap().to_owned();
    click(&ctx, &mut app, 13, "语句操作");
    click(&ctx, &mut app, 13, "转为正式台词…");
    click(&ctx, &mut app, 13, "明确选择正式角色");
    click(&ctx, &mut app, 13, "同名 · character:second");
    click(&ctx, &mut app, 13, "预览语句变更");
    click(&ctx, &mut app, 13, "纳入正文草稿");
    assert_eq!(app.project.document(&path).unwrap(), original);
    assert!(app
        .manuscript
        .writing_buffers()
        .iter()
        .any(|buffer| buffer.source().contains("say second \"远航。\"")));
    click(&ctx, &mut app, 13, "应用正文草稿");
    assert!(app
        .project
        .document(&path)
        .unwrap()
        .contains("say second \"远航。\""));
    click(&ctx, &mut app, 13, "语句操作");
    click(&ctx, &mut app, 13, "转为普通旁白…");
    click(&ctx, &mut app, 13, "预览语句变更");
    click(&ctx, &mut app, 13, "纳入正文草稿");
    click(&ctx, &mut app, 13, "应用正文草稿");
    assert!(!app
        .project
        .document(&path)
        .unwrap()
        .contains("say second \"远航。\""));
    // Text writer 使用显式转义前缀；核正式 AST 与准确来源，而非假定原裸行字节。
    let target = TargetRef::new("event", "departure");
    let returned_buffer = app.project.open_writing_buffer(&target).unwrap();
    assert!(
        returned_buffer
            .source()
            .lines()
            .any(|line| line == "  \\远航。"),
        "{}",
        returned_buffer.source()
    );
    let returned = app
        .project
        .project_dialogue_buffer(&returned_buffer, &target)
        .unwrap();
    assert_eq!(returned.statements.len(), 1, "{}", returned_buffer.source());
    let statement = &returned.statements[0];
    assert_eq!(
        statement.kind,
        worldline_core::manuscript::DialogueKind::Text
    );
    assert_eq!(
        statement.draft.parts,
        vec![worldline_core::manuscript::DialoguePart::Literal {
            text: "远航。".into()
        }]
    );
    assert!(statement.draft.speaker.is_none());
    assert!(statement.draft.direction.is_none());
    assert_eq!(
        statement.source.excerpt,
        "\\远航。",
        "{}",
        returned_buffer.source()
    );
    app.edit_undo(false);
    assert!(app
        .project
        .document(&path)
        .unwrap()
        .contains("say second \"远航。\""));
    app.edit_undo(true);
    assert!(!app
        .project
        .document(&path)
        .unwrap()
        .contains("say second \"远航。\""));
    std::fs::remove_dir_all(app.project.root).unwrap();
}

#[test]
fn dialogue_modes_author_back_restores_explicit_submode_and_protected_field_focus() {
    let (ctx, mut app) = dialogue::fixture();
    click(&ctx, &mut app, 13, "编辑此句");
    replace_text_area(&ctx, &mut app, 13, "原来的正式对白", "返回仍保留的字段");
    let owner = ctx.memory(|memory| memory.focused());
    let retained = app.manuscript.runtime_drafts(&app.project.root);
    let original = app.project.document(&app.active_file).unwrap().to_owned();
    let origin = app.author_location(Some(&ctx));
    let session: crate::app::manuscript::ManuscriptSession =
        serde_json::from_value(origin.manuscript.clone()).unwrap();
    assert_eq!(session.dialogue_mode, Some(true));
    app.remember_author_location(origin);
    click(&ctx, &mut app, 13, "源码");
    super::dialogue::toggle_mode(&ctx, &mut app);
    assert_eq!(app.manuscript_session().dialogue_mode, Some(false));
    assert_eq!(app.manuscript.runtime_drafts(&app.project.root), retained);
    app.author_back(&ctx);
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![], 13);
    }
    assert_eq!(app.manuscript_session().dialogue_mode, Some(true));
    assert_eq!(
        app.manuscript_session().mode,
        crate::app::writing_workspace::Mode::Prose
    );
    assert_eq!(ctx.memory(|memory| memory.focused()), owner);
    assert_eq!(app.manuscript.runtime_drafts(&app.project.root), retained);
    assert_eq!(app.project.document(&app.active_file).unwrap(), original);
    frame(&ctx, &mut app, vec![Event::Text("继续编辑".into())], 13);
    assert!(app
        .manuscript
        .runtime_drafts(&app.project.root)
        .values()
        .any(|text| text.contains("继续编辑")));
    std::fs::remove_dir_all(app.project.root).unwrap();
}
