//! 字体与本轮源码换行控件必须从任务命令用键盘到达，不能继续输入背景源码。
use super::*;
use egui::{Context, Event, Key, Modifiers};

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn frame(
    ctx: &Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> Vec<egui::output::OutputEvent> {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 850.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            app.author_shortcuts(ctx);
            app.source_tab(ctx);
            app.command_window(ctx);
            app.preferences_window(ctx);
            app.capture_edit_focus(ctx);
        },
    )
    .platform_output
    .events
}

fn app() -> (Context, WorldeditApp, egui::Id, String) {
    let ctx = Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let text = "event start\n  背景源码不能接收设置窗口的Tab\n".to_owned();
    app.project
        .set_text(&app.active_file.clone(), text.clone())
        .unwrap();
    app.tab = Tab::Edit;
    let id = egui::Id::new(("source", &app.active_file));
    frame(&ctx, &mut app, vec![]);
    ctx.memory_mut(|memory| memory.request_focus(id));
    frame(&ctx, &mut app, vec![]);
    (ctx, app, id, text)
}

#[test]
fn preferences_command_moves_focus_inside_and_tab_reaches_source_wrap_without_editing_source() {
    let (ctx, mut app, source_id, text) = app();
    frame(
        &ctx,
        &mut app,
        vec![key(Key::P, Modifiers::COMMAND | Modifiers::SHIFT)],
    );
    frame(&ctx, &mut app, vec![Event::Text("设置字体".into())]);
    frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::NONE)]);
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    assert!(app.personal.preferences_open);
    assert!(!app.command_palette.open);
    assert!(ctx.memory(|memory| memory.focused()).is_some());
    assert_ne!(ctx.memory(|memory| memory.focused()), Some(source_id));
    let mut tested_font_arrow = false;
    for count in 1..=12 {
        let events = frame(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)]);
        for event in events {
            if let egui::output::OutputEvent::FocusGained(info) = event {
                println!("PREFERENCES_TAB {count}: {info:?}");
                if info.label.as_deref() == Some("正文字号") {
                    let before = app.personal.settings.body_size;
                    frame(&ctx, &mut app, vec![key(Key::ArrowRight, Modifiers::NONE)]);
                    println!(
                        "PREFERENCES_FONT_RIGHT: {before} -> {}",
                        app.personal.settings.body_size
                    );
                    assert!(app.personal.settings.body_size > before);
                    tested_font_arrow = true;
                }
                if info.label.as_deref() == Some("源码自动换行") {
                    frame(&ctx, &mut app, vec![key(Key::Space, Modifiers::NONE)]);
                    println!("PREFERENCES_WRAP_TAB_COUNT: {count}");
                }
            }
        }
        assert_eq!(
            app.project.document(&app.active_file).unwrap(),
            text,
            "设置键盘操作不能插入背景源码"
        );
        if app.personal.settings.source_wrap {
            break;
        }
    }
    assert!(tested_font_arrow, "字号slider的键盘路径必须实测");
    assert!(
        app.personal.settings.source_wrap,
        "Tab应从主题依次到达源码自动换行"
    );
    assert!(app.personal.preferences_open);
}

#[test]
fn preferences_only_takes_initial_focus_and_retakes_it_after_close_and_reopen() {
    let (ctx, mut app, source_id, text) = app();
    app.personal.preferences_open = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    let first = ctx.memory(|memory| memory.focused());
    assert!(first.is_some());
    assert_ne!(first, Some(source_id));
    frame(&ctx, &mut app, vec![key(Key::Tab, Modifiers::NONE)]);
    let second = ctx.memory(|memory| memory.focused());
    assert_ne!(first, second);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        second,
        "已打开窗口不能每帧抢回首控件"
    );
    app.personal.preferences_open = false;
    frame(&ctx, &mut app, vec![]);
    ctx.memory_mut(|memory| memory.request_focus(source_id));
    app.personal.preferences_open = true;
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(ctx.memory(|memory| memory.focused()), first);
    assert_eq!(app.project.document(&app.active_file).unwrap(), text);
}
