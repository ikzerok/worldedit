//! Native frame cadence varies independently from real pointer/key event delivery.
use super::*;

#[test]
fn localization_short_keyboard_advanced_actions_at_fast_native_frame_cadence() {
    for seconds in [1.0 / 240.0, 1.0 / 120.0] {
        eprintln!("advanced native frame_seconds={seconds}");
        let mut h = Harness::new(SOURCE);
        h.frame_seconds = seconds;
        advanced_actions(h);
    }
}

#[test]
fn localization_short_keyboard_advanced_actions_at_delayed_native_frame_cadence() {
    for seconds in [1.0 / 30.0, 0.1] {
        eprintln!("advanced native frame_seconds={seconds}");
        let mut h = Harness::new(SOURCE);
        h.frame_seconds = seconds;
        advanced_actions(h);
    }
}

#[test]
fn localization_short_keyboard_advanced_actions_with_spacious_controls() {
    let mut h = Harness::new(SOURCE);
    h.app.personal.settings.appearance.density = crate::theme::Density::Spacious;
    for _ in 0..4 {
        h.frame(vec![]);
    }
    advanced_actions(h);
}

#[test]
fn localization_short_keyboard_advanced_actions_in_other_supported_layouts() {
    for style in [
        crate::theme::StylePreset::Manuscript,
        crate::theme::StylePreset::Technical,
        crate::theme::StylePreset::Focus,
        crate::theme::StylePreset::Ledger,
    ] {
        eprintln!("advanced supported layout={style:?}");
        let mut h = Harness::new(SOURCE);
        h.app.personal.settings.appearance.style = style;
        for _ in 0..4 {
            h.frame(vec![]);
        }
        advanced_actions(h);
    }
}

#[test]
fn localization_short_keyboard_repeated_navigation_reveals_the_same_target_again() {
    let (mut h, field) = typed_detail();
    h.trace_frames = true;
    let baseline = h.app.project.content_baseline();
    let history = h.app.history.len();
    let mut target = None;
    for round in 0..3 {
        let add = tab(&mut h, "添加文字段", false);
        if let Some(id) = target {
            // A later user scroll establishes the next starting position. Once Tab is pressed,
            // only keyboard/empty frames may bring the same target back into view.
            h.wheel(h.scroll_point(), 10_000.0);
            assert_eq!(
                focused(&h),
                add,
                "wheel setup must preserve the real origin"
            );
            let widget = current_widget(&h, id);
            assert!(
                !widget.interact_rect.contains_rect(widget.rect),
                "round {round} must need a fresh reveal for the same target"
            );
        }
        let keep = tab(&mut h, "保留此译文并待复核", false);
        if let Some(id) = target {
            assert_eq!(keep, id, "repeat navigation must use the same real control");
        }
        target = Some(keep);
        tab(&mut h, "添加文字段", true);
        tab_field(
            &mut h,
            field,
            "return to translation before another Tab",
            true,
        );
        assert_typed_unchanged(&h, &baseline, history);
    }
}

#[test]
fn localization_short_keyboard_new_ime_cancels_pending_reverse_reveal() {
    let (mut h, field) = typed_detail();
    h.trace_frames = true;
    let baseline = h.app.project.content_baseline();
    let history = h.app.history.len();
    tab(&mut h, "添加文字段", false);
    tab(&mut h, "保留此译文并待复核", false);
    let add = tab(&mut h, "添加文字段", true);
    let before = current_widget(&h, field);
    assert!(
        !before.interact_rect.contains_rect(before.rect),
        "reverse destination must actually need a reveal"
    );
    h.frame(vec![Event::Key {
        key: Key::Tab,
        physical_key: Some(Key::Tab),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::SHIFT,
    }]);
    assert_eq!(
        focused(&h),
        add,
        "Shift+Tab schedules its next-frame destination"
    );
    h.frame(vec![
        Event::Key {
            key: Key::Tab,
            physical_key: Some(Key::Tab),
            pressed: false,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        },
        Event::Ime(egui::ImeEvent::Enabled),
    ]);
    for _ in 0..8 {
        h.frame(vec![]);
    }
    assert_eq!(
        focused(&h),
        field,
        "IME does not fabricate a focus transfer"
    );
    let after = current_widget(&h, field);
    assert!(
        !after.interact_rect.contains_rect(after.rect)
            && after.rect.min.distance(before.rect.min) < 0.5,
        "new IME intent must cancel the older full-field reveal: before={before:?}, after={after:?}, frames={:#?}",
        h.frame_trace
    );
    h.frame(vec![Event::Ime(egui::ImeEvent::Disabled)]);
    tab(&mut h, "添加文字段", false);
    tab_field(
        &mut h,
        field,
        "new navigation after IME can reveal again",
        true,
    );
    assert_typed_unchanged(&h, &baseline, history);
}
