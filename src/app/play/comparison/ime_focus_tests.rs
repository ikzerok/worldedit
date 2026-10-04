//! 原生观察到返回完成后短暂 IME 保护；具体 IME 子型用独立回归覆盖，不冒称实测。
use super::saltbell_focus::{frame_full, key, setup_saltbell};
use super::*;
const LABEL: &str = "打开实际动作来源";

fn ready(right: bool) -> (egui::Context, WorldeditApp, egui::Id) {
    let (ctx, mut app) = setup_saltbell();
    frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
    let mut seen = 0;
    for _ in 0..180 {
        if key(&ctx, &mut app, egui::Key::Tab, egui::Modifiers::NONE, false)
            .iter()
            .any(|info| {
                info.typ == egui::WidgetType::Button && info.label.as_deref() == Some(LABEL)
            })
        {
            seen += 1;
            if seen == usize::from(right) + 1 {
                let id = ctx.memory(|m| m.focused()).unwrap();
                for _ in 0..16 {
                    frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
                }
                key(
                    &ctx,
                    &mut app,
                    egui::Key::Enter,
                    egui::Modifiers::NONE,
                    false,
                );
                assert_eq!(app.tab, Tab::Edit);
                for _ in 0..8 {
                    frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
                }
                frame_full(
                    &ctx,
                    &mut app,
                    vec![arrow(true)],
                    egui::Modifiers::ALT,
                    false,
                );
                for _ in 0..3 {
                    frame_full(&ctx, &mut app, vec![], egui::Modifiers::ALT, false);
                }
                assert_eq!(app.tab, Tab::Play);
                assert_eq!(ctx.memory(|m| m.focused()), Some(id));
                return (ctx, app, id);
            }
        }
    }
    panic!("实际来源键盘不可达");
}
fn arrow(pressed: bool) -> egui::Event {
    egui::Event::Key {
        key: egui::Key::ArrowLeft,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::ALT,
    }
}
fn ime(ctx: &egui::Context, app: &mut WorldeditApp, event: egui::ImeEvent) {
    frame_full(
        ctx,
        app,
        vec![arrow(false), egui::Event::Ime(event)],
        egui::Modifiers::ALT,
        false,
    );
}
#[test]
fn transient_ime_guard_keeps_source_identity_and_restores_focus_for_enter() {
    for right in [false, true] {
        for event in [
            egui::ImeEvent::Disabled,
            egui::ImeEvent::Commit(String::new()),
        ] {
            let (ctx, mut app, id) = ready(right);
            let before = invariant(&app);
            let mut guarded_passes = Vec::new();
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1188.0, 848.0),
                    )),
                    modifiers: egui::Modifiers::ALT,
                    events: vec![arrow(false), egui::Event::Ime(event)],
                    ..Default::default()
                },
                |ctx| {
                    eframe::App::update(&mut app, ctx, &mut eframe::Frame::_new_kittest());
                    if app.command_palette.ime_frame {
                        guarded_passes.push(ctx.read_response(id).map(|r| r.enabled()));
                    }
                },
            );
            assert!(!guarded_passes.is_empty());
            assert!(
                guarded_passes.iter().all(|enabled| *enabled == Some(false)),
                "每个IME保护pass必须禁用同一语义来源，right={right}: {guarded_passes:?}"
            );
            assert_eq!(app.tab, Tab::Play);
            frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
            assert_eq!(ctx.memory(|m| m.focused()), Some(id));
            key(
                &ctx,
                &mut app,
                egui::Key::Enter,
                egui::Modifiers::NONE,
                false,
            );
            assert_eq!(app.tab, Tab::Edit);
            assert_eq!(app.comparison.selected_action, Some((right, 2)));
            assert_eq!(invariant(&app), before);
        }
    }
}
#[test]
fn ongoing_ime_keeps_navigation_blocked_and_does_not_change_live_story() {
    let (ctx, mut app, id) = ready(false);
    let before = invariant(&app);
    ime(&ctx, &mut app, egui::ImeEvent::Enabled);
    for _ in 0..3 {
        ime(&ctx, &mut app, egui::ImeEvent::Preedit("未提交组合".into()));
        key(
            &ctx,
            &mut app,
            egui::Key::Enter,
            egui::Modifiers::NONE,
            false,
        );
        assert_eq!(app.tab, Tab::Play);
        assert!(!ctx.read_response(id).unwrap().enabled());
        assert_eq!(invariant(&app), before);
    }
    ime(&ctx, &mut app, egui::ImeEvent::Disabled);
    frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
    assert_eq!(ctx.memory(|m| m.focused()), Some(id));
}
#[test]
fn ime_focus_is_not_revived_after_comparison_context_changes() {
    for change in 0..5 {
        let (ctx, mut app, id) = ready(false);
        ime(&ctx, &mut app, egui::ImeEvent::Enabled);
        match change {
            0 => app.comparison.swap(),
            1 => app.tab = Tab::Overview,
            2 => {
                app.comparison.selected_state = Some("ledger_state".into());
            }
            3 => {
                app.comparison.result.as_mut().unwrap().id += 1;
            }
            _ => {
                let text = app.project.document(&app.active_file).unwrap().to_owned();
                app.project
                    .set_text(&app.active_file.clone(), format!("{text}\n# author edit"))
                    .unwrap();
                app.recompile();
            }
        }
        frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
        ime(&ctx, &mut app, egui::ImeEvent::Disabled);
        app.tab = Tab::Play;
        frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
        assert_ne!(
            ctx.memory(|m| m.focused()),
            Some(id),
            "旧上下文不得复活来源焦点"
        );
    }
}

#[test]
fn ime_guard_preserves_unapplied_draft_and_does_not_revive_stale_source_focus() {
    let (ctx, mut app, id) = ready(false);
    let before = invariant(&app);
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    let draft = format!("{}\n# kept unapplied draft", buffer.source());
    buffer.replace_source(draft.clone());
    app.manuscript.restore_writing_buffers(&[buffer]);
    ime(&ctx, &mut app, egui::ImeEvent::Enabled);
    key(
        &ctx,
        &mut app,
        egui::Key::Enter,
        egui::Modifiers::NONE,
        false,
    );
    assert_eq!(app.tab, Tab::Play);
    ime(&ctx, &mut app, egui::ImeEvent::Disabled);
    frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
    assert_ne!(ctx.memory(|m| m.focused()), Some(id));
    assert_eq!(app.manuscript.writing_buffers()[0].source(), draft);
    assert_eq!(invariant(&app), before);
}

#[test]
fn window_focus_loss_cancels_suspended_source_focus() {
    let (ctx, mut app, id) = ready(false);
    ime(&ctx, &mut app, egui::ImeEvent::Enabled);
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1188.0, 848.0),
            )),
            focused: false,
            events: vec![egui::Event::WindowFocused(false)],
            ..Default::default()
        },
        |ctx| eframe::App::update(&mut app, ctx, &mut eframe::Frame::_new_kittest()),
    );
    ime(&ctx, &mut app, egui::ImeEvent::Disabled);
    frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
    assert_ne!(ctx.memory(|m| m.focused()), Some(id));
}
