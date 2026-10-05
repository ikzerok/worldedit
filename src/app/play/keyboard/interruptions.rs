use super::*;

#[test]
fn all_locked_fallthrough_and_empty_group_follow_runtime_end() {
    for source in [
        "event start\n  choice \"第一步\"\n    -> next\nevent next\n  choice \"锁定\" enable false disabled \"不满足\"\n    -> END\n  落穿正文\n  -> END\n",
        "event start\n  choice \"第一步\"\n    -> next\nevent next\n  choice \"隐藏\" if false\n    -> END\n  空组正文\n  -> END\n",
    ] {
        let mut h = Harness::new(source);
        h.start(Key::Enter);
        h.key(Key::Enter);
        h.assert_focus("● 保存当前路径");
        assert_eq!(h.steps(), 1);
        let play = h.app.play.as_ref().unwrap();
        assert!(play.ended && !play.paused && play.error.is_none());
        assert!(play.story.as_ref().unwrap().choice_presentations().is_empty());
        assert!(h.app.replay_debugger.saved_paths.is_empty());
    }
}

#[test]
fn story_error_and_budget_interruptions_never_request_success_focus() {
    for (source, steps) in [
        ("event start\n  choice \"第一步\"\n    -> next\nevent next\n  choice \"失败\" if rnd(1, 100) / 0 > 0\n    -> END\n", 100),
        ("event start\n  choice \"第一步\"\n    -> next\nevent next\n  -> loop\nevent loop\n  -> next\n", 4),
    ] {
        let mut h = Harness::new(source);
        h.start(Key::Enter);
        h.app.replay_debugger.live_max_steps = steps;
        h.key(Key::Enter);
        let play = h.app.play.as_ref().unwrap();
        assert!(play.paused && !play.ended);
        if steps == 100 { assert!(play.error.is_some()); }
        else { assert_eq!(play.interruption, Some(ContinuationOutcome::StepBudgetExceeded)); }
        assert!(h.app.play_keyboard.pending.is_none());
        let state = play.story.as_ref().unwrap().save().unwrap();
        for _ in 0..4 { h.idle(); }
        assert_eq!(h.app.play.as_ref().unwrap().story.as_ref().unwrap().save().unwrap(), state);
    }
}

#[test]
fn pause_stop_and_cancelled_outcome_discard_pending_without_resuming() {
    for action in ["Ⅱ 暂停", "■ 停止", "cancelled"] {
        let mut h = Harness::new(NORMAL);
        h.start(Key::Enter);
        h.press(Key::Enter);
        assert!(h.app.play_keyboard.pending.is_some());
        if action == "cancelled" {
            // 此状态由 runtime 停止/取消出口提供；普通 UI 没有额外的自动续行。
            h.app.play.as_mut().unwrap().paused = true;
            h.app.play.as_mut().unwrap().interruption = Some(ContinuationOutcome::Cancelled);
        } else {
            h.pointer(action);
        }
        h.release(Key::Enter);
        for _ in 0..3 {
            h.idle();
        }
        assert!(h.app.play_keyboard.pending.is_none());
        assert!(h.app.play.as_ref().unwrap().paused);
        assert!(!h.app.play.as_ref().unwrap().ended);
        assert_eq!(h.steps(), 1);
    }
}

#[test]
fn newer_keyboard_navigation_and_commands_win_over_pending_restore() {
    for (key, modifiers) in [
        (Key::Tab, Modifiers::NONE),
        (Key::Tab, Modifiers::SHIFT),
        (Key::ArrowDown, Modifiers::NONE),
        (Key::ArrowUp, Modifiers::NONE),
        (Key::Escape, Modifiers::NONE),
        (Key::P, Modifiers::COMMAND),
    ] {
        let mut h = Harness::new(NORMAL);
        h.start(Key::Enter);
        h.press(Key::Enter);
        h.frame(vec![key_event(key, true, false, modifiers)]);
        assert!(
            h.app.play_keyboard.pending.is_none(),
            "{key:?} {modifiers:?}"
        );
        h.release(Key::Enter);
        h.frame(vec![key_event(key, false, false, modifiers)]);
        let focus = h.focused();
        for _ in 0..3 {
            h.idle();
        }
        assert_eq!(h.focused(), focus);
        assert_eq!(h.steps(), 1);
    }
}

#[test]
fn pointer_wheel_view_and_window_changes_cancel_pending() {
    for change in 0..6 {
        let mut h = Harness::new(NORMAL);
        h.start(Key::Enter);
        h.press(Key::Enter);
        match change {
            0 => {
                h.pointer("路线对照");
            }
            1 => {
                h.frame(vec![Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -50.0),
                    modifiers: Modifiers::NONE,
                }]);
            }
            2 => {
                h.frame(vec![Event::WindowFocused(false)]);
            }
            3 => {
                h.app.tab = Tab::Edit;
                h.idle();
            }
            4 => {
                h.app.command_palette.open = true;
                h.idle();
            }
            _ => {
                h.pointer("普通试玩");
            }
        }
        assert!(h.app.play_keyboard.pending.is_none(), "case {change}");
        h.release(Key::Enter);
        h.app.tab = Tab::Play;
        h.app.comparison.active = false;
        h.app.command_palette.open = false;
        for _ in 0..3 {
            h.idle();
        }
        assert!(h.app.play_keyboard.pending.is_none());
        assert_eq!(h.steps(), 1);
    }
}

#[test]
fn ime_preedit_commit_disabled_and_repeat_require_a_new_confirmation() {
    for key in [Key::Enter, Key::Space] {
        let mut h = Harness::new(NORMAL);
        h.start(key);
        let focus = h.focused();
        h.frame(vec![
            Event::Ime(egui::ImeEvent::Enabled),
            Event::Ime(egui::ImeEvent::Preedit("正在输入".into())),
            key_event(key, true, false, Modifiers::NONE),
        ]);
        assert_eq!(h.steps(), 0);
        h.frame(vec![
            Event::Ime(egui::ImeEvent::Commit("确认".into())),
            key_event(key, true, true, Modifiers::NONE),
        ]);
        h.frame(vec![
            Event::Ime(egui::ImeEvent::Disabled),
            key_event(key, true, true, Modifiers::NONE),
        ]);
        h.frame(vec![key_event(key, true, true, Modifiers::NONE)]);
        assert_eq!(h.steps(), 0);
        assert_eq!(h.focused(), focus);
        assert!(h.app.play_keyboard.pending.is_none());
        h.release(key);
        h.key(key);
        h.assert_focus("选择：第二步");
        assert_eq!(h.steps(), 1);
    }
}

#[test]
fn ime_during_pending_cancels_handoff_and_does_not_save_at_end() {
    let mut h = Harness::new("event start\n  choice \"第一步\"\n    -> END\n");
    h.start(Key::Enter);
    h.press(Key::Enter);
    h.frame(vec![Event::Ime(egui::ImeEvent::Preedit("新输入".into()))]);
    h.frame(vec![Event::Ime(egui::ImeEvent::Commit("新输入".into()))]);
    h.release(Key::Enter);
    for _ in 0..3 {
        h.idle();
    }
    assert!(h.app.play_keyboard.pending.is_none());
    assert!(h.app.replay_debugger.saved_paths.is_empty());
}

#[test]
fn applied_draft_new_session_and_workspace_replacement_cannot_reuse_pending() {
    for change in 0..4 {
        let mut h = Harness::new(NORMAL);
        h.start(Key::Enter);
        h.press(Key::Enter);
        match change {
            0 => {
                h.app
                    .project
                    .set_text(&h.app.active_file.clone(), NORMAL.replace("第一步", "新稿"))
                    .unwrap();
                h.app.recompile();
            }
            1 => h.app.start_play(),
            2 => h.app.reset_views(),
            _ => {
                h.app.project.root = h.app.project.root.join("different-workspace");
            }
        }
        h.release(Key::Enter);
        for _ in 0..3 {
            h.idle();
        }
        assert!(h.app.play_keyboard.pending.is_none(), "case {change}");
        assert!(h.app.replay_debugger.saved_paths.is_empty());
    }
}

#[test]
fn draft_scope_confirmation_is_not_a_successful_keyboard_start() {
    let mut h = Harness::new(NORMAL);
    let mut buffer = h
        .app
        .project
        .open_source_writing_buffer(&h.app.active_file)
        .unwrap();
    buffer.replace_source("event start\n  尚未应用\n".into());
    h.app.manuscript.restore_writing_buffers(&[buffer]);
    h.start(Key::Enter);
    assert!(h.app.play.is_none());
    assert!(h.app.play_confirmation.is_some());
    assert!(h.app.play_keyboard.pending.is_none());
    h.pointer("取消运行");
    for _ in 0..3 {
        h.idle();
    }
    assert!(h.app.play.is_none());
    assert!(h.app.play_confirmation.is_none());
    assert!(h.app.play_keyboard.pending.is_none());
}

#[test]
fn rejected_snapshot_start_cancels_pending_without_creating_a_session() {
    let mut h = Harness::new(NORMAL);
    h.start(Key::Enter);
    h.press(Key::Enter);
    let scope = h.app.play.as_ref().unwrap().scope.clone();
    let session = h.app.play_keyboard.session();
    let state = h
        .app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .save()
        .unwrap();
    h.app.version += 1;
    h.app.start_play_inner(scope);
    assert_eq!(h.app.play_keyboard.session(), session);
    assert!(h.app.play_keyboard.pending.is_none());
    assert_eq!(
        h.app
            .play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .save()
            .unwrap(),
        state
    );
    h.release(Key::Enter);
    h.idle();
    assert_eq!(h.steps(), 1);
}
