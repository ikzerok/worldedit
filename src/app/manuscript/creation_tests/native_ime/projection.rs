use super::*;

#[test]
fn incomplete_worldline_preedit_keeps_the_editor_until_chinese_commit() {
    for empty in [false, true] {
        let (ctx, mut app) = blank();
        if empty {
            create_start(&ctx, &mut app);
            app.personal.settings.focus = true;
            app.manuscript.reader_open = false;
            native_click(&ctx, &mut app, "从这里写下第一段……", None);
            native_frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Enabled)]);
        } else {
            native_body(&ctx, &mut app, true);
        }
        let baseline = app.project.content_baseline();
        let focus = ctx.memory(|memory| memory.focused()).unwrap();
        let preedit = if empty { "if (" } else { "{未完成" };
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Preedit(preedit.into()))],
        );
        assert!(!app.manuscript.writing_buffers[&app.project.entry]
            .source()
            .contains(preedit));
        assert!(app.manuscript.has_unsubmitted_work());
        let waiting = native_settle(&ctx, &mut app);
        assert!(
            labels(&waiting).contains(preedit),
            "candidate must remain visible"
        );
        assert!(
            ctx.memory(|memory| memory.has_focus(focus)),
            "empty={empty}: editor disappeared"
        );
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Commit("如果完成".into()))],
        );
        let source = app.manuscript.writing_buffers[&app.project.entry].source();
        assert!(source.contains("如果完成"), "empty={empty}: {source}");
        assert!(!source.contains(preedit));
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(app.history.len(), 1);
    }
}

#[test]
fn active_body_composition_cannot_open_global_navigation_search_or_appearance() {
    for button in ["外观", "▶ 试玩", "搜索", "查找对象 / 命令    Ctrl/Cmd+P"] {
        let (ctx, mut app) = blank();
        native_body(&ctx, &mut app, true);
        let focus = ctx.memory(|memory| memory.focused()).unwrap();
        let baseline = app.project.content_baseline();
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Preedit("未提交候选".into()))],
        );
        native_click(&ctx, &mut app, button, None);
        assert_eq!(app.tab, Tab::Manuscript, "{button}");
        assert!(
            !app.personal.preferences_open && !app.search_open && !app.command_palette.open,
            "{button}"
        );
        assert!(ctx.memory(|memory| memory.has_focus(focus)), "{button}");
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Commit("提交完成".into()))],
        );
        assert!(
            app.manuscript.writing_buffers[&app.project.entry]
                .source()
                .contains("提交完成"),
            "{button}"
        );
        assert_eq!(app.project.content_baseline(), baseline);
        native_click(&ctx, &mut app, button, None);
        assert!(
            match button {
                "外观" => app.personal.preferences_open,
                "搜索" => app.search_open,
                "查找对象 / 命令    Ctrl/Cmd+P" => app.command_palette.open,
                _ => app.play_confirmation.is_some() || app.tab == Tab::Play,
            },
            "next intentional click must work: {button}"
        );
    }
}

#[test]
fn source_changes_during_composition_keep_the_complete_commit_without_overwriting_new_text() {
    for replacement in ["外部新稿。", "# 外部插入注释\n  外部新稿。", "if ("] {
        let (ctx, mut app) = blank();
        native_body(&ctx, &mut app, true);
        let baseline = app.project.content_baseline();
        let focus = ctx.memory(|memory| memory.focused()).unwrap();
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Preedit("未完候选".into()))],
        );
        let path = app.project.entry.clone();
        let changed = app.manuscript.writing_buffers[&path]
            .source()
            .replace("剪贴板正文。", replacement);
        app.manuscript
            .writing_buffers
            .get_mut(&path)
            .unwrap()
            .replace_source(changed.clone());
        native_settle(&ctx, &mut app);
        assert!(
            ctx.memory(|memory| memory.has_focus(focus)),
            "{replacement}"
        );
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Commit("最终合成".into()))],
        );
        assert_eq!(app.manuscript.writing_buffers[&path].source(), changed);
        let retained = app
            .manuscript
            .writing_view
            .retained_runtime_drafts(&app.project.root);
        assert!(
            retained
                .values()
                .any(|text| text.contains("剪贴板正文。最终合成")),
            "{replacement}: {retained:?}"
        );
        assert!(!retained.values().any(|text| text.contains("未完候选")));
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(app.history.len(), 1);
        let output = native_settle(&ctx, &mut app);
        assert!(labels(&output).contains("保留输入"));
        if replacement == "外部新稿。" {
            native_frame(&ctx, &mut app, vec![Event::Text("续".into())]);
            let fresh = app.manuscript.writing_buffers[&path].source();
            assert_eq!(
                fresh.replace("续", ""),
                changed,
                "rejected retained must not replace the current text"
            );
            assert!(app
                .manuscript
                .writing_view
                .retained_runtime_drafts(&app.project.root)
                .values()
                .any(|text| text.contains("剪贴板正文。最终合成")));
        }
    }
}

#[test]
fn a_global_press_blocked_by_composition_does_not_open_appearance_after_commit_release() {
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
        vec![Event::Ime(egui::ImeEvent::Commit("提交".into()))],
    );
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
    assert!(app.personal.preferences_open);
}

#[test]
fn composition_only_input_survives_arrangement_removal_and_receives_commit_on_the_start_page() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    app.personal.settings.focus = true;
    app.manuscript.reader_open = false;
    native_click(&ctx, &mut app, "从这里写下第一段……", None);
    native_frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Enabled)]);
    native_frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Preedit("if (".into()))],
    );
    let focus = ctx.memory(|memory| memory.focused()).unwrap();
    assert!(!app.manuscript.writing_buffers[&app.project.entry].is_changed());
    assert!(app.manuscript.has_unsubmitted_work());
    // 模拟已发生的外部编排事务；正常全局撤销手势在组合期间由 guard 拒绝。
    app.undo(false);
    let baseline = app.project.content_baseline();
    assert!(app.project.manuscript_indices().is_empty());
    native_settle(&ctx, &mut app);
    assert!(ctx.memory(|memory| memory.has_focus(focus)));
    native_frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Commit("首章完整提交".into()))],
    );
    let drafts = app.manuscript.runtime_drafts(&app.project.root);
    assert!(
        drafts.values().any(|text| text.contains("首章完整提交")),
        "{drafts:?}"
    );
    assert!(!drafts.values().any(|text| text.contains("if (")));
    assert!(labels(&native_settle(&ctx, &mut app)).contains("首章完整提交"));
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn save_and_escape_during_real_composition_do_not_apply_or_save_temporary_prose() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    app.personal.settings.focus = true;
    app.manuscript.reader_open = false;
    native_click(&ctx, &mut app, "从这里写下第一段……", None);
    native_frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Enabled)]);
    let baseline = app.project.content_baseline();
    let source = app.project.document(&app.project.entry).unwrap().to_owned();
    assert!(!app.project.root.exists());
    native_frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Preedit("if (".into()))],
    );
    for (key, modifiers) in [
        (egui::Key::S, egui::Modifiers::COMMAND),
        (egui::Key::Escape, egui::Modifiers::NONE),
    ] {
        for pressed in [true, false] {
            native_frame(
                &ctx,
                &mut app,
                vec![Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed,
                    repeat: false,
                    modifiers,
                }],
            );
        }
        assert_eq!(app.tab, Tab::Manuscript);
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(
            app.manuscript.writing_buffers[&app.project.entry].source(),
            source
        );
        assert!(!app.project.root.exists());
    }
    native_frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Commit("如果保存".into()))],
    );
    assert!(app.manuscript.writing_buffers[&app.project.entry]
        .source()
        .contains("如果保存"));
    assert_eq!(app.project.document(&app.project.entry).unwrap(), source);
    assert!(!app.project.root.exists());
}

#[test]
fn disabled_without_commit_keeps_preedit_recoverable_instead_of_compiling_it_as_prose() {
    let (ctx, mut app) = blank();
    native_body(&ctx, &mut app, true);
    let original = app.manuscript.writing_buffers[&app.project.entry]
        .source()
        .to_owned();
    native_frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Preedit("if (".into()))],
    );
    native_frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Disabled)]);
    let output = native_settle(&ctx, &mut app);
    assert_eq!(
        app.manuscript.writing_buffers[&app.project.entry].source(),
        original
    );
    assert!(app.manuscript.has_unsubmitted_work());
    assert!(labels(&output).contains("未收到提交"));
    assert!(app
        .manuscript
        .runtime_drafts(&app.project.root)
        .values()
        .any(|text| text.contains("if (")));
}
