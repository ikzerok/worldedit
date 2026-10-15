//! 真实 App 跨帧 IME 合成事件；物理中文输入法仍另行验收。
use super::*;
use egui::{Event, ImeEvent};
fn frame(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2, events: Vec<Event>) {
    let mut raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        events,
        focused: true,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    let _ = ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest());
    });
}
#[test]
fn app_condition_string_ime_preedit_resize_and_commit_keep_same_live_field() {
    let (ctx, mut app) = setup();
    let source = app
        .project
        .document(&app.active_file)
        .unwrap()
        .replace("if allowed", "if title == \"\"");
    app.project
        .set_text(&app.active_file.clone(), source)
        .unwrap();
    app.project.save().unwrap();
    app.recompile();
    app.select_event("start");
    app.tab = crate::app::Tab::Timeline;
    app.personal.pending_restore = false;
    let wide = egui::vec2(1280.0, 1200.0);
    for _ in 0..3 {
        frame(&ctx, &mut app, wide, vec![]);
    }
    let focus = ctx
        .data_mut(|data| data.get_temp::<egui::Id>(egui::Id::new("condition-literal-test-focus")))
        .expect("typed string field is rendered by actual event inspector");
    ctx.memory_mut(|memory| memory.request_focus(focus));
    let baseline = app.project.content_baseline();
    frame(&ctx, &mut app, wide, vec![Event::Ime(ImeEvent::Enabled)]);
    frame(
        &ctx,
        &mut app,
        egui::vec2(900.0, 900.0),
        vec![Event::Ime(ImeEvent::Preedit("候选".into()))],
    );
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(focus));
    app.open_object_inspection(&ctx, TargetRef::new("event", "start"));
    assert!(
        !app.replay_debugger.object_inspection.open,
        "composition must not move into a new window"
    );
    frame(
        &ctx,
        &mut app,
        egui::vec2(900.0, 900.0),
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(focus));
    assert!(app.event_editor.is_some());
    // 候选导航键不能变成正文缩进/换行或另一控件的导航。
    for key in [egui::Key::Tab, egui::Key::Enter, egui::Key::ArrowDown] {
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                egui::vec2(900.0, 900.0),
                vec![Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(focus));
    }
    let action = crate::chrome::action_layout::trace(&ctx)
        .into_iter()
        .find(|r| r.enabled && (r.role == "global/appearance" || r.role == "global/more"))
        .expect("real action that can open another UI layer");
    assert!(action.clip.contains_rect(action.rect));
    assert!(ctx.screen_rect().contains_rect(action.rect));
    frame(
        &ctx,
        &mut app,
        egui::vec2(900.0, 900.0),
        vec![
            Event::PointerMoved(action.rect.center()),
            Event::PointerButton {
                pos: action.rect.center(),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(focus));
    assert!(!app.personal.preferences_open && !egui::Popup::is_any_open(&ctx));
    frame(
        &ctx,
        &mut app,
        wide,
        vec![Event::Ime(ImeEvent::Commit("中文完成".into()))],
    );
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(focus));
    frame(
        &ctx,
        &mut app,
        wide,
        vec![
            Event::Ime(ImeEvent::Disabled),
            // 提交后的松开也属于被拒绝的那次按下，不能突然打开窗口。
            Event::PointerButton {
                pos: action.rect.center(),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(focus));
    assert!(!app.personal.preferences_open && !egui::Popup::is_any_open(&ctx));
    let form = app.event_editor.as_ref().unwrap();
    let (_, _, buffer) = form
        .condition_forms
        .entries
        .iter()
        .find(|(_, slot, _)| *slot == ConditionSlot::ChoiceDisplay)
        .unwrap();
    let Some(ConditionNode::Compare {
        value: ConditionLiteral::Str(text),
        ..
    }) = &buffer.tree
    else {
        panic!("string comparison must remain typed");
    };
    assert_eq!(text, "中文完成");
    assert!(form.condition_forms.has_pending());
    assert_eq!(app.project.content_baseline(), baseline);
    // 新的明确手势不再被旧组合阻塞。
    let fresh = crate::chrome::action_layout::trace(&ctx)
        .into_iter()
        .find(|r| r.enabled && (r.role == "global/appearance" || r.role == "global/more"))
        .unwrap();
    for pressed in [true, false] {
        frame(
            &ctx,
            &mut app,
            wide,
            vec![
                Event::PointerMoved(fresh.rect.center()),
                Event::PointerButton {
                    pos: fresh.rect.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert!(app.personal.preferences_open || egui::Popup::is_any_open(&ctx));
    assert_eq!(app.project.content_baseline(), baseline);
}
