//! Pure keyboard navigation through full App frames; no requested focus or synthetic scrolling.
use super::*;
use egui::{Event, Key, Modifiers, Pos2, Rect};

struct Harness {
    ctx: egui::Context,
    app: WorldeditApp,
    physical: egui::Vec2,
    time: f64,
    focused: Option<egui::Response>,
    observed: Option<egui::Id>,
    observed_response: Option<egui::Response>,
}
impl Harness {
    fn new() -> Self {
        let (ctx, mut app) = fixture();
        app.active_file = app.project.entry.clone();
        app.saved_location = true;
        app.personal.pending_restore = false;
        app.personal.settings.appearance.ui_scale = 2.0;
        app.personal.settings.appearance.reduce_motion = true;
        app.replay_debugger.locale.enabled = false;
        app.replay_debugger.seed = 1;
        let mut h = Self {
            ctx,
            app,
            physical: egui::vec2(763.0, 542.0),
            time: 0.0,
            focused: None,
            observed: None,
            observed_response: None,
        };
        for _ in 0..5 {
            h.frame(vec![]);
        }
        assert_eq!(h.ctx.screen_rect().size(), egui::vec2(381.5, 271.0));
        assert_eq!(h.ctx.pixels_per_point(), 2.0);
        assert_eq!(h.ctx.native_pixels_per_point(), Some(1.0));
        h
    }
    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.time += 1.0 / 60.0;
        let screen = Rect::from_min_size(Pos2::ZERO, self.physical / self.ctx.pixels_per_point());
        let modifiers = events
            .iter()
            .find_map(|event| match event {
                Event::Key { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or(Modifiers::NONE);
        let mut raw = egui::RawInput {
            screen_rect: Some(screen),
            time: Some(self.time),
            modifiers,
            events,
            ..Default::default()
        };
        let viewport = raw.viewports.entry(egui::ViewportId::ROOT).or_default();
        viewport.native_pixels_per_point = Some(1.0);
        viewport.inner_rect = Some(screen);
        viewport.outer_rect = Some(screen);
        viewport.focused = Some(true);
        eframe::App::raw_input_hook(&mut self.app, &self.ctx, &mut raw);
        let mut focused = None;
        let mut observed_response = None;
        let output = self.ctx.run(raw, |ctx| {
            eframe::App::update(&mut self.app, ctx, &mut eframe::Frame::_new_kittest());
            focused = ctx
                .memory(|memory| memory.focused())
                .and_then(|id| ctx.read_response(id));
            observed_response = self.observed.and_then(|id| ctx.read_response(id));
        });
        self.focused = focused;
        self.observed_response = observed_response;
        output
    }
    fn key(&mut self, key: Key, modifiers: Modifiers) -> Vec<egui::WidgetInfo> {
        let mut gained = Vec::new();
        for pressed in [true, false] {
            let output = self.frame(vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            }]);
            gained.extend(output.platform_output.events.into_iter().filter_map(
                |event| match event {
                    egui::output::OutputEvent::FocusGained(info) => Some(info),
                    _ => None,
                },
            ));
        }
        // Reverse focus can be handed over at end_pass, so read the following actual frame too.
        let output = self.frame(vec![]);
        gained.extend(
            output
                .platform_output
                .events
                .into_iter()
                .filter_map(|event| match event {
                    egui::output::OutputEvent::FocusGained(info) => Some(info),
                    _ => None,
                }),
        );
        gained
    }
    fn tab(&mut self, reverse: bool) -> Vec<egui::WidgetInfo> {
        self.key(
            Key::Tab,
            if reverse {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
        )
    }
    fn focus_label(&mut self, label: &str, reverse: bool) -> egui::Id {
        for _ in 0..180 {
            let gained = self.tab(reverse);
            if gained
                .iter()
                .any(|info| info.label.as_deref() == Some(label))
            {
                return self
                    .ctx
                    .memory(|memory| memory.focused())
                    .expect("real Tab focus");
            }
        }
        panic!("Tab cannot reach {label}");
    }
    fn translated(&mut self) {
        self.focus_label("译文", false);
        self.key(Key::Enter, Modifiers::NONE);
        assert!(self.app.replay_debugger.locale.enabled);
        let gained = self.tab(false);
        assert!(
            gained
                .iter()
                .any(|info| info.current_text_value.as_deref() == Some("zh-Hant")),
            "{gained:?}"
        );
    }
    fn assert_visible(&mut self, expected: &str) {
        let output = self.frame(vec![]);
        let response = self.focused.as_ref().expect("focused control still exists");
        assert!(response.has_focus() && response.enabled());
        let visible = response
            .rect
            .intersect(response.interact_rect)
            .intersect(self.ctx.screen_rect());
        assert!(visible.is_positive()
            && visible.width() + 0.5 >= response.rect.width()
            && visible.height() + 0.5 >= response.rect.height(),
            "keyboard focus must reveal full control {expected}: rect={:?}, interact={:?}, screen={:?}",
            response.rect, response.interact_rect, self.ctx.screen_rect());
        let glyphs = painted(&output, expected);
        assert!(
            glyphs.iter().any(|(rect, clip)| {
                response.rect.contains(rect.center())
                    && clip.intersect(self.ctx.screen_rect()).contains_rect(*rect)
            }),
            "focused {expected} must paint real visible glyphs: rect={:?}, glyphs={glyphs:?}",
            response.rect
        );
    }
    fn stable(&self) -> serde_json::Value {
        let play = self.app.play.as_ref().map(|play| {
            let story = play.story.as_ref().unwrap();
            serde_json::json!({
                "save": story.save().unwrap(), "trace": story.replay_trace(),
                "locale": story.presentation_identity(), "transcript": play.transcript,
                "outputs": play.localized_outputs, "ended": play.ended,
            })
        });
        serde_json::json!({
            "baseline": self.app.project.content_baseline(), "version": self.app.version,
            "dirty": self.app.project.is_dirty(), "undo": self.app.history.len(), "redo": self.app.redo.len(),
            "source": self.app.project.document(&self.app.project.entry).unwrap(),
            "disk": std::fs::read(&self.app.project.entry).unwrap(), "play": play,
            "seed": self.app.replay_debugger.seed, "steps": self.app.replay_debugger.live_max_steps,
            "time": self.app.replay_debugger.live_time_budget_ms,
            "saved_paths": self.app.replay_debugger.saved_paths.len(),
        })
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.app.project.root);
    }
}
fn painted(output: &egui::FullOutput, expected: &str) -> Vec<(Rect, Rect)> {
    fn gather(shape: &egui::Shape, expected: &str, clip: Rect, result: &mut Vec<(Rect, Rect)>) {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == expected => {
                for (index, row) in text.galley.rows.iter().enumerate() {
                    let rect = if index == 0 {
                        row.rect_without_leading_space()
                    } else {
                        row.rect()
                    };
                    result.push((rect.translate(text.pos.to_vec2()), clip));
                }
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    gather(shape, expected, clip, result);
                }
            }
            _ => {}
        }
    }
    let mut result = Vec::new();
    for shape in &output.shapes {
        gather(&shape.shape, expected, shape.clip_rect, &mut result);
    }
    result
}

#[test]
fn ordinary_short_200_keyboard_locale_policy_seed_tab_reveals_without_starting_story() {
    let mut h = Harness::new();
    let before = h.stable();
    h.translated();
    let field = h.ctx.memory(|memory| memory.focused()).unwrap();
    h.key(
        Key::A,
        Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        },
    );
    let mut value = String::new();
    for character in "zh-Hant".chars() {
        value.push(character);
        h.frame(vec![Event::Text(character.to_string())]);
        assert_eq!(h.app.replay_debugger.locale.locale, value);
        assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(field));
    }
    let gained = h.tab(false);
    assert!(
        gained
            .iter()
            .any(|info| info.label.as_deref() == Some("允许有标记的源文回退")),
        "{gained:?}"
    );
    let gained = h.tab(false);
    assert!(
        gained
            .iter()
            .any(|info| info.value == Some(1.0) || info.current_text_value.as_deref() == Some("1")),
        "{gained:?}"
    );
    assert_eq!(h.stable(), before);
    h.assert_visible("1");
    assert!(h.app.play.is_none());
}

#[test]
fn ordinary_short_200_keyboard_seed_to_policy_shift_tab_reveals_without_mouse_scroll() {
    let mut h = Harness::new();
    let before = h.stable();
    h.translated();
    // Existing Start keyboard handling scrolls to the lower group without any helper scroll.
    h.focus_label("▶ 开始试玩", false);
    h.assert_visible("▶ 开始试玩");
    h.focus_label("允许有标记的源文回退", true);
    assert_eq!(h.stable(), before);
    h.assert_visible("允许有标记的源文回退");
    let gained = h.tab(true);
    assert!(
        gained
            .iter()
            .any(|info| info.current_text_value.as_deref() == Some("zh-Hant")),
        "{gained:?}"
    );
    h.assert_visible("zh-Hant");
    assert!(h.app.play.is_none());
}

#[test]
fn ordinary_short_200_keyboard_expanded_budget_values_reveal_in_both_directions() {
    let mut h = Harness::new();
    let before = h.stable();
    h.translated();
    h.focus_label("普通试玩预算（每次推进）", false);
    h.key(Key::Enter, Modifiers::NONE);
    let gained = h.tab(false);
    assert!(
        gained.iter().any(|info| info.value == Some(100_000.0)
            || info.current_text_value.as_deref() == Some("100000")),
        "{gained:?}"
    );
    h.assert_visible("100000");
    let gained = h.tab(false);
    assert!(
        gained
            .iter()
            .any(|info| info.value == Some(250.0)
                || info.current_text_value.as_deref() == Some("250")),
        "{gained:?}"
    );
    h.assert_visible("250");
    h.tab(true);
    h.assert_visible("100000");
    assert_eq!(h.stable(), before);
    assert!(h.app.play.is_none());
}

#[test]
fn ordinary_short_200_keyboard_navigation_keeps_real_locale_story_rng_and_trace() {
    let mut h = Harness::new();
    h.translated();
    h.focus_label("▶ 开始试玩", false);
    h.key(Key::Enter, Modifiers::NONE);
    assert!(h
        .app
        .play
        .as_ref()
        .is_some_and(|play| !play.localized_outputs.is_empty()));
    let before = h.stable();
    h.focus_label("允许有标记的源文回退", true);
    h.assert_visible("允许有标记的源文回退");
    h.tab(true);
    h.assert_visible("zh-Hant");
    h.tab(false);
    h.assert_visible("允许有标记的源文回退");
    assert_eq!(h.stable(), before);
}

#[test]
fn ordinary_short_200_keyboard_seed_identity_survives_dynamic_notice_while_typing() {
    let mut h = Harness::new();
    h.translated();
    h.tab(false);
    h.tab(false);
    h.assert_visible("1");
    let id = h.ctx.memory(|memory| memory.focused()).unwrap();
    let baseline = h.app.project.content_baseline();
    let old = h
        .app
        .project
        .document(&h.app.project.entry)
        .unwrap()
        .to_owned();
    // The startup notice is a dynamic sibling immediately before the seed row.
    let request = LocalizationPresentationRequest {
        schema_version: 1,
        target_locale: "fr".into(),
        policy: LocalizationPresentationPolicy::Strict,
    };
    h.app.replay_debugger.notice = Some(
        h.app
            .project
            .prepare_localization_presentation(&request)
            .unwrap_err()
            .to_string(),
    );
    h.frame(vec![]);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(id));
    h.key(
        Key::A,
        Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        },
    );
    for (character, expected) in [('1', 1), ('7', 17)] {
        h.frame(vec![Event::Text(character.to_string())]);
        assert_eq!(h.app.replay_debugger.seed, expected);
        assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(id));
    }
    assert!(h.app.play.is_none());
    assert!(h.app.replay_debugger.saved_paths.is_empty());
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert_eq!(h.app.project.document(&h.app.project.entry).unwrap(), old);
    assert!(!h.app.project.is_dirty());
}

#[path = "host_transition_tests.rs"]
mod host_transition;

#[path = "mode_action_layout_tests.rs"]
mod mode_action_layout;

#[path = "replay_control_visibility_tests.rs"]
mod replay_control_visibility;
