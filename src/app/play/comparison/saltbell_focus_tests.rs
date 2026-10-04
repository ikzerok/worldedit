//! 真实盐钟双文件/动作别名 fixture；按事件与帧末状态区分返回焦点，不推测原生投递。
use super::*;
const SIZE: egui::Vec2 = egui::vec2(1188.0, 848.0);
const SOURCE_BUTTON: &str = "打开实际动作来源";
const SUGGESTION: &str = "从选中文本建档";

fn setup_saltbell() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = setup();
    let root = app.project.root.clone();
    std::fs::create_dir_all(root.join(".world")).unwrap();
    for (path, contents) in [
        (
            "world.wl",
            include_str!("../../../../tests/fixtures/route-comparison-saltbell/world.wl"),
        ),
        (
            "shared.wl",
            include_str!("../../../../tests/fixtures/route-comparison-saltbell/shared.wl"),
        ),
        (
            ".world/project.json",
            include_str!("../../../../tests/fixtures/route-comparison-saltbell/project.json"),
        ),
    ] {
        std::fs::write(root.join(path), contents).unwrap();
    }
    app.project = Project::open(&root.join("world.wl")).unwrap();
    app.active_file = app.project.entry.clone();
    app.personal.pending_restore = false;
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    let snapshot = &app.snapshot.as_ref().unwrap().result;
    app.replay_debugger.saved_paths = [0, 1]
        .into_iter()
        .map(|choice| {
            let mut story =
                Story::new_with_seed(&snapshot.program, &snapshot.analysis, 42).unwrap();
            story.continue_story().unwrap();
            story.choose(choice).unwrap();
            story.continue_story().unwrap();
            story.choose(0).unwrap();
            story.continue_story().unwrap();
            assert!(story.is_ended());
            SavedReplayPath {
                name: format!("路径 {}", choice + 1),
                trace: story.replay_trace(),
            }
        })
        .collect();
    app.play = None;
    app.start_play();
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .continue_story()
        .unwrap();
    compare(&ctx, &mut app);
    assert_eq!(app.comparison.selected_state.as_deref(), Some("bell_state"));
    (ctx, app)
}

fn frame_full(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
    log: bool,
) -> egui::FullOutput {
    let mut before_end = None;
    let events_text = format!("{events:?}");
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
            modifiers,
            events,
            ..Default::default()
        },
        |ctx| {
            eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest());
            before_end = ctx.memory(|memory| memory.focused());
        },
    );
    if log {
        let after = ctx.memory(|memory| memory.focused());
        let owner = after.and_then(|id| ctx.read_response(id)).map(|response| {
            let mut texts = Vec::new();
            for shape in &output.shapes {
                labels(&shape.shape, &mut texts);
            }
            let labels: Vec<_> = texts
                .into_iter()
                .filter(|(_, rect)| response.rect.contains(rect.center()))
                .map(|(text, _)| text.chars().take(100).collect::<String>())
                .collect();
            (
                response.id,
                response.layer_id,
                response.rect,
                response.enabled(),
                labels,
            )
        });
        println!("FOCUS frame={} input={} mods={modifiers:?} pre_end={before_end:?} post_end={after:?} owner={owner:?} tab={:?} pending={:?} scroll={} layers={:?} window_focus={}", ctx.cumulative_frame_nr(), events_text, app.tab, app.comparison.restore_focus, app.comparison.scroll, app.command_palette.focus_stack, ctx.input(|input| input.focused));
    }
    output
}
fn key_event(key: egui::Key, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers,
    }
}
fn key(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    key: egui::Key,
    modifiers: egui::Modifiers,
    log: bool,
) -> Vec<egui::WidgetInfo> {
    let mut focus = Vec::new();
    for pressed in [true, false] {
        let output = frame_full(
            ctx,
            app,
            vec![key_event(key, pressed, modifiers)],
            modifiers,
            log,
        );
        for event in output.platform_output.events {
            if let egui::output::OutputEvent::FocusGained(info) = event {
                focus.push(info);
            }
        }
    }
    focus
}
fn has_label(output: &egui::FullOutput, label: &str) -> bool {
    let mut text = Vec::new();
    for shape in &output.shapes {
        labels(&shape.shape, &mut text);
    }
    text.iter().any(|(text, _)| text == label)
}
fn roundtrip(compact_input: bool, move_focus: bool, settled: bool) {
    println!("CASE compact_input={compact_input} move_focus={move_focus} settled={settled} resize_id={:?}", egui::Id::new("project").with("__resize"));
    let (ctx, mut app) = setup_saltbell();
    frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
    let before = invariant(&app);
    let mut focused = None;
    for _ in 0..180 {
        if key(&ctx, &mut app, egui::Key::Tab, egui::Modifiers::NONE, false)
            .iter()
            .any(|info| {
                info.typ == egui::WidgetType::Button && info.label.as_deref() == Some(SOURCE_BUTTON)
            })
        {
            focused = ctx.memory(|memory| memory.focused());
            break;
        }
    }
    let focused = focused.expect("实际来源按钮键盘不可达");
    for _ in 0..16 {
        frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
    }
    let scroll = app.comparison.scroll;
    assert!(scroll > 0.0);
    frame_full(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, true, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
        true,
    );
    let first_source = frame_full(
        &ctx,
        &mut app,
        vec![key_event(egui::Key::Enter, false, egui::Modifiers::NONE)],
        egui::Modifiers::NONE,
        true,
    );
    assert_eq!(app.tab, Tab::Edit);
    assert_eq!(app.comparison.selected_action, Some((false, 2)));
    let source_id = egui::Id::new(("source", &app.active_file));
    let mut suggestion = has_label(&first_source, SUGGESTION);
    for _ in 0..if settled { 8 } else { 0 } {
        suggestion |= has_label(
            &frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, true),
            SUGGESTION,
        );
    }
    assert!(!suggestion, "程序定位不得自动出现建档建议");
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(source_id));
    if move_focus {
        let output = frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
        let mut text = Vec::new();
        for shape in &output.shapes {
            labels(&shape.shape, &mut text);
        }
        let point = text
            .iter()
            .find(|(text, _)| text == "world.wl")
            .unwrap()
            .1
            .center();
        for pressed in [true, false] {
            frame_full(
                &ctx,
                &mut app,
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                egui::Modifiers::NONE,
                true,
            );
        }
        assert_ne!(ctx.memory(|memory| memory.focused()), Some(source_id));
    }
    if compact_input {
        frame_full(
            &ctx,
            &mut app,
            vec![
                key_event(egui::Key::ArrowLeft, true, egui::Modifiers::ALT),
                key_event(egui::Key::ArrowLeft, false, egui::Modifiers::ALT),
            ],
            egui::Modifiers::NONE,
            true,
        );
    } else {
        key(
            &ctx,
            &mut app,
            egui::Key::ArrowLeft,
            egui::Modifiers::ALT,
            true,
        );
    }
    assert_eq!(app.tab, Tab::Play);
    for _ in 0..8 {
        frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, true);
    }
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        Some(focused),
        "saltbell compact={compact_input} moved={move_focus}"
    );
    assert!(ctx.read_response(focused).unwrap().has_focus());
    assert_eq!(app.comparison.scroll, scroll);
    key(
        &ctx,
        &mut app,
        egui::Key::Enter,
        egui::Modifiers::NONE,
        true,
    );
    assert_eq!(app.tab, Tab::Edit);
    assert_eq!(invariant(&app), before);
}
#[test]
fn saltbell_return_held_focused() {
    roundtrip(false, false, true);
}
#[test]
fn saltbell_return_held_unfocused() {
    roundtrip(false, true, true);
}
#[test]
fn saltbell_return_same_frame_focused() {
    roundtrip(true, false, true);
}
#[test]
fn saltbell_return_same_frame_unfocused() {
    roundtrip(true, true, true);
}

#[test]
fn saltbell_return_same_frame_immediate_source() {
    roundtrip(true, false, false);
}

fn source_selection_authoring(manual: bool) {
    let (ctx, mut app) = setup_saltbell();
    let before = invariant(&app);
    frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
    let mut reached = false;
    for _ in 0..180 {
        if key(&ctx, &mut app, egui::Key::Tab, egui::Modifiers::NONE, false)
            .iter()
            .any(|info| {
                info.typ == egui::WidgetType::Button && info.label.as_deref() == Some(SOURCE_BUTTON)
            })
        {
            reached = true;
            break;
        }
    }
    assert!(reached);
    key(
        &ctx,
        &mut app,
        egui::Key::Enter,
        egui::Modifiers::NONE,
        false,
    );
    assert_eq!(app.tab, Tab::Edit);
    for _ in 0..8 {
        let output = frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
        assert!(
            !has_label(&output, SUGGESTION),
            "程序动作定位不应被当作人工建档选区"
        );
    }
    let source_id = egui::Id::new(("source", &app.active_file));
    let range = egui::TextEdit::load_state(&ctx, source_id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    let selected: String = app
        .project
        .document(&app.active_file)
        .unwrap()
        .chars()
        .skip(range.primary.index.min(range.secondary.index))
        .take(range.primary.index.abs_diff(range.secondary.index))
        .collect();
    assert_eq!(selected, "become bell_state with ringing as \"决定鸣响\"");
    if manual {
        key(
            &ctx,
            &mut app,
            egui::Key::ArrowLeft,
            egui::Modifiers::NONE,
            false,
        );
        for _ in 0..selected.chars().count() {
            key(
                &ctx,
                &mut app,
                egui::Key::ArrowRight,
                egui::Modifiers::SHIFT,
                false,
            );
        }
        let output = frame_full(&ctx, &mut app, vec![], egui::Modifiers::NONE, false);
        assert!(
            has_label(&output, SUGGESTION),
            "人工重新选中相同文本后建档建议必须恢复"
        );
    }
    key(
        &ctx,
        &mut app,
        egui::Key::Enter,
        egui::Modifiers::COMMAND,
        false,
    );
    let form = app
        .entity_editor
        .as_ref()
        .expect("显式 CtrlEnter 仍可按选区建档");
    assert_eq!(
        form.source_selection.as_ref().unwrap().expected_text,
        selected
    );
    assert_eq!(invariant(&app), before);
}
#[test]
fn saltbell_programmatic_source_suppresses_suggestion_but_keeps_explicit_ctrl_enter() {
    source_selection_authoring(false);
}
#[test]
fn saltbell_manual_reselection_restores_suggestion_and_ctrl_enter() {
    source_selection_authoring(true);
}
