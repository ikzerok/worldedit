//! 完整 App 更新与真实 egui 事件；不冒充系统窗口或物理 IME 验收。
use super::*;
use egui::{Event, Key, Modifiers};

fn key(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers,
    }
}

fn update_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    update_frame_observed(ctx, app, events, |_, _| {})
}

fn update_frame_observed(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
    mut inspect: impl FnMut(&egui::Context, &WorldeditApp),
) -> egui::FullOutput {
    let mut raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280.0, 900.0),
        )),
        events,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest());
        inspect(ctx, app);
    })
}

fn preparation_over_inputs() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app();
    app.personal.pending_restore = false;
    app.new_file = Some("events/未提交🌦️.wl".into());
    app.open_search(&ctx, false, true);
    app.project_query = "未应用真实正文".into();
    update_frame(&ctx, &mut app, vec![]);
    update_frame(&ctx, &mut app, vec![]);
    app.request_draft_rehearsal(&ctx);
    wait_prepared(&ctx, &mut app);
    update_frame(&ctx, &mut app, vec![]);
    update_frame(&ctx, &mut app, vec![]);
    (ctx, app)
}

fn assert_author_inputs(app: &WorldeditApp, baseline: &str, source: &str, generation: u64) {
    assert_eq!(app.new_file.as_deref(), Some("events/未提交🌦️.wl"));
    assert!(app.search_open);
    assert_eq!(app.project_query, "未应用真实正文");
    assert_eq!(app.project.content_baseline(), baseline);
    let buffer = &app.manuscript.writing_buffers()[0];
    assert_eq!(buffer.source(), source);
    assert_eq!(buffer.generation(), generation);
    assert!(app.directory.is_none());
    assert!(app.pending.is_none());
    assert!(app.draft_action.is_none());
    assert!(!app.project.root.exists(), "快捷键不得创建磁盘工程");
}

#[test]
fn preparation_escape_preserves_new_file_search_and_blocks_same_frame_global_shortcuts() {
    let (ctx, mut app) = preparation_over_inputs();
    let baseline = app.project.content_baseline();
    let buffer = app.manuscript.writing_buffers()[0].clone();
    let search = app.play_search_signature();
    app.remember(app.project.clone());
    app.remember(app.project.clone());
    app.restore_history_step(false).unwrap();
    let history = (app.history.len(), app.redo.len());
    for shortcut in [Key::Z, Key::S, Key::O, Key::P] {
        // 没有 TextEdit 焦点时 Ctrl+Z 原本会走工程撤销，不能靠控件抢键掩盖问题。
        ctx.memory_mut(|memory| {
            if let Some(id) = memory.focused() {
                memory.surrender_focus(id);
            }
        });
        update_frame(
            &ctx,
            &mut app,
            vec![key(shortcut, true, Modifiers::COMMAND)],
        );
        update_frame(
            &ctx,
            &mut app,
            vec![key(shortcut, false, Modifiers::COMMAND)],
        );
        assert!(app.draft_rehearsal.has_pending(), "{shortcut:?}");
        assert_eq!((app.history.len(), app.redo.len()), history);
        assert!(!app.command_palette.open);
        assert_eq!(app.play_search_signature(), search);
        assert_author_inputs(&app, &baseline, buffer.source(), buffer.generation());
    }
    // 同一输入批先取消后仍不能撤销、保存、打开目录或重设底下的搜索。
    update_frame(
        &ctx,
        &mut app,
        vec![
            key(Key::Escape, true, Modifiers::NONE),
            key(Key::Z, true, Modifiers::COMMAND),
            key(Key::S, true, Modifiers::COMMAND),
            key(Key::O, true, Modifiers::COMMAND),
            key(Key::F, true, Modifiers::COMMAND),
        ],
    );
    assert!(!app.draft_rehearsal.has_pending());
    assert!(app.draft_rehearsal.running.is_none());
    assert_eq!((app.history.len(), app.redo.len()), history);
    assert_eq!(app.play_search_signature(), search);
    assert_author_inputs(&app, &baseline, buffer.source(), buffer.generation());
    update_frame(&ctx, &mut app, vec![]);
    update_frame(&ctx, &mut app, vec![key(Key::P, true, Modifiers::COMMAND)]);
    assert!(
        app.command_palette.open,
        "后一帧的新命令不能永久被准备层拦截"
    );
}

#[test]
fn preparation_complete_ime_and_candidate_keys_do_not_cancel_or_confirm() {
    let (ctx, mut app) = preparation_over_inputs();
    let baseline = app.project.content_baseline();
    let buffer = app.manuscript.writing_buffers()[0].clone();
    for event in [
        egui::ImeEvent::Enabled,
        egui::ImeEvent::Preedit("待提交候选🌦️".into()),
    ] {
        update_frame(&ctx, &mut app, vec![Event::Ime(event)]);
        assert!(app.draft_rehearsal.has_pending());
    }
    for candidate in [
        Key::ArrowDown,
        Key::ArrowUp,
        Key::Space,
        Key::Enter,
        Key::Escape,
    ] {
        for pressed in [true, false] {
            update_frame(
                &ctx,
                &mut app,
                vec![key(candidate, pressed, Modifiers::NONE)],
            );
            assert!(app.draft_rehearsal.has_pending(), "{candidate:?}");
            assert!(app.draft_rehearsal.running.is_none());
            assert_author_inputs(&app, &baseline, buffer.source(), buffer.generation());
        }
    }
    for event in [
        egui::ImeEvent::Commit("已提交候选🌦️".into()),
        egui::ImeEvent::Disabled,
    ] {
        update_frame(
            &ctx,
            &mut app,
            vec![
                Event::Ime(event),
                key(Key::Escape, true, Modifiers::NONE),
                key(Key::S, true, Modifiers::COMMAND),
                key(Key::O, true, Modifiers::COMMAND),
            ],
        );
        assert!(app.draft_rehearsal.has_pending(), "完成帧不能取消准备");
        assert!(app.draft_rehearsal.running.is_none());
        assert_author_inputs(&app, &baseline, buffer.source(), buffer.generation());
    }
    update_frame(
        &ctx,
        &mut app,
        vec![key(Key::Escape, false, Modifiers::NONE)],
    );
    update_frame(
        &ctx,
        &mut app,
        vec![key(Key::Escape, true, Modifiers::NONE)],
    );
    assert!(!app.draft_rehearsal.has_pending());
    assert_author_inputs(&app, &baseline, buffer.source(), buffer.generation());
}

#[test]
fn preparation_handler_preserves_ime_and_text_events_for_the_receiving_control() {
    let (ctx, mut app) = preparation_over_inputs();
    let events = vec![
        Event::Ime(egui::ImeEvent::Enabled),
        Event::Ime(egui::ImeEvent::Preedit("候选".into())),
        key(Key::Escape, true, Modifiers::NONE),
        key(Key::Enter, true, Modifiers::NONE),
        Event::Ime(egui::ImeEvent::Commit("确认🌦️".into())),
        Event::Ime(egui::ImeEvent::Disabled),
        Event::Text("保留输入".into()),
    ];
    let _ = ctx.run(
        egui::RawInput {
            events: events.clone(),
            ..Default::default()
        },
        |ctx| {
            app.author_shortcuts(ctx);
            assert_eq!(ctx.input(|input| input.events.clone()), events);
            assert!(app.draft_rehearsal.has_pending());
            assert!(app.command_palette.ime_frame);
            assert!(!app.command_palette.ime);
        },
    );
}

#[test]
fn cancelling_preparation_cannot_activate_an_underlying_regular_choice_in_the_same_frame() {
    let (ctx, mut app) = app();
    app.personal.pending_restore = false;
    app.start_play_inner(app.applied_play_scope().unwrap());
    app.tab = Tab::Play;
    update_frame(&ctx, &mut app, vec![]);
    update_frame(&ctx, &mut app, vec![]);
    let mut found = false;
    for _ in 0..100 {
        let output = update_frame(&ctx, &mut app, vec![key(Key::Tab, true, Modifiers::NONE)]);
        found = output.platform_output.events.iter().any(|event| {
            matches!(event, egui::output::OutputEvent::FocusGained(info)
                if info.label.as_deref() == Some("选择：继续"))
        });
        update_frame(&ctx, &mut app, vec![key(Key::Tab, false, Modifiers::NONE)]);
        if found {
            break;
        }
    }
    assert!(found, "普通选择须先有真实键盘焦点");
    let choice_focus = ctx.memory(|memory| memory.focused()).unwrap();
    let original = app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .save()
        .unwrap();
    app.request_draft_rehearsal(&ctx);
    assert!(app.draft_rehearsal.has_pending());
    // 尚未画准备层：旧普通按钮仍有焦点，只有本帧保护能可靠拦住 Enter。
    let expected_frame = ctx.cumulative_frame_nr();
    let mut choice_responses = Vec::new();
    update_frame_observed(
        &ctx,
        &mut app,
        vec![
            key(Key::Escape, true, Modifiers::NONE),
            key(Key::Enter, true, Modifiers::NONE),
        ],
        |ctx, app| {
            assert_eq!(ctx.cumulative_frame_nr(), expected_frame);
            assert_eq!(app.draft_rehearsal.preparation_frame, Some(expected_frame));
            assert!(!app.draft_rehearsal.has_pending());
            // egui 0.32.3 end_pass 会交换 this_pass/prev_pass；read_response 优先
            // this_pass，所以必须在交换前捕获当前取消帧，不能读成前一帧按钮。
            choice_responses.push(ctx.read_response(choice_focus).unwrap());
        },
    );
    assert!(!app.draft_rehearsal.has_pending());
    assert!(!choice_responses.is_empty());
    for response in choice_responses {
        assert!(!response.enabled(), "取消帧仍须禁用底下的普通选择");
    }
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .save()
            .unwrap(),
        original
    );
    assert!(app.draft_rehearsal.running.is_none());
    assert!(app.replay_debugger.saved_paths.is_empty());
}
