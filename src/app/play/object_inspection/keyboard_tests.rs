//! 实际 App 的 Tab 域与返回：不以离屏按键代替物理键盘验收。
use super::*;
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
            egui::vec2(1100.0, 800.0),
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
#[test]
fn object_window_real_app_tab_cycle_never_reaches_author_and_escape_restores_opener() {
    let (ctx, mut app) = setup();
    ctx.style_mut(|style| style.animation_time = 0.0);
    start(&mut app);
    app.personal.pending_restore = false;
    app.tab = Tab::Edit;
    app.select_event("start");
    app.event_editor.as_mut().unwrap().draft.summary = "未应用事件输入".into();
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    let opener = app.source_position_document(&app.active_file).unwrap().0;
    assert!(ctx.read_response(opener).is_some());
    ctx.memory_mut(|memory| memory.request_focus(opener));
    app.open_object_inspection(&ctx, TargetRef::new("character", "hero"));
    app.select_object_run(RunKind::Ordinary);
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    let before = unchanged(&app);
    let input = app.event_editor.as_ref().unwrap().draft.clone();
    for reverse in [false, true] {
        let mut seen = std::collections::HashSet::new();
        let mut repeated = false;
        for _ in 0..80 {
            tap(
                &ctx,
                &mut app,
                Key::Tab,
                if reverse {
                    Modifiers::SHIFT
                } else {
                    Modifiers::NONE
                },
            );
            let focus = ctx
                .memory(|memory| memory.focused())
                .expect("window keeps a real focus owner");
            let response = ctx.read_response(focus).expect("current-pass response");
            assert_eq!(
                response.layer_id,
                keyboard::layer(),
                "Tab must remain in the top object window"
            );
            assert_ne!(focus, opener);
            assert_eq!(unchanged(&app), before);
            assert!(app.event_editor.as_ref().unwrap().draft == input);
            if !seen.insert(focus) {
                repeated = true;
                break;
            }
        }
        assert!(repeated, "bounded focus cycle must repeat");
    }
    tap(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    assert!(!app.replay_debugger.object_inspection.open);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(opener));
    assert_eq!(unchanged(&app), before);
    assert!(app.event_editor.as_ref().unwrap().draft == input);
}

#[test]
fn object_long_page_keyboard_reveal_respects_new_wheel_without_changing_run_or_draft() {
    use crate::chrome::action_layout::trace;
    let (ctx, mut app) = setup();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let added = (0..52)
        .map(|index| {
            format!(
                "state extra_{index:02} on character hero with calm as \"长页状态 {index:02}\"\n"
            )
        })
        .collect::<String>();
    let source = SOURCE.replace("event start", &format!("{added}event start"));
    app.project
        .set_text(&app.active_file.clone(), source.clone())
        .unwrap();
    app.project.save().unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    start(&mut app);
    app.select_event("start");
    app.event_editor.as_mut().unwrap().draft.summary = "保留的未应用作者稿".into();
    app.personal.pending_restore = false;
    select(&ctx, &mut app, "character", "hero");
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    let page = app.object_inspection_page_for_test().unwrap().clone();
    assert_eq!(page.total_items, 54);
    assert_eq!(page.limit, 20);
    assert_eq!(page.items.len(), 20);
    assert_eq!(page.next_offset, Some(20));
    let before = unchanged(&app);
    let draft = app.event_editor.as_ref().unwrap().draft.clone();
    let role = "object/state/extra_08";
    let mut focused = None;
    for _ in 0..160 {
        if let Some(response) = trace(&ctx).into_iter().find(|r| {
            r.role == role && r.enabled && ctx.memory(|memory| memory.focused()) == Some(r.id)
        }) {
            focused = Some(response.id);
            break;
        }
        tap(&ctx, &mut app, Key::Tab, Modifiers::NONE);
    }
    let id = focused.expect("real Tab must reach a state in the long object page");
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    let action = trace(&ctx)
        .into_iter()
        .find(|r| r.id == id && r.role == role)
        .unwrap();
    assert!(
        action.clip.expand(1.0).contains_rect(action.rect),
        "state keyboard focus must fully reveal: {action:?}"
    );
    assert!(ctx.screen_rect().contains_rect(action.rect));
    assert!(
        app.replay_debugger.object_inspection.scroll > 40.0,
        "this state starts beyond the body viewport"
    );
    let scroll = app.replay_debugger.object_inspection.scroll;
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
    let later = ctx.read_response(id).unwrap().rect;
    let after_scroll = app.replay_debugger.object_inspection.scroll;
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    assert!(
        after_scroll > scroll + 40.0,
        "wheel must really move the long page: {scroll} -> {after_scroll}"
    );
    assert!(
        later.top() < action.rect.top() - 40.0,
        "wheel must move away from the keyboard owner: {:?} -> {later:?}",
        action.rect
    );
    assert!(
        !action.clip.contains_rect(later),
        "old focus is intentionally outside the viewport after the wheel"
    );
    for _ in 0..6 {
        frame(&ctx, &mut app, vec![]);
    }
    assert!(
        (ctx.read_response(id).unwrap().rect.top() - later.top()).abs() < 0.5,
        "old keyboard focus must not pull the page back"
    );
    assert!((app.replay_debugger.object_inspection.scroll - after_scroll).abs() < 0.5);
    assert_eq!(app.object_inspection_page_for_test().unwrap(), &page);
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .inspection_stamp(),
        page.stamp
    );
    assert_eq!(unchanged(&app), before);
    assert!(app.event_editor.as_ref().unwrap().draft == draft);
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    assert_eq!(std::fs::read_to_string(&app.active_file).unwrap(), source);
    std::fs::remove_dir_all(&app.project.root).unwrap();
}
