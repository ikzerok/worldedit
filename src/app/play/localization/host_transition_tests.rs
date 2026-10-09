//! Real full-App host transitions must not register the same settings twice in a frame.
use super::*;

fn no_id_clash(output: &egui::FullOutput, phase: &str) {
    let text = rendered(output);
    let clashes: Vec<_> = text
        .lines()
        .filter(|line| line.contains("use of widget ID"))
        .collect();
    assert!(clashes.is_empty(), "{phase}: {clashes:?}");
}

fn next_locale(h: &Harness) -> serde_json::Value {
    let locale = &h.app.replay_debugger.locale;
    serde_json::json!([
        locale.enabled,
        locale.locale,
        locale.fallback,
        locale.parallel
    ])
}

fn host_diagnostic(h: &Harness, output: &egui::FullOutput, phase: &str) -> String {
    // Responses are captured before end_pass swaps the widget registries.
    let ordinary = h.observed_response.as_ref().map(|response| {
        (
            response.id,
            response.rect,
            response.interact_rect,
            response.has_focus(),
        )
    });
    let focused = h.focused.as_ref().map(|response| {
        (
            response.id,
            response.rect,
            response.interact_rect,
            response.has_focus(),
        )
    });
    format!(
        "{phase}: frame={}, physical={:?}, running={}, comparison={}, screen={:?}, focused={focused:?}, ordinary={ordinary:?}, mode_glyphs={:?}, painted={}",
        h.ctx.cumulative_frame_nr(), h.physical, h.app.play.is_some(), h.app.comparison.active,
        h.ctx.screen_rect(), painted(output, "普通试玩"), rendered(output),
    )
}

fn comparison_fixture(physical: egui::Vec2, running: bool) -> Harness {
    let mut h = Harness::new();
    h.ctx.options_mut(|options| options.warn_on_id_clash = true);
    h.translated();
    if running {
        h.focus_label("▶ 开始试玩", false);
        h.key(Key::Enter, Modifiers::NONE);
        assert!(h.app.play.as_ref().is_some_and(|play| {
            !play.localized_outputs.is_empty()
                && play
                    .story
                    .as_ref()
                    .unwrap()
                    .presentation_identity()
                    .is_some()
        }));
    }
    h.physical = physical;
    h.app.comparison.active = true;
    let context = egui::Id::new((&h.app.project.root, h.app.version));
    let semantic_ui = egui::Id::new(("ordinary-play-setting", Some(context), "mode-ordinary"));
    h.observed = Some(egui::Id::new(semantic_ui.with("auto").value()));
    for _ in 0..4 {
        let output = h.frame(vec![]);
        no_id_clash(&output, "comparison fixture");
        let ordinary = h
            .observed_response
            .as_ref()
            .expect("actual comparison header mode response");
        assert!(painted(&output, "普通试玩")
            .iter()
            .any(|(rect, _)| { ordinary.rect.contains(rect.center()) }));
    }
    assert!(h.app.comparison.active);
    assert_eq!(h.ctx.screen_rect().size(), physical / 2.0);
    h
}

fn verify_return(h: &mut Harness, before: &serde_json::Value, locale: &serde_json::Value) {
    assert!(
        !h.app.comparison.active,
        "the real mode control must switch hosts"
    );
    for frame in 0..3 {
        let output = h.frame(vec![]);
        no_id_clash(&output, "ordinary host after transition");
        let response = h.observed_response.as_ref().unwrap_or_else(|| {
            panic!(
                "{}",
                host_diagnostic(h, &output, &format!("ordinary settled frame {frame}"))
            )
        });
        assert_eq!(Some(response.id), h.observed);
        assert!(response.enabled() && response.rect.is_positive() && response.rect.is_finite());
        assert_eq!(h.stable(), *before, "switching host cannot execute a story");
        assert_eq!(next_locale(h), *locale);
    }
}

#[test]
fn ordinary_settings_host_transition_mouse_comparison_to_play_has_unique_ids_and_keeps_session() {
    for physical in [egui::vec2(763.0, 542.0), egui::vec2(2560.0, 1800.0)] {
        for running in [false, true] {
            let mut h = comparison_fixture(physical, running);
            let before = h.stable();
            let locale = next_locale(&h);
            let output = h.frame(vec![]);
            let (rect, clip) = painted(&output, "普通试玩")
                .into_iter()
                .find(|(rect, clip)| clip.intersect(h.ctx.screen_rect()).contains_rect(*rect))
                .expect("comparison header's actual ordinary mode label must be visible");
            assert!(clip.contains_rect(rect));
            let point = rect.center();
            no_id_clash(&h.frame(vec![Event::PointerMoved(point)]), "mouse hover");
            for pressed in [true, false] {
                let output = h.frame(vec![Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                }]);
                assert_eq!(h.stable(), before);
                assert_eq!(next_locale(&h), locale);
                no_id_clash(
                    &output,
                    &format!("mouse pressed={pressed}, {physical:?}, running={running}"),
                );
            }
            verify_return(&mut h, &before, &locale);
        }
    }
}

#[test]
fn ordinary_settings_host_transition_keyboard_comparison_to_play_has_unique_ids_and_keeps_session()
{
    for physical in [egui::vec2(763.0, 542.0), egui::vec2(2560.0, 1800.0)] {
        for running in [false, true] {
            let mut h = comparison_fixture(physical, running);
            let before = h.stable();
            let locale = next_locale(&h);
            let ordinary = h.focus_label("普通试玩", false);
            h.assert_visible("普通试玩");
            assert_eq!(h.stable(), before);
            for pressed in [true, false] {
                let output = h.frame(vec![Event::Key {
                    key: Key::Enter,
                    physical_key: Some(Key::Enter),
                    pressed,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }]);
                assert_eq!(h.stable(), before);
                assert_eq!(next_locale(&h), locale);
                no_id_clash(
                    &output,
                    &format!("keyboard pressed={pressed}, {physical:?}, running={running}"),
                );
                if pressed {
                    for repeat in 0..2 {
                        let output = h.frame(vec![Event::Key {
                            key: Key::Enter,
                            physical_key: Some(Key::Enter),
                            pressed: true,
                            repeat: true,
                            modifiers: Modifiers::NONE,
                        }]);
                        no_id_clash(&output, "held Enter after switching host");
                        assert!(!h.app.comparison.active);
                        let response = h
                            .observed_response
                            .as_ref()
                            .expect("current ordinary mode response");
                        assert_eq!(response.id, ordinary);
                        assert!(response.has_focus());
                        if repeat == 1 {
                            // One ordinary layout frame may apply the return scroll target.
                            let visible = response
                                .rect
                                .intersect(response.interact_rect)
                                .intersect(h.ctx.screen_rect());
                            assert!(
                                visible.is_positive()
                                    && visible.width() + 0.5 >= response.rect.width()
                                    && visible.height() + 0.5 >= response.rect.height(),
                                "{}",
                                host_diagnostic(&h, &output, "keyboard return next layout")
                            );
                            assert!(
                                painted(&output, "普通试玩").iter().any(|(rect, clip)| {
                                    response.rect.contains(rect.center())
                                        && clip.intersect(h.ctx.screen_rect()).contains_rect(*rect)
                                }),
                                "{}",
                                host_diagnostic(&h, &output, "keyboard return glyphs")
                            );
                        }
                        assert_eq!(h.stable(), before);
                        assert_eq!(next_locale(&h), locale);
                    }
                }
            }
            verify_return(&mut h, &before, &locale);
            assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(ordinary));
            assert!(h.focused.as_ref().is_some_and(|response| {
                response.id == ordinary && response.has_focus() && response.enabled()
            }));
            let gained = h.tab(false);
            assert!(
                gained
                    .iter()
                    .any(|info| info.label.as_deref() == Some("路线对照")),
                "{gained:?}"
            );
            h.assert_visible("路线对照");
            let gained = h.tab(true);
            assert!(
                gained
                    .iter()
                    .any(|info| info.label.as_deref() == Some("普通试玩")),
                "{gained:?}"
            );
            h.assert_visible("普通试玩");
            assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(ordinary));
            assert!(!h.app.comparison.active);
            assert_eq!(h.stable(), before);
            assert_eq!(next_locale(&h), locale);
        }
    }
}

fn comparison_fixture_after_real_scroll() -> (Harness, (Rect, Rect)) {
    let mut h = Harness::new();
    h.ctx.options_mut(|options| options.warn_on_id_clash = true);
    h.translated();
    // Locale focus need not hide the whole mode row when its actions wrap compactly.
    // Establish the cancellation precondition through real keyboard navigation instead.
    h.focus_label("▶ 开始试玩", false);
    h.assert_visible("▶ 开始试玩");
    let context = egui::Id::new((&h.app.project.root, h.app.version));
    let semantic_ui = egui::Id::new(("ordinary-play-setting", Some(context), "mode-ordinary"));
    h.observed = Some(egui::Id::new(semantic_ui.with("auto").value()));
    for _ in 0..3 {
        no_id_clash(
            &h.frame(vec![]),
            "settled ordinary scroll before changing hosts",
        );
    }
    // Captured after App::update and before end_pass: these are this completed frame's
    // actual widget and clipping rectangles, without requesting focus or writing scroll state.
    let response = h.observed_response.as_ref().unwrap();
    let previous = (response.rect, response.interact_rect);
    assert!(previous.0.bottom() < previous.1.top(), "{previous:?}");
    h.app.comparison.active = true;
    for _ in 0..4 {
        no_id_clash(&h.frame(vec![]), "comparison after real ordinary scroll");
    }
    (h, previous)
}

#[test]
fn ordinary_settings_host_transition_new_pointer_or_ime_cancels_the_one_time_reveal() {
    for interruption in [
        Event::PointerMoved(Pos2::new(270.0, 180.0)),
        Event::Ime(egui::ImeEvent::Enabled),
    ] {
        let (mut h, previous_geometry) = comparison_fixture_after_real_scroll();
        let before = h.stable();
        let locale = next_locale(&h);
        h.focus_label("普通试玩", false);
        let output = h.frame(vec![Event::Key {
            key: Key::Enter,
            physical_key: Some(Key::Enter),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        no_id_clash(&output, "keyboard switch before newer intent");
        assert!(!h.app.comparison.active);
        no_id_clash(&h.frame(vec![interruption]), "new pointer or IME wins");
        h.frame(vec![
            Event::Key {
                key: Key::Enter,
                physical_key: Some(Key::Enter),
                pressed: false,
                repeat: false,
                modifiers: Modifiers::NONE,
            },
            Event::Ime(egui::ImeEvent::Disabled),
        ]);
        for _ in 0..3 {
            let output = h.frame(vec![]);
            no_id_clash(&output, "cancelled host reveal stays cancelled");
            let response = h
                .observed_response
                .as_ref()
                .expect("current ordinary response");
            assert!(
                response.rect.bottom() < response.interact_rect.top(),
                "{}",
                host_diagnostic(&h, &output, "new intent must retain the prior scroll")
            );
            assert_eq!(
                (response.rect, response.interact_rect),
                previous_geometry,
                "{}",
                host_diagnostic(
                    &h,
                    &output,
                    "cancelled reveal must not change prior geometry"
                )
            );
            assert_eq!(h.stable(), before);
            assert_eq!(next_locale(&h), locale);
        }
    }
}

#[test]
fn ordinary_settings_host_transition_successful_reveal_does_not_follow_later_mouse_scroll() {
    let mut h = comparison_fixture(egui::vec2(763.0, 542.0), false);
    let before = h.stable();
    let locale = next_locale(&h);
    let ordinary = h.focus_label("普通试玩", false);
    h.key(Key::Space, Modifiers::NONE);
    assert!(!h.app.comparison.active);
    h.assert_visible("普通试玩");
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(ordinary));
    // This is a negative check of later user scrolling, never a way to repair the keyboard path.
    h.frame(vec![
        Event::PointerMoved(Pos2::new(270.0, 180.0)),
        Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -250.0),
            modifiers: Modifiers::NONE,
        },
    ]);
    for _ in 0..12 {
        h.frame(vec![]);
    }
    for _ in 0..3 {
        let output = h.frame(vec![]);
        no_id_clash(&output, "later mouse scroll");
        let response = h
            .observed_response
            .as_ref()
            .expect("current ordinary response");
        assert!(
            response.rect.bottom() < response.interact_rect.top(),
            "{}",
            host_diagnostic(&h, &output, "consumed reveal must not pull back")
        );
        assert_eq!(h.stable(), before);
        assert_eq!(next_locale(&h), locale);
    }
}
