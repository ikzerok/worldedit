//! 真正 App 的 Tab/Enter 旅程，所有条件改变都来自已绘制控件。
use super::*;
use crate::chrome::action_layout::{trace, ActionRect};
use egui::{Event, Key, Modifiers};
fn frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) {
    let modifiers = events
        .iter()
        .find_map(|event| match event {
            Event::Key { modifiers, .. } => Some(*modifiers),
            _ => None,
        })
        .unwrap_or_default();
    let mut raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1188.0, 760.0),
        )),
        events,
        modifiers,
        focused: true,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    let _ = ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest())
    });
}
fn tap(ctx: &egui::Context, app: &mut WorldeditApp, key: Key, modifiers: Modifiers) {
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            }],
        );
    }
    frame(ctx, app, vec![]);
}
fn seek(ctx: &egui::Context, app: &mut WorldeditApp, role: &str) -> ActionRect {
    // 顶部事件应用从条件区反向返回；不跨过下面具有 Tab 缩进语义的高级正文。
    let reverse = role == "condition/event-apply"
        && ctx.memory(|memory| memory.focused()).is_some_and(|id| {
            ctx.viewport(|viewport| viewport.this_pass.widgets.get(id).is_some())
        });
    for _ in 0..300 {
        if let Some(response) = trace(ctx).into_iter().find(|r| {
            r.role == role && r.enabled && ctx.memory(|memory| memory.focused()) == Some(r.id)
        }) {
            for _ in 0..2 {
                frame(ctx, app, vec![]);
            }
            let response = trace(ctx)
                .into_iter()
                .find(|r| r.role == role && r.id == response.id)
                .unwrap();
            assert!(
                response.clip.expand(1.0).contains_rect(response.rect),
                "focused action must reveal: {response:?}"
            );
            assert!(ctx.screen_rect().expand(1.0).contains_rect(response.rect));
            return response;
        }
        let body = app
            .event_editor
            .as_ref()
            .map(|editor| editor.draft.body.clone());
        tap(
            ctx,
            app,
            Key::Tab,
            if reverse {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
        );
        assert_eq!(
            app.event_editor
                .as_ref()
                .map(|editor| editor.draft.body.clone()),
            body,
            "keyboard route must not type indentation into the advanced code editor"
        );
    }
    panic!(
        "Tab did not reach {role}; focus={:?}",
        ctx.memory(|memory| memory.focused())
    );
}
fn activate(ctx: &egui::Context, app: &mut WorldeditApp, role: &str) {
    seek(ctx, app, role);
    tap(ctx, app, Key::Enter, Modifiers::NONE);
}

#[test]
fn condition_ui_keyboard_reveal_does_not_pull_back_a_later_pointer_scroll() {
    let (ctx, mut app) = setup();
    ctx.style_mut(|style| style.animation_time = 0.0);
    app.select_event("start");
    app.tab = crate::app::Tab::Timeline;
    app.personal.pending_restore = false;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    let before = app.project.content_baseline();
    let draft = app.event_editor.as_ref().unwrap().draft.clone();
    let action = seek(&ctx, &mut app, "condition/显示条件/preview");
    frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(action.rect.center()),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -10_000.0),
                modifiers: Modifiers::NONE,
            },
        ],
    );
    for _ in 0..90 {
        frame(&ctx, &mut app, vec![]);
    }
    let later = ctx.read_response(action.id).unwrap().rect;
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(action.id));
    assert!(
        later.top() < action.rect.top() - 40.0,
        "pointer scroll must genuinely move away from focused button: {:?} -> {later:?}",
        action.rect
    );
    for _ in 0..6 {
        frame(&ctx, &mut app, vec![]);
    }
    assert!(
        (ctx.read_response(action.id).unwrap().rect.top() - later.top()).abs() < 0.5,
        "old keyboard focus must not pull back a newer pointer scroll"
    );
    assert_eq!(app.project.content_baseline(), before);
    assert!(app.event_editor.as_ref().unwrap().draft == draft);
}
#[test]
fn condition_ui_real_app_keyboard_state_tag_preview_apply_save_and_owner_run_journey() {
    let (ctx, mut app) = setup();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let source = format!(
        "world room\ntag calm\ntag alert\nstate mood on world room with calm\n{}",
        app.project
            .document(&app.active_file)
            .unwrap()
            .replace("if allowed", "if has(mood, calm)")
    );
    app.project
        .set_text(&app.active_file.clone(), source.clone())
        .unwrap();
    app.project.save().unwrap();
    app.saved_location = true;
    app.recompile();
    app.start_play();
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .continue_story()
        .unwrap();
    let stamp = app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .inspection_stamp();
    app.select_event("start");
    app.tab = crate::app::Tab::Timeline;
    app.personal.pending_restore = false;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    activate(&ctx, &mut app, "condition/tag-picker");
    activate(&ctx, &mut app, "condition/candidate/tag:alert");
    assert!(app
        .event_editor
        .as_ref()
        .unwrap()
        .condition_forms
        .has_pending());
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    let draft = app.event_editor.as_ref().unwrap().draft.clone();
    let pending = app
        .event_editor
        .as_ref()
        .unwrap()
        .condition_forms
        .signature();
    seek(&ctx, &mut app, "condition/state-definition");
    frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Enabled)]);
    tap(&ctx, &mut app, Key::Enter, Modifiers::NONE);
    assert_eq!(app.tab, crate::app::Tab::Edit);
    let (source_id, text) = app.source_position_document(&app.active_file).unwrap();
    let range = egui::TextEdit::load_state(&ctx, source_id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    let low = range.primary.index.min(range.secondary.index);
    let high = range.primary.index.max(range.secondary.index);
    let selected = text.chars().skip(low).take(high - low).collect::<String>();
    assert_eq!(selected, "state mood on world room with calm");
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(source_id));
    assert!(app.event_editor.as_ref().unwrap().draft == draft);
    assert_eq!(
        app.event_editor
            .as_ref()
            .unwrap()
            .condition_forms
            .signature(),
        pending
    );
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    // 原生离开文字宿主会结束空闲输入会话；全局返回键仍保留既有 IME 保护。
    frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Disabled)]);
    frame(&ctx, &mut app, vec![]);
    tap(&ctx, &mut app, Key::ArrowLeft, Modifiers::ALT);
    assert_eq!(app.tab, crate::app::Tab::Timeline);
    assert!(app.event_editor.as_ref().unwrap().draft == draft);
    assert_eq!(
        app.event_editor
            .as_ref()
            .unwrap()
            .condition_forms
            .signature(),
        pending
    );
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    activate(&ctx, &mut app, "condition/显示条件/preview");
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    activate(&ctx, &mut app, "condition/显示条件/apply");
    assert_eq!(
        app.event_editor.as_ref().unwrap().draft.choices()[0].condition,
        "has(mood, alert)"
    );
    assert_eq!(std::fs::read_to_string(&app.active_file).unwrap(), source);
    activate(&ctx, &mut app, "condition/event-apply");
    assert_eq!(
        app.project.document(&app.active_file).unwrap(),
        source.replace("has(mood, calm)", "has(mood, alert)")
    );
    tap(&ctx, &mut app, Key::S, Modifiers::COMMAND);
    assert!(!app.project.is_dirty());
    let reopened = Project::open(&app.project.root).unwrap();
    assert_eq!(
        reopened.event_draft("start").unwrap().1.choices()[0].condition,
        "has(mood, alert)"
    );
    seek(&ctx, &mut app, "condition/state-owner");
    frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Enabled)]);
    tap(&ctx, &mut app, Key::Enter, Modifiers::NONE);
    assert!(app.replay_debugger.object_inspection.open);
    activate(&ctx, &mut app, "object/run-picker");
    assert!(egui::Popup::is_any_open(&ctx));
    let ordinary = seek(&ctx, &mut app, "object/run/ordinary");
    let unavailable = trace(&ctx)
        .into_iter()
        .find(|r| r.role == "object/run/draft")
        .unwrap();
    assert!(
        !unavailable.enabled,
        "absent draft run must not be selectable"
    );
    for modifiers in [Modifiers::NONE, Modifiers::SHIFT, Modifiers::NONE] {
        tap(&ctx, &mut app, Key::Tab, modifiers);
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(ordinary.id));
        assert!(
            egui::Popup::is_any_open(&ctx),
            "Tab must stay in the run menu"
        );
        assert!(app.object_inspection_page_for_test().is_none());
    }
    tap(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    assert!(!egui::Popup::is_any_open(&ctx));
    assert!(app.replay_debugger.object_inspection.open);
    let picker = trace(&ctx)
        .into_iter()
        .find(|r| r.role == "object/run-picker")
        .unwrap();
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(picker.id));
    activate(&ctx, &mut app, "object/run-picker");
    activate(&ctx, &mut app, "object/run/ordinary");
    let page = app.object_inspection_page_for_test().unwrap();
    assert_eq!(
        page.owner.as_ref().unwrap().target,
        TargetRef::new("world", "room")
    );
    assert_eq!(
        page.stamp, stamp,
        "selection must not restart the existing run"
    );
    activate(&ctx, &mut app, "object/return");
    assert!(!app.replay_debugger.object_inspection.open);
    assert_eq!(app.tab, crate::app::Tab::Timeline);
    assert_eq!(
        app.event_editor.as_ref().unwrap().draft.choices()[0].condition,
        "has(mood, alert)"
    );
}
