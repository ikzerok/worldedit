//! Full App localized recording: real END/path, then time field -> Tab -> Replay.
//! Synthetic input verifies current-frame geometry; it does not replace physical desktop QA.
use super::*;

const RECORD: &str = "● 保存当前路径";
const REPLAY: &str = "▶ 重放所选路径";
const OBSERVATION: &str = "查看原记录首次观测（只读）";
const DIFF: &str = "步骤前后状态差异";
const CHANGES: &str = "原路径逐步字段变化（只读记录）";
const EXPORT: &str = "导出所选路径 JSON";
const COPY: &str = "复制 JSON";

fn whole_control(h: &mut Harness, label: &str) -> egui::Response {
    let output = h.frame(vec![]);
    assert!(!rendered(&output).contains("use of widget ID"));
    // frame() captures after App::update, before end_pass swaps response registries.
    // Match that response against every glyph row in this same completed paint output.
    let response = h.focused.clone().expect("current-frame focused control");
    assert!(response.has_focus() && response.enabled());
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(response.id));
    let visible = response
        .rect
        .intersect(response.interact_rect)
        .intersect(h.ctx.screen_rect());
    assert!(
        visible.is_positive()
            && visible.width() + 0.5 >= response.rect.width()
            && visible.height() + 0.5 >= response.rect.height(),
        "{label}: actual keyboard target must be fully visible without pointer scrolling; rect={:?}, interact={:?}, screen={:?}",
        response.rect, response.interact_rect, h.ctx.screen_rect(),
    );
    let glyphs = painted(&output, label);
    assert!(!glyphs.is_empty(), "{label}: missing actual text paint");
    for (glyph, clip) in glyphs {
        let clip = clip.intersect(h.ctx.screen_rect());
        assert!(
            response.rect.contains_rect(glyph) && clip.contains_rect(glyph),
            "{label}: every glyph must fit its current paint clip; glyph={glyph:?}, clip={clip:?}, control={:?}",
            response.rect,
        );
    }
    response
}

fn tab_to(h: &mut Harness, label: &str, reverse: bool) -> egui::Id {
    let gained = h.tab(reverse);
    assert!(
        gained
            .iter()
            .any(|info| info.label.as_deref() == Some(label)),
        "expected real {}Tab to {label}, got {gained:?}",
        if reverse { "Shift+" } else { "" },
    );
    h.ctx.memory(|memory| memory.focused()).unwrap()
}

fn select_all(h: &mut Harness) {
    h.key(
        Key::A,
        Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        },
    );
    let id = h.ctx.memory(|memory| memory.focused()).unwrap();
    let selected = egui::TextEdit::load_state(&h.ctx, id)
        .expect("real numeric or path text input")
        .cursor
        .char_range()
        .unwrap();
    assert_ne!(selected.primary.index, selected.secondary.index);
}

fn recorded() -> Harness {
    let mut h = Harness::new();
    h.app.personal.settings.appearance.style = crate::theme::StylePreset::Studio;
    h.ctx.options_mut(|options| options.warn_on_id_clash = true);
    h.translated();
    assert!(!h.app.replay_debugger.locale.fallback);
    h.focus_label("▶ 开始试玩", false);
    whole_control(&mut h, "▶ 开始试玩");
    h.key(Key::Enter, Modifiers::NONE);
    whole_control(&mut h, "选择：继续 👋");
    let story = h.app.play.as_ref().unwrap().story.as_ref().unwrap();
    assert_eq!(
        story.vars().get("n"),
        Some(&worldline_runtime::Value::Num(0.0))
    );
    assert_eq!(
        story.presentation_identity().unwrap().request.target_locale,
        "zh-Hant"
    );
    h.key(Key::Enter, Modifiers::NONE);
    assert!(h.app.play.as_ref().unwrap().ended);
    let story = h.app.play.as_ref().unwrap().story.as_ref().unwrap();
    assert_eq!(
        story.vars().get("n"),
        Some(&worldline_runtime::Value::Num(7.0))
    );
    assert_eq!(story.replay_trace().steps.len(), 1);
    assert!(h.app.replay_debugger.saved_paths.is_empty());
    // The real END handoff opens the debugger and focuses Record. Edit its preceding field.
    whole_control(&mut h, RECORD);
    h.tab(true);
    select_all(&mut h);
    h.frame(vec![Event::Text("keyboard-path".into())]);
    assert_eq!(h.app.replay_debugger.path_name, "keyboard-path");
    tab_to(&mut h, RECORD, false);
    whole_control(&mut h, RECORD);
    h.key(Key::Enter, Modifiers::NONE);
    let paths = &h.app.replay_debugger.saved_paths;
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].name, "keyboard-path");
    assert!(paths[0].trace.complete);
    assert_eq!(paths[0].trace.steps.len(), 1);
    assert_eq!(h.app.replay_debugger.selected_path, Some(0));
    assert_eq!(h.ctx.screen_rect().size(), egui::vec2(381.5, 271.0));
    assert_eq!(h.ctx.pixels_per_point(), 2.0);
    h
}

fn focus_time(h: &mut Harness) {
    for _ in 0..12 {
        let gained = h.tab(false);
        if gained.iter().any(|info| {
            info.value == Some(30_000.0) || info.current_text_value.as_deref() == Some("30000")
        }) {
            whole_control(h, "30000");
            select_all(h);
            assert_eq!(h.app.replay_debugger.time_budget_ms, 30_000);
            return;
        }
    }
    panic!("the actual recorded path must expose its time-budget input");
}

fn wait_for_replay(h: &mut Harness) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while h.app.replay_debugger.job.is_some() {
        h.frame(vec![]);
        assert!(
            std::time::Instant::now() < deadline,
            "real replay worker timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let result = h.app.replay_debugger.result.as_ref().unwrap();
    assert!(matches!(
        result.status,
        worldline_runtime::ReplayStatus::Replayed {
            ended: true,
            complete: true
        }
    ));
    assert_eq!(result.completed_choices, 1);
    assert_eq!(
        result.state_diff["vars"]["n"],
        serde_json::to_value(worldline_runtime::Value::Num(7.0)).unwrap()
    );
}

#[test]
fn ordinary_replay_short_200_keyboard_time_tab_reveals_replay_then_real_result() {
    let mut h = recorded();
    let before = h.stable();
    focus_time(&mut h);
    tab_to(&mut h, REPLAY, false);
    whole_control(&mut h, REPLAY);
    assert_eq!(
        h.stable(),
        before,
        "revealing Replay cannot rerun the Story"
    );
    h.key(Key::Enter, Modifiers::NONE);
    wait_for_replay(&mut h);
    assert_eq!(
        h.stable(),
        before,
        "Replay keeps the live Story and its RNG"
    );
    h.focus_label(DIFF, false);
    whole_control(&mut h, DIFF);
    // This real disclosure is open by default; close and reopen it through its keyboard target.
    h.key(Key::Enter, Modifiers::NONE);
    h.key(Key::Enter, Modifiers::NONE);
    whole_control(&mut h, DIFF);
    let delta = h.app.replay_debugger.result.as_ref().unwrap().state_diff["vars"].to_string();
    read_text_after_replay(&mut h, &delta);
    let transcript = h.app.play.as_ref().unwrap().transcript.clone();
    read_text_after_replay(&mut h, &transcript);
    assert_eq!(h.stable(), before);
}

#[test]
fn ordinary_replay_short_200_keyboard_shift_tab_reveals_same_replay_identity() {
    let mut h = recorded();
    let before = h.stable();
    focus_time(&mut h);
    let replay = tab_to(&mut h, REPLAY, false);
    tab_to(&mut h, OBSERVATION, false);
    assert_eq!(tab_to(&mut h, REPLAY, true), replay);
    whole_control(&mut h, REPLAY);
    assert_eq!(h.stable(), before);
    assert!(h.app.replay_debugger.result.is_none());
}

// Pointer scrolling is permitted only after the keyboard Replay and result checks have passed,
// to verify ordinary reading of real output. It never rescues an invisible keyboard target.
fn read_text_after_replay(h: &mut Harness, label: &str) {
    assert!(h.app.replay_debugger.result.is_some());
    let mut from_top = false;
    for _ in 0..160 {
        let output = h.frame(vec![]);
        let glyphs = painted(&output, label);
        if !glyphs.is_empty()
            && glyphs
                .iter()
                .all(|(glyph, clip)| clip.intersect(h.ctx.screen_rect()).contains_rect(*glyph))
        {
            return;
        }
        let (point, delta) = if let Some((glyph, clip)) = glyphs.first() {
            (
                clip.center(),
                if glyph.center().y < clip.center().y {
                    30.0
                } else {
                    -30.0
                },
            )
        } else {
            let point = Pos2::new(270.0, 180.0);
            let delta = if from_top { -30.0 } else { 100_000.0 };
            from_top = true;
            (point, delta)
        };
        h.frame(vec![
            Event::PointerMoved(point),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, delta),
                modifiers: Modifiers::NONE,
            },
        ]);
    }
    panic!("real Replay output remains unreachable for ordinary reading: {label}");
}

#[test]
fn ordinary_replay_short_200_keyboard_record_details_export_and_copy_are_visible() {
    let mut h = recorded();
    let before = h.stable();
    focus_time(&mut h);
    tab_to(&mut h, REPLAY, false);
    whole_control(&mut h, REPLAY);
    tab_to(&mut h, OBSERVATION, false);
    whole_control(&mut h, OBSERVATION);
    h.key(Key::Enter, Modifiers::NONE);
    h.focus_label(CHANGES, false);
    whole_control(&mut h, CHANGES);
    h.key(Key::Enter, Modifiers::NONE);
    h.focus_label(EXPORT, false);
    whole_control(&mut h, EXPORT);
    h.key(Key::Enter, Modifiers::NONE);
    let expected =
        worldline_runtime::encode_replay_trace(&h.app.replay_debugger.saved_paths[0].trace)
            .unwrap();
    assert_eq!(h.app.replay_debugger.export_json, expected);
    h.focus_label(COPY, false);
    whole_control(&mut h, COPY);
    let output = h.frame(vec![key_event(Key::Enter, true)]);
    assert!(output.platform_output.commands.iter().any(
        |command| matches!(command, egui::OutputCommand::CopyText(text) if text == &expected)
    ));
    h.frame(vec![key_event(Key::Enter, false)]);
    for label in [EXPORT, CHANGES, OBSERVATION, REPLAY] {
        h.focus_label(label, true);
        whole_control(&mut h, label);
    }
    assert_eq!(h.stable(), before);
    assert!(h.app.replay_debugger.result.is_none());
}

fn key_event(key: Key, pressed: bool) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn assert_hidden_replay_stays_put(h: &mut Harness, before: &serde_json::Value) {
    h.frame(vec![]);
    let response = h.observed_response.as_ref().unwrap();
    let geometry = (response.rect, response.interact_rect);
    assert!(
        response.rect.top() >= response.interact_rect.bottom(),
        "negative precondition: actual Replay is wholly below the viewport: {geometry:?}",
    );
    for _ in 0..4 {
        let output = h.frame(vec![]);
        assert!(!rendered(&output).contains("use of widget ID"));
        let response = h.observed_response.as_ref().unwrap();
        assert_eq!((response.rect, response.interact_rect), geometry);
        assert_eq!(h.stable(), *before);
        assert!(h.app.replay_debugger.result.is_none());
    }
}

#[test]
fn ordinary_replay_short_200_keyboard_new_wheel_or_ime_blocks_focus_reveal() {
    for ime in [false, true] {
        let mut h = recorded();
        let before = h.stable();
        focus_time(&mut h);
        let mut events = vec![key_event(Key::Tab, true)];
        if ime {
            events.push(Event::Ime(egui::ImeEvent::Enabled));
        } else {
            events.extend([
                Event::PointerMoved(Pos2::new(270.0, 180.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, 180.0),
                    modifiers: Modifiers::NONE,
                },
            ]);
        }
        let pressed = h.frame(events);
        let mut release = vec![key_event(Key::Tab, false)];
        if ime {
            release.push(Event::Ime(egui::ImeEvent::Disabled));
        }
        let released = h.frame(release);
        assert!(
            pressed
                .platform_output
                .events
                .iter()
                .chain(&released.platform_output.events)
                .any(|event| {
                    matches!(event, egui::output::OutputEvent::FocusGained(info)
                    if info.label.as_deref() == Some(REPLAY))
                }),
            "the guard must be tested on real Replay focus"
        );
        h.observed = h.ctx.memory(|memory| memory.focused());
        for _ in 0..24 {
            h.frame(vec![]);
        }
        assert_hidden_replay_stays_put(&mut h, &before);
    }
}

#[test]
fn ordinary_replay_short_200_keyboard_later_wheel_and_ime_never_pull_focus_back() {
    let mut h = recorded();
    let before = h.stable();
    focus_time(&mut h);
    h.observed = Some(tab_to(&mut h, REPLAY, false));
    whole_control(&mut h, REPLAY);
    // Scroll away only after proving the original keyboard reveal succeeded.
    h.frame(vec![
        Event::PointerMoved(Pos2::new(270.0, 180.0)),
        Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 240.0),
            modifiers: Modifiers::NONE,
        },
    ]);
    for _ in 0..24 {
        h.frame(vec![]);
    }
    assert_hidden_replay_stays_put(&mut h, &before);
    h.frame(vec![Event::Ime(egui::ImeEvent::Enabled)]);
    h.frame(vec![Event::Ime(egui::ImeEvent::Preedit("候選".into()))]);
    h.frame(vec![Event::Ime(egui::ImeEvent::Disabled)]);
    assert_hidden_replay_stays_put(&mut h, &before);
}
