//! 离屏真实线程/Story往返；不冒充原生窗口或物理IME测试。
use super::*;
use crate::{
    app::{manuscript::ManuscriptSession, Tab},
    draft_rehearsal_worker::Action,
};
use worldline_core::project::Project;
#[path = "layout_tests.rs"]
mod layout;
#[path = "shortcut_tests.rs"]
mod shortcuts;

fn app() -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = std::env::temp_dir().join(format!(
        "draft-rehearsal-ui-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.project = Project::new(&root);
    app.active_file = app.project.entry.clone();
    app.project
        .set_text(
            &app.active_file.clone(),
            "let n = 0\nevent start\n  已应用正文\n  choice \"继续\"\n    -> END\n".into(),
        )
        .unwrap();
    app.project.create_authoring_document(&root.join(".world/project.json"),br#"{"schema_version":1,"required_features":["presentation.manuscripts.v1"],"manuscripts":{"book":".world/manuscripts/book.json"}}"#.to_vec()).unwrap();
    app.project.create_authoring_document(&root.join(".world/manuscripts/book.json"),r#"{"schema_version":1,"id":"book","title":"试演书稿","entries":[{"id":"first","kind":"chapter","title":"第一章","target_ref":{"kind":"event","id":"start"}}]}"#.as_bytes().to_vec()).unwrap();
    app.reset_views();
    app.recompile();
    app.tab = Tab::Manuscript;
    app.restore_manuscript_session(ManuscriptSession {
        manuscript_id: Some("book".into()),
        selected_id: Some("first".into()),
        ..Default::default()
    });
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, vec![]);
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    buffer.replace_source("let n = 0\nevent start\n  未应用真实正文🌦️\n  choice \"继续\"\n    set n = 4\n    -> END\n".into());
    app.manuscript.restore_writing_buffers(&[buffer]);
    frame(&ctx, &mut app, vec![]);
    (ctx, app)
}
fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            app.author_shortcuts(ctx);
            if app.tab == Tab::Manuscript {
                app.manuscript_tab(ctx);
            } else {
                app.play_tab(ctx);
            }
            app.draft_rehearsal_dialog(ctx);
        },
    )
}
fn wait_prepared(ctx: &egui::Context, app: &mut WorldeditApp) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        app.poll_draft_rehearsal(ctx);
        let pending = app.draft_rehearsal.pending.as_ref().unwrap();
        assert!(pending.error.is_none(), "{:?}", pending.error);
        if pending.view.is_some() {
            return;
        }
        assert!(std::time::Instant::now() < end, "准备未返回");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
fn wait_idle(ctx: &egui::Context, app: &mut WorldeditApp) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        app.poll_draft_rehearsal(ctx);
        let running = app.draft_rehearsal.running.as_ref().unwrap();
        if !running.worker.as_ref().is_some_and(SessionWorker::busy) {
            return;
        }
        assert!(std::time::Instant::now() < end, "试演未返回");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
fn start(ctx: &egui::Context, app: &mut WorldeditApp) {
    app.request_draft_rehearsal(ctx);
    wait_prepared(ctx, app);
    app.confirm_draft_rehearsal(ctx);
    wait_idle(ctx, app);
}

#[test]
fn isolated_play_keeps_regular_story_undo_disk_and_same_author_draft() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let before = app.project.snapshot_files().unwrap();
    let history = (app.history.len(), app.redo.len());
    let buffer = app.manuscript.writing_buffers()[0].clone();
    let origin = app.manuscript_session();
    app.start_play_inner(app.applied_play_scope().unwrap());
    let regular = app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .save()
        .unwrap();
    start(&ctx, &mut app);
    assert!(app
        .draft_rehearsal
        .running
        .as_ref()
        .unwrap()
        .transcript
        .contains("未应用真实正文🌦️"));
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .save()
            .unwrap(),
        regular
    );
    assert_eq!(app.project.snapshot_files().unwrap(), before);
    assert_eq!((app.history.len(), app.redo.len()), history);
    let choice = app.draft_rehearsal.running.as_ref().unwrap().view.choices[0]
        .id
        .clone();
    app.send_rehearsal_action(Action::Choose {
        id: choice,
        budget: ReplayBudget::new(100, 250),
    });
    wait_idle(&ctx, &mut app);
    assert!(app.draft_rehearsal.running.as_ref().unwrap().view.ended);
    assert_eq!(
        app.draft_rehearsal
            .running
            .as_ref()
            .unwrap()
            .view
            .inspection
            .as_ref()
            .unwrap()
            .items[0]
            .current
            .display,
        "4"
    );
    app.return_rehearsal_origin(&ctx);
    assert_eq!(app.tab, Tab::Manuscript, "{:?}", app.draft_rehearsal.notice);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.manuscript_session().mode, origin.mode);
    assert_eq!(
        app.manuscript.writing_buffers()[0].source(),
        buffer.source()
    );
    assert_eq!(
        app.manuscript.writing_buffers()[0].generation(),
        buffer.generation()
    );
    assert_eq!(
        std::fs::read_to_string(&app.active_file).unwrap(),
        app.project.document(&app.active_file).unwrap()
    );
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn cancel_repeat_ime_stale_source_and_explicit_rehearsal_preserve_inputs() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    app.ime_composing = true;
    app.request_draft_rehearsal(&ctx);
    assert!(app.draft_rehearsal.pending.is_none());
    app.ime_composing = false;
    app.request_draft_rehearsal(&ctx);
    let id = app
        .draft_rehearsal
        .pending
        .as_ref()
        .unwrap()
        .session_id
        .clone();
    app.request_draft_rehearsal(&ctx);
    assert_eq!(app.draft_rehearsal.pending.as_ref().unwrap().session_id, id);
    app.draft_rehearsal.pending = None;
    start(&ctx, &mut app);
    let before = app
        .draft_rehearsal
        .running
        .as_ref()
        .unwrap()
        .view
        .inspection
        .as_ref()
        .unwrap()
        .stamp;
    let path = app.active_file.clone();
    let buffer = app.manuscript.writing_buffer_mut(&path).unwrap();
    buffer.replace_source(buffer.source().replace("真实正文", "新的正文"));
    app.poll_draft_rehearsal(&ctx);
    assert!(app.draft_rehearsal.running.as_ref().unwrap().stale);
    app.send_rehearsal_action(Action::Continue {
        budget: ReplayBudget::new(100, 250),
    });
    assert!(app
        .draft_rehearsal
        .notice
        .as_ref()
        .unwrap()
        .contains("变化"));
    assert_eq!(
        app.draft_rehearsal
            .running
            .as_ref()
            .unwrap()
            .view
            .inspection
            .as_ref()
            .unwrap()
            .stamp,
        before
    );
    app.return_rehearsal_origin(&ctx);
    assert_eq!(app.tab, Tab::Play);
    assert_eq!(app.project.content_baseline(), baseline);
    start(&ctx, &mut app);
    assert!(app
        .draft_rehearsal
        .running
        .as_ref()
        .unwrap()
        .transcript
        .contains("未应用新的正文"));
}

#[test]
fn confirmation_and_rehearsal_controls_fit_narrow_offscreen_view() {
    layout::check_dialog_and_rehearsal(egui::vec2(800.0, 600.0), 1.0);
}

#[test]
fn explicit_apply_then_new_regular_story_can_record_a_strict_formal_path() {
    let (ctx, mut app) = app();
    start(&ctx, &mut app);
    let frozen = app
        .draft_rehearsal
        .running
        .as_ref()
        .unwrap()
        .transcript
        .clone();
    app.return_rehearsal_origin(&ctx);
    fn position(shape: &egui::Shape, label: &str, clip: egui::Rect) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.job.text == label => {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                clip.contains_rect(rect).then_some(rect.center())
            }
            egui::Shape::Vec(shapes) => {
                shapes.iter().find_map(|shape| position(shape, label, clip))
            }
            _ => None,
        }
    }
    fn point(ctx: &egui::Context, output: &egui::FullOutput, label: &str) -> Option<egui::Pos2> {
        output.shapes.iter().find_map(|shape| {
            position(
                &shape.shape,
                label,
                shape.clip_rect.intersect(ctx.screen_rect()),
            )
        })
    }
    fn click(ctx: &egui::Context, app: &mut WorldeditApp, pos: egui::Pos2) {
        for pressed in [true, false] {
            frame(
                ctx,
                app,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
    }
    let mut output = frame(&ctx, &mut app, vec![]);
    if point(&ctx, &output, "应用正文草稿").is_none() {
        assert!(!egui::Popup::is_any_open(&ctx));
        let tools = point(&ctx, &output, "正文工具").expect("可见的正文工具入口");
        click(&ctx, &mut app, tools);
        assert!(egui::Popup::is_any_open(&ctx));
        frame(&ctx, &mut app, vec![]); // 首次 popup 测量后绘出真实控件。
        output = frame(&ctx, &mut app, vec![]);
    }
    let apply = point(&ctx, &output, "应用正文草稿").expect("现有完整可见的显式应用按钮");
    click(&ctx, &mut app, apply);
    assert!(app
        .project
        .document(&app.active_file)
        .unwrap()
        .contains("未应用真实正文"));
    assert_eq!(
        app.draft_rehearsal.running.as_ref().unwrap().transcript,
        frozen
    );
    app.draft_rehearsal.active = false;
    app.tab = Tab::Play;
    app.start_play();
    assert!(app.play_confirmation.is_none());
    let story = app.play.as_mut().unwrap().story.as_mut().unwrap();
    story.continue_story().unwrap();
    story.choose(0).unwrap();
    story.continue_story().unwrap();
    let trace = story.replay_trace();
    assert!(trace.complete);
    app.replay_debugger.record_replay_path(trace.clone());
    assert_eq!(app.replay_debugger.saved_paths.last().unwrap().trace, trace);
    assert_eq!(
        trace.fingerprint,
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint
    );
}

#[test]
fn inspector_complete_ime_events_hold_actions_without_staling_author_source() {
    let (ctx, mut app) = app();
    start(&ctx, &mut app);
    app.draft_rehearsal.pane = Pane::State;
    frame(&ctx, &mut app, vec![]);
    ctx.memory_mut(|memory| {
        memory.request_focus(egui::Id::new("draft-rehearsal-inspection-search"))
    });
    let before = app.manuscript.writing_buffers()[0].source().to_owned();
    let generation = app.manuscript.writing_buffers()[0].generation();
    let stamp = app
        .draft_rehearsal
        .running
        .as_ref()
        .unwrap()
        .view
        .inspection
        .as_ref()
        .unwrap()
        .stamp;
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Ime(egui::ImeEvent::Enabled)],
    );
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Ime(egui::ImeEvent::Preedit("变量".into()))],
    );
    for key in [egui::Key::ArrowDown, egui::Key::Space, egui::Key::Enter] {
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        assert_eq!(app.tab, Tab::Play);
        assert!(!app.draft_rehearsal.running.as_ref().unwrap().stale);
        assert_eq!(
            app.draft_rehearsal
                .running
                .as_ref()
                .unwrap()
                .view
                .inspection
                .as_ref()
                .unwrap()
                .stamp,
            stamp
        );
    }
    app.send_rehearsal_action(Action::Continue {
        budget: ReplayBudget::new(100, 250),
    });
    assert!(app
        .draft_rehearsal
        .notice
        .as_ref()
        .unwrap()
        .contains("组合"));
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Ime(egui::ImeEvent::Commit("变量".into()))],
    );
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Ime(egui::ImeEvent::Disabled)],
    );
    frame(&ctx, &mut app, vec![]);
    wait_idle(&ctx, &mut app);
    assert!(app
        .draft_rehearsal
        .running
        .as_ref()
        .unwrap()
        .query
        .text
        .contains("变量"));
    assert!(!app.draft_rehearsal.running.as_ref().unwrap().stale);
    assert_eq!(app.manuscript.writing_buffers()[0].source(), before);
    assert_eq!(app.manuscript.writing_buffers()[0].generation(), generation);
    assert_eq!(
        app.draft_rehearsal
            .running
            .as_ref()
            .unwrap()
            .view
            .inspection
            .as_ref()
            .unwrap()
            .stamp,
        stamp
    );
}

#[test]
fn held_and_repeated_enter_cannot_select_the_next_runtime_choice_group() {
    let (ctx, mut app) = app();
    let path = app.active_file.clone();
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source(
            concat!(
                "event start\n  choice \"继续\"\n    -> next\n",
                "event next\n  choice \"继续\"\n    -> last\n",
                "event last\n  choice \"继续\"\n    -> END\n",
            )
            .into(),
        );
    start(&ctx, &mut app);
    fn event(key: egui::Key, pressed: bool, repeat: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn focus_choice(ctx: &egui::Context, app: &mut WorldeditApp) -> egui::Id {
        for _ in 0..100 {
            let output = frame(ctx, app, vec![event(egui::Key::Tab, true, false)]);
            let found = output.platform_output.events.iter().any(|event| {
                matches!(event, egui::output::OutputEvent::FocusGained(info)
                    if info.label.as_deref() == Some("选择：继续"))
            });
            frame(ctx, app, vec![event(egui::Key::Tab, false, false)]);
            if found {
                return ctx.memory(|memory| memory.focused()).unwrap();
            }
        }
        panic!("隔离试演选择不可用键盘到达");
    }
    let old_id = focus_choice(&ctx, &mut app);
    frame(&ctx, &mut app, vec![event(egui::Key::Enter, true, false)]);
    frame(&ctx, &mut app, vec![event(egui::Key::Enter, true, true)]);
    wait_idle(&ctx, &mut app);
    assert_eq!(app.draft_rehearsal.running.as_ref().unwrap().view.turns, 1);
    for _ in 0..4 {
        frame(&ctx, &mut app, vec![event(egui::Key::Enter, true, true)]);
        assert_eq!(app.draft_rehearsal.running.as_ref().unwrap().view.turns, 1);
    }
    frame(&ctx, &mut app, vec![event(egui::Key::Enter, false, false)]);
    let next_id = focus_choice(&ctx, &mut app);
    assert_ne!(old_id, next_id, "新观测不得复用旧行的控件身份");
    frame(&ctx, &mut app, vec![event(egui::Key::Enter, true, false)]);
    frame(&ctx, &mut app, vec![event(egui::Key::Enter, false, false)]);
    wait_idle(&ctx, &mut app);
    assert_eq!(app.draft_rehearsal.running.as_ref().unwrap().view.turns, 2);
}
