//! 完整原生尺寸换算与应用外壳；合成事件不冒充物理桌面验收。
use super::navigation::unchanged_state;
use super::*;
use egui::{Event, Pos2, Rect, Vec2};

struct Harness {
    ctx: egui::Context,
    app: WorldeditApp,
    physical: Vec2,
    viewport: Rect,
    time: f64,
}

impl Harness {
    fn new(translated: bool) -> Self {
        let (ctx, mut app) = fixture();
        app.active_file = app.project.entry.clone();
        app.saved_location = true;
        app.personal.pending_restore = false;
        app.personal.settings.appearance.ui_scale = 2.0;
        app.personal.settings.appearance.reduce_motion = true;
        app.replay_debugger.locale.enabled = translated;
        let mut harness = Self {
            ctx,
            app,
            physical: egui::vec2(763.0, 542.0),
            viewport: Rect::NOTHING,
            time: 0.0,
        };
        harness.settle();
        harness.assert_short_screen();
        harness
    }

    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.time += 1.0 / 60.0;
        let screen = Rect::from_min_size(Pos2::ZERO, self.physical / self.ctx.pixels_per_point());
        let mut raw = egui::RawInput {
            screen_rect: Some(screen),
            time: Some(self.time),
            modifiers: if events
                .iter()
                .any(|event| matches!(event, Event::Key { modifiers, .. } if modifiers.alt))
            {
                egui::Modifiers::ALT
            } else {
                egui::Modifiers::NONE
            },
            events,
            ..Default::default()
        };
        let viewport = raw.viewports.entry(egui::ViewportId::ROOT).or_default();
        viewport.native_pixels_per_point = Some(1.0);
        viewport.inner_rect = Some(screen);
        viewport.outer_rect = Some(screen);
        viewport.focused = Some(true);
        eframe::App::raw_input_hook(&mut self.app, &self.ctx, &mut raw);
        self.ctx.run(raw, |ctx| {
            eframe::App::update(&mut self.app, ctx, &mut eframe::Frame::_new_kittest());
        })
    }

    fn settle(&mut self) -> egui::FullOutput {
        for _ in 0..4 {
            self.frame(vec![]);
        }
        self.frame(vec![])
    }

    fn assert_short_screen(&mut self) {
        let output = self.settle();
        assert_eq!(self.ctx.zoom_factor(), 2.0);
        assert_eq!(self.ctx.native_pixels_per_point(), Some(1.0));
        assert_eq!(output.pixels_per_point, 2.0);
        assert_eq!(self.ctx.screen_rect().size(), egui::vec2(381.5, 271.0));
        assert_eq!(
            self.ctx.screen_rect().size() * output.pixels_per_point,
            self.physical
        );
        let (_, clip) =
            painted(&output, "普通试玩").expect("ordinary mode paints within main scroll");
        assert!(
            clip.top() > 80.0 && clip.height() < 150.0,
            "missing global chrome: {clip:?}"
        );
        assert!(
            clip.width() > 330.0,
            "a fixed side pane must not squeeze story: {clip:?}"
        );
        self.viewport = clip.intersect(self.ctx.screen_rect());
    }

    fn wheel(&mut self, point: Pos2, delta: f32) {
        self.frame(vec![
            Event::PointerMoved(point),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, delta),
                modifiers: egui::Modifiers::NONE,
            },
        ]);
    }

    fn visible(&mut self, label: &str) -> Pos2 {
        let mut from_top = false;
        let mut last = String::new();
        for _ in 0..160 {
            let output = self.frame(vec![]);
            if let Some((rect, clip)) = painted(&output, label) {
                let visible = rect.intersect(clip).intersect(self.ctx.screen_rect());
                if visible.is_positive()
                    && visible.height() + 0.5 >= rect.height().min(clip.height())
                    && visible.width() + 0.5 >= rect.width().min(clip.width())
                {
                    return visible.center();
                }
                last = format!("{label}: {rect:?}, clip={clip:?}");
                let delta = if rect.center().y < clip.center().y {
                    30.0
                } else {
                    -30.0
                };
                self.wheel(clip.center(), delta);
            } else {
                last = rendered(&output);
                self.wheel(
                    self.viewport.center(),
                    if from_top { -30.0 } else { 100_000.0 },
                );
                from_top = true;
            }
        }
        panic!("763×542 physical / 200% cannot reach {label}: {last}");
    }

    fn click(&mut self, label: &str) {
        let point = self.visible(label);
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        self.settle();
    }

    fn replace_locale_by_typing(&mut self, current: &str, replacement: &str) {
        self.click(current);
        let id = self
            .ctx
            .memory(|memory| memory.focused())
            .expect("locale input focus");
        let mut state = egui::TextEdit::load_state(&self.ctx, id).unwrap();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(current.chars().count()),
            )));
        state.store(&self.ctx, id);
        let mut expected = String::new();
        for ch in replacement.chars() {
            expected.push(ch);
            self.frame(vec![Event::Text(ch.to_string())]);
            self.frame(vec![]);
            assert_eq!(self.app.replay_debugger.locale.locale, expected);
            assert_eq!(self.ctx.memory(|memory| memory.focused()), Some(id));
        }
    }

    fn back(&mut self) {
        for pressed in [true, false] {
            self.frame(vec![Event::Key {
                key: egui::Key::ArrowLeft,
                physical_key: Some(egui::Key::ArrowLeft),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::ALT,
            }]);
        }
        self.settle();
        assert_eq!(self.app.tab, Tab::Play);
    }

    fn start(&mut self) {
        self.click("▶ 开始试玩");
        assert!(self
            .app
            .play
            .as_ref()
            .is_some_and(|play| play.error.is_none()));
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.app.project.root);
    }
}

fn painted(output: &egui::FullOutput, label: &str) -> Option<(Rect, Rect)> {
    fn find(shape: &egui::Shape, label: &str) -> Option<Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.galley.rect.translate(text.pos.to_vec2()))
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, label)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|shape| find(&shape.shape, label).map(|rect| (rect, shape.clip_rect)))
}

#[test]
fn ordinary_short_200_locale_typing_source_translation_return_and_choice_are_reachable() {
    let mut h = Harness::new(false);
    let baseline = h.app.project.content_baseline();
    h.click("译文");
    h.replace_locale_by_typing("zh-Hant", "fr");
    h.replace_locale_by_typing("fr", "zh-Hant");
    h.click("允许有标记的源文回退");
    assert!(h.app.replay_debugger.locale.fallback);
    h.click("允许有标记的源文回退");
    assert!(!h.app.replay_debugger.locale.fallback);
    h.start();
    let before = unchanged_state(&h.app);
    h.visible("实际语言 · zh-Hant");
    h.click("源译对照");
    assert!(h.app.replay_debugger.locale.parallel);
    let transcript = h.app.play.as_ref().unwrap().transcript.clone();
    h.visible(&transcript);
    h.visible("源文 · 同次求值");
    h.click("逐项来源、状态与修正");
    assert_eq!(unchanged_state(&h.app), before);
    h.click("查看此源文");
    assert_eq!(h.app.tab, Tab::Edit);
    assert_eq!(h.app.active_file, h.app.project.entry);
    let selection =
        egui::TextEdit::load_state(&h.ctx, egui::Id::new(("source", &h.app.active_file)))
            .unwrap()
            .cursor
            .char_range()
            .unwrap();
    assert_ne!(selection.primary.index, selection.secondary.index);
    h.back();
    assert_eq!(unchanged_state(&h.app), before);
    h.click("修正此译文");
    assert_eq!(h.app.tab, Tab::Localization);
    assert_eq!(h.app.localization_ui.target_locale, "zh-Hant");
    h.click("返回当前体验");
    assert_eq!(h.app.tab, Tab::Play);
    assert_eq!(unchanged_state(&h.app), before);
    h.click("选择：继续 👋");
    assert!(h.app.play.as_ref().unwrap().ended);
    h.visible("—— 世界线收束,故事结束 ——");
    assert_eq!(
        h.app
            .play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .replay_trace()
            .steps
            .len(),
        1
    );
    assert_eq!(h.app.project.content_baseline(), baseline);
}

#[test]
fn ordinary_short_200_pause_stop_inspection_record_and_replay_keep_real_story() {
    let mut h = Harness::new(true);
    h.start();
    let before = unchanged_state(&h.app);
    h.click("Ⅱ 暂停");
    assert!(h.app.play.as_ref().unwrap().paused);
    h.click("普通试玩预算（每次推进）");
    h.visible("步数上限");
    h.click("普通试玩预算（每次推进）");
    h.click("▶ 继续");
    assert!(!h.app.play.as_ref().unwrap().paused);
    h.click("⌕ 状态检查：查值与变化…");
    assert!(h.app.replay_debugger.inspection.open);
    h.visible("全局变量 · n");
    h.visible("当前");
    h.click("返回试玩与选择");
    assert!(!h.app.replay_debugger.inspection.open);
    assert_eq!(unchanged_state(&h.app), before);
    h.click("■ 停止");
    assert!(h.app.play.as_ref().unwrap().stopped);
    assert_eq!(unchanged_state(&h.app), before);
    h.click("↻ 重新开始（已应用稿）");
    assert!(!h.app.play.as_ref().unwrap().stopped);
    assert_eq!(unchanged_state(&h.app), before);
    h.click("选择：继续 👋");
    assert!(h.app.play.as_ref().unwrap().ended);
    let ended = unchanged_state(&h.app);
    h.click("调试与路径");
    h.click("● 保存当前路径");
    assert_eq!(h.app.replay_debugger.saved_paths.len(), 1);
    assert!(h.app.replay_debugger.saved_paths[0].trace.complete);
    h.click("▶ 重放所选路径");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while h.app.replay_debugger.job.is_some() {
        h.frame(vec![]);
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(matches!(
        h.app.replay_debugger.result.as_ref().unwrap().status,
        worldline_runtime::ReplayStatus::Replayed {
            ended: true,
            complete: true
        }
    ));
    assert_eq!(unchanged_state(&h.app), ended);
}

#[test]
fn ordinary_short_200_source_only_and_wide_layout_switch_keep_session_identity() {
    let mut h = Harness::new(false);
    h.start();
    let before = unchanged_state(&h.app);
    assert!(h.app.play.as_ref().unwrap().localized_outputs.is_empty());
    let play = h.app.play.as_ref().unwrap();
    let transcript = play.transcript.clone();
    assert_eq!(play.transcript_links.len(), 1);
    let link = play.transcript_links[0].clone();
    assert_eq!(
        link.target,
        worldline_core::TargetRef {
            kind: "event".into(),
            id: "start".into()
        }
    );
    assert_eq!(&transcript[link.start..link.end], "Harbor");
    assert!(transcript[..link.start].starts_with("Hello "));
    assert_eq!(&transcript[link.end..], "");
    // Source-only output deliberately lays out real links separately from its surrounding prose.
    h.visible(&transcript[..link.start]);
    h.visible("Harbor");
    let shown = h.frame(vec![]);
    for label in [&transcript[..link.start], "Harbor"] {
        let (text, clip) = shown
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some((text, shape.clip_rect))
                }
                _ => None,
            })
            .expect("same evaluated prose and link both paint");
        for (index, row) in text.galley.rows.iter().enumerate() {
            let row = if index == 0 {
                row.rect_without_leading_space()
            } else {
                row.rect()
            };
            let rect = row.translate(text.pos.to_vec2());
            let visible = rect.intersect(clip).intersect(h.ctx.screen_rect());
            assert!(
                visible.is_positive()
                    && visible.height() + 0.5 >= rect.height()
                    && visible.width() + 0.5 >= rect.width(),
                "source glyphs must actually be visible: {label}"
            );
        }
    }
    assert_eq!(unchanged_state(&h.app), before);
    h.physical = egui::vec2(1600.0, 1000.0);
    h.app.personal.settings.appearance.ui_scale = 1.0;
    let output = h.settle();
    assert_eq!(h.ctx.pixels_per_point(), 1.0);
    let (choice, _) = painted(&output, "选择").unwrap();
    let (mode, _) = painted(&output, "普通试玩").unwrap();
    assert!(choice.left() > 1000.0 && mode.left() < 800.0);
    assert_eq!(unchanged_state(&h.app), before);
    h.physical = egui::vec2(763.0, 542.0);
    h.app.personal.settings.appearance.ui_scale = 2.0;
    h.settle();
    h.wheel(egui::pos2(190.0, 175.0), 100_000.0);
    h.assert_short_screen();
    assert_eq!(unchanged_state(&h.app), before);
    h.click("选择：Continue");
    assert!(h.app.play.as_ref().unwrap().ended);
    assert!(h
        .app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .presentation_identity()
        .is_none());
}
