//! 完整 WorldeditApp 多帧合成输入；不冒充物理键盘、鼠标或 IME 验收。
use super::*;
use crate::app::SavedReplayPath;
use egui::{Event, Key, Modifiers};
use worldline_core::project::Project;

#[path = "interruptions.rs"]
mod interruptions;

const NORMAL: &str = "event start\n  choice \"第一步\"\n    -> next\nevent next\n  choice \"第二步\"\n    -> finish\nevent finish\n  choice \"第三步\"\n    -> END\n";
const LOCKED: &str = "event start\n  choice \"第一步\"\n    -> next\nevent next\n  choice \"不可走\" enable false disabled \"未解锁\"\n    -> END\n  choice \"可以走\"\n    -> END\n";

struct Harness {
    ctx: egui::Context,
    app: WorldeditApp,
    size: egui::Vec2,
    frame_focus: Option<egui::Response>,
}
impl Harness {
    fn new(source: &str) -> Self {
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.0);
        let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        let root = std::env::temp_dir().join(format!(
            "play-keyboard-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        app.project = Project::new(&root);
        app.active_file = app.project.entry.clone();
        app.project
            .documents
            .retain(|path, _| path == &app.active_file);
        app.project
            .set_text(&app.active_file.clone(), source.into())
            .unwrap();
        app.project
            .create_authoring_document(
                &root.join(".world/project.json"),
                br#"{"schema_version":1,"language_version":"1.12","required_features":[]}"#
                    .to_vec(),
            )
            .unwrap();
        app.recompile();
        assert!(
            !app.snapshot.as_ref().unwrap().result.has_errors(),
            "{:?}",
            app.snapshot.as_ref().unwrap().result.diagnostics
        );
        app.personal.pending_restore = false;
        app.tab = Tab::Play;
        let mut h = Self {
            ctx,
            app,
            size: egui::vec2(1188.0, 848.0),
            frame_focus: None,
        };
        h.idle();
        h
    }
    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        let mut focus = None;
        let output = self.ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                events,
                ..Default::default()
            },
            |ctx| {
                eframe::App::update(&mut self.app, ctx, &mut eframe::Frame::_new_kittest());
                // end_pass 会交换 this_pass/prev_pass；必须在交换前取当前绘制帧几何。
                focus = ctx
                    .memory(|memory| memory.focused())
                    .and_then(|id| ctx.read_response(id));
            },
        );
        self.frame_focus = focus;
        output
    }
    fn idle(&mut self) -> egui::FullOutput {
        self.frame(vec![])
    }
    fn press(&mut self, key: Key) {
        self.frame(vec![key_event(key, true, false, Modifiers::NONE)]);
    }
    fn release(&mut self, key: Key) {
        self.frame(vec![key_event(key, false, false, Modifiers::NONE)]);
    }
    fn key(&mut self, key: Key) {
        self.press(key);
        self.release(key);
    }
    fn steps(&self) -> usize {
        self.app
            .play
            .as_ref()
            .and_then(|p| p.story.as_ref())
            .map_or(0, |story| story.replay_trace().steps.len())
    }
    fn focused(&self) -> Option<egui::Id> {
        self.ctx.memory(|m| m.focused())
    }
    fn focus(&mut self, label: &str) -> egui::Id {
        for _ in 0..200 {
            let output = self.frame(vec![key_event(Key::Tab, true, false, Modifiers::NONE)]);
            let found = output.platform_output.events.iter().any(|event| {
                matches!(event, egui::output::OutputEvent::FocusGained(info)
                    if info.label.as_deref() == Some(label) || info.current_text_value.as_deref() == Some(label))
            });
            self.release(Key::Tab);
            if found {
                return self.focused().expect("Tab 焦点");
            }
        }
        panic!("键盘无法到达 {label}");
    }
    fn assert_focus(&mut self, label: &str) {
        let output = self.idle();
        let id = self.focused().unwrap_or_else(|| panic!("{label} 没有焦点"));
        let response = self.frame_focus.as_ref().unwrap();
        assert_eq!(response.id, id);
        assert!(response.has_focus() && response.enabled());
        let mut rects = vec![];
        for shape in &output.shapes {
            text_rects(&shape.shape, shape.clip_rect, label, &mut rects);
        }
        assert!(
            rects.iter().any(|(rect, clip)| {
                response.rect.contains(rect.center()) && clip.contains_rect(*rect)
            }),
            "焦点按钮必须含可见文字 {label}，response={:?}, text={rects:?}",
            response.rect
        );
    }
    fn start(&mut self, key: Key) {
        self.focus("▶ 开始试玩");
        self.key(key);
        self.idle();
    }
    fn pointer(&mut self, label: &str) {
        let output = self.idle();
        let mut rects = vec![];
        for shape in &output.shapes {
            text_rects(&shape.shape, shape.clip_rect, label, &mut rects);
        }
        let point = rects
            .iter()
            .find(|(rect, clip)| clip.contains(rect.center()))
            .unwrap()
            .0
            .center();
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ]);
        }
    }
}
fn key_event(key: Key, pressed: bool, repeat: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat,
        modifiers,
    }
}
fn text_rects(
    shape: &egui::Shape,
    clip: egui::Rect,
    label: &str,
    result: &mut Vec<(egui::Rect, egui::Rect)>,
) {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text == label => {
            result.push((text.galley.rect.translate(text.pos.to_vec2()), clip))
        }
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                text_rects(shape, clip, label, result);
            }
        }
        _ => {}
    }
}

#[test]
fn keyboard_start_multiple_choices_end_and_explicit_record_are_continuous() {
    for key in [Key::Enter, Key::Space] {
        let mut h = Harness::new(NORMAL);
        let baseline = h.app.project.content_baseline();
        h.start(key);
        for (index, label) in ["第一步", "第二步", "第三步"].iter().enumerate() {
            h.assert_focus(&format!("选择：{label}"));
            assert_eq!(h.steps(), index);
            h.key(key);
        }
        h.assert_focus("● 保存当前路径");
        assert_eq!(h.steps(), 3);
        assert!(h.app.play.as_ref().unwrap().ended);
        assert!(h.app.replay_debugger.saved_paths.is_empty());
        h.key(key);
        assert_eq!(h.app.replay_debugger.saved_paths.len(), 1);
        assert!(h.app.replay_debugger.saved_paths[0].trace.complete);
        assert_eq!(h.app.project.content_baseline(), baseline);
        h.focus("↻ 重新开始（已应用稿）");
        h.key(key);
        h.assert_focus("选择：第一步");
        assert_eq!(h.steps(), 0);
    }
}

#[test]
fn locked_first_choice_is_skipped_in_wide_and_narrow_layouts() {
    for size in [egui::vec2(1188.0, 848.0), egui::vec2(800.0, 600.0)] {
        let mut h = Harness::new(LOCKED);
        h.size = size;
        h.start(Key::Enter);
        h.key(Key::Enter);
        h.assert_focus("选择：可以走");
        assert_eq!(h.steps(), 1);
        h.key(Key::Enter);
        h.assert_focus("● 保存当前路径");
        assert_eq!(h.steps(), 2);
    }
}

#[test]
fn held_repeat_never_activates_reused_choice_or_record_button() {
    for key in [Key::Enter, Key::Space] {
        let mut h = Harness::new(NORMAL);
        h.start(key);
        for expected in 1..=3 {
            h.press(key);
            for _ in 0..8 {
                h.frame(vec![key_event(key, true, true, Modifiers::NONE)]);
                h.idle();
                assert_eq!(h.steps(), expected);
                assert!(h.app.replay_debugger.saved_paths.is_empty());
            }
            h.release(key);
            h.idle();
        }
        h.assert_focus("● 保存当前路径");
        h.press(key);
        assert_eq!(h.app.replay_debugger.saved_paths.len(), 1);
        for _ in 0..3 {
            h.frame(vec![key_event(key, true, true, Modifiers::NONE)]);
        }
        assert_eq!(h.app.replay_debugger.saved_paths.len(), 1);
        h.release(key);
        assert_eq!(h.app.replay_debugger.saved_paths.len(), 1);
        // egui 按 keys_down 重算 repeat；释放后的新 down 是合法的新确认。
        h.key(key);
        assert_eq!(h.app.replay_debugger.saved_paths.len(), 2);
    }
}

#[test]
fn compressed_confirmation_and_idle_repaints_do_not_replay_activation() {
    let mut h = Harness::new(NORMAL);
    h.start(Key::Enter);
    let old = h.focused();
    h.frame(vec![
        key_event(Key::Enter, true, false, Modifiers::NONE),
        key_event(Key::Enter, false, false, Modifiers::NONE),
    ]);
    for _ in 0..4 {
        h.idle();
    }
    h.assert_focus("选择：第二步");
    assert_ne!(h.focused(), old);
    assert_eq!(h.steps(), 1);
    assert!(h.app.play_keyboard.pending.is_none());
}

#[test]
fn mouse_start_and_choice_do_not_request_focus_and_text_entry_does_not_choose() {
    let mut h = Harness::new(NORMAL);
    h.pointer("▶ 开始试玩");
    h.idle();
    h.idle();
    assert!(h.app.play_keyboard.pending.is_none());
    h.pointer("选择：第一步");
    h.idle();
    assert_eq!(h.steps(), 1);
    assert!(h.app.play_keyboard.pending.is_none());
    h.focus("路径 1");
    h.key(Key::Space);
    h.key(Key::Enter);
    assert_eq!(h.steps(), 1);
}

#[test]
fn completed_record_capacity_is_a_visible_safe_stop_without_saving() {
    let mut h = Harness::new("event start\n  完成\n  -> END\n");
    h.app.start_play();
    h.idle();
    let trace = h
        .app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .replay_trace();
    h.app.replay_debugger.saved_paths = (0..64)
        .map(|i| SavedReplayPath {
            name: format!("路径{i}"),
            trace: trace.clone(),
        })
        .collect();
    h.focus("↻ 重新开始（已应用稿）");
    h.key(Key::Enter);
    for _ in 0..3 {
        h.idle();
    }
    assert_eq!(h.app.replay_debugger.saved_paths.len(), 64);
    assert!(h.app.play_keyboard.pending.is_none());
    assert!(h
        .app
        .replay_debugger
        .notice
        .as_deref()
        .unwrap()
        .contains("最多保留"));
}

#[test]
fn locked_click_is_rejected_and_deep_enabled_target_scrolls_into_view_once() {
    let mut source = "event start\n  choice \"第一步\"\n    -> next\nevent next\n".to_owned();
    for i in 0..14 {
        source.push_str(&format!(
            "  choice \"锁定路线{i}\" enable false disabled \"尚未满足路线条件\"\n    -> END\n"
        ));
    }
    let label = "唯一可走的长路线说明：继续核实沿岸旧灯塔记录并与守塔人讨论这次潮汐";
    source.push_str(&format!("  choice \"{label}\"\n    -> END\n"));
    let mut h = Harness::new(&source);
    h.size = egui::vec2(800.0, 600.0);
    h.start(Key::Enter);
    h.key(Key::Enter);
    h.assert_focus(&format!("选择：{label}"));
    let focus = h.focused();
    for _ in 0..5 {
        h.idle();
    }
    assert_eq!(h.focused(), focus);
    assert!(h.app.play_keyboard.pending.is_none());
    assert_eq!(h.steps(), 1);

    let mut h = Harness::new(LOCKED);
    h.start(Key::Enter);
    h.key(Key::Enter);
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
    h.pointer("选择：不可走");
    h.idle();
    assert_eq!(h.steps(), 1);
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
    assert!(h.app.play_keyboard.pending.is_none());
}

#[test]
fn interruption_outcomes_never_become_a_keyboard_target() {
    let ctx = egui::Context::default();
    for outcome in [
        ContinuationOutcome::Cancelled,
        ContinuationOutcome::StepBudgetExceeded,
        ContinuationOutcome::TimeBudgetExceeded,
    ] {
        let mut keyboard = PlayKeyboard::default();
        keyboard.advanced(
            &ctx,
            Activation {
                source: Some(egui::Id::new("confirm")),
            },
        );
        keyboard.settled(&ctx, outcome, Some(0));
        assert!(keyboard.pending.is_none());
    }
}

#[test]
fn inspector_returns_to_same_choice_focus_without_enter_carryover() {
    let mut h = Harness::new(NORMAL);
    h.start(Key::Enter);
    h.assert_focus("选择：第一步");
    let focus = h.focused();
    let before = h
        .app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .replay_trace();
    h.app.replay_debugger.inspection.show(&h.ctx);
    h.idle();
    h.focus("返回试玩与选择");
    h.key(Key::Enter);
    h.idle();
    assert!(!h.app.replay_debugger.inspection.open);
    assert_eq!(h.focused(), focus);
    h.assert_focus("选择：第一步");
    assert_eq!(h.steps(), 0);
    assert_eq!(
        h.app
            .play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .replay_trace(),
        before
    );
    h.key(Key::Enter);
    h.assert_focus("选择：第二步");
    assert_eq!(h.steps(), 1);
}

#[test]
fn closing_inspector_does_not_resume_a_stopped_or_failed_play() {
    let mut h = Harness::new(NORMAL);
    h.start(Key::Enter);
    let play = h.app.play.as_mut().unwrap();
    play.paused = true;
    play.stopped = true;
    play.error = Some("测试保留错误".into());
    let save = play.story.as_ref().unwrap().save().unwrap();
    h.app.replay_debugger.inspection.show(&h.ctx);
    h.idle();
    h.pointer("返回试玩与选择");
    h.idle();
    let play = h.app.play.as_ref().unwrap();
    assert!(play.paused && play.stopped);
    assert_eq!(play.error.as_deref(), Some("测试保留错误"));
    assert_eq!(play.story.as_ref().unwrap().save().unwrap(), save);
    assert_eq!(h.steps(), 0);
}
