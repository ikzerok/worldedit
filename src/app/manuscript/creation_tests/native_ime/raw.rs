use super::*;

fn candidate_key(key: egui::Key, pressed: bool) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn raw_first_preedit_with_tab_or_escape_keeps_the_original_field_for_commit() {
    for body in [false, true] {
        for key in [egui::Key::Tab, egui::Key::Escape] {
            let (ctx, mut app) = blank();
            if body {
                native_body(&ctx, &mut app, true);
            } else {
                native_titles(&ctx, &mut app, true);
            }
            let baseline = app.project.content_baseline();
            let focus = ctx.memory(|memory| memory.focused()).unwrap();
            native_frame(
                &ctx,
                &mut app,
                vec![
                    Event::Ime(egui::ImeEvent::Preedit("同帧候选".into())),
                    candidate_key(key, true),
                ],
            );
            native_frame(&ctx, &mut app, vec![candidate_key(key, false)]);
            assert!(
                ctx.memory(|memory| memory.has_focus(focus)),
                "body={body}, key={key:?}"
            );
            native_frame(
                &ctx,
                &mut app,
                vec![Event::Ime(egui::ImeEvent::Commit("同帧完成".into()))],
            );
            let text = if body {
                app.manuscript.writing_buffers[&app.project.entry]
                    .source()
                    .to_owned()
            } else {
                app.manuscript
                    .creation
                    .as_ref()
                    .unwrap()
                    .chapter_title
                    .clone()
            };
            assert!(
                text.contains("同帧完成"),
                "body={body}, key={key:?}: {text}"
            );
            assert!(!text.contains("同帧候选"));
            assert!(!app.manuscript.creation_dismissed);
            assert_eq!(app.project.content_baseline(), baseline);
        }
    }
}

#[test]
fn raw_manuscript_filter_does_not_consume_upper_layer_or_source_input() {
    for layer in ["commands", "search", "preferences", "source"] {
        let (ctx, mut app) = blank();
        native_body(&ctx, &mut app, true);
        let body_focus = ctx.memory(|memory| memory.focused()).unwrap();
        let before = app.manuscript.writing_buffers[&app.project.entry]
            .source()
            .to_owned();
        match layer {
            "commands" => app.open_commands(&ctx, false),
            "search" => app.open_search(&ctx, false, false),
            "preferences" => app.personal.preferences_open = true,
            "source" => app.tab = Tab::Edit,
            _ => unreachable!(),
        }
        native_settle(&ctx, &mut app);
        if layer == "source" {
            let source_id = egui::Id::new(("source", &app.project.entry));
            ctx.memory_mut(|memory| memory.request_focus(source_id));
            native_frame(&ctx, &mut app, vec![]);
        }
        assert_ne!(
            ctx.memory(|memory| memory.focused()),
            Some(body_focus),
            "{layer}"
        );
        let mut raw = RawInput {
            events: vec![
                Event::Ime(egui::ImeEvent::Enabled),
                Event::Ime(egui::ImeEvent::Preedit("上层候选".into())),
                candidate_key(egui::Key::Tab, true),
                candidate_key(egui::Key::Escape, true),
                Event::Ime(egui::ImeEvent::Commit("上层提交".into())),
                Event::Paste("粘贴".into()),
                Event::Text("@".into()),
            ],
            ..Default::default()
        };
        let events = raw.events.clone();
        eframe::App::raw_input_hook(&mut app, &ctx, &mut raw);
        assert_eq!(raw.events, events, "upper input must stay with {layer}");
        assert_eq!(
            app.manuscript.writing_buffers[&app.project.entry].source(),
            before
        );
    }
}

#[test]
fn raw_candidate_key_releases_clear_keys_held_before_composition() {
    let (ctx, mut app) = blank();
    native_titles(&ctx, &mut app, true);
    native_frame(
        &ctx,
        &mut app,
        vec![candidate_key(egui::Key::ArrowLeft, true)],
    );
    assert!(ctx.input(|input| input.key_down(egui::Key::ArrowLeft)));
    native_frame(
        &ctx,
        &mut app,
        vec![
            Event::Ime(egui::ImeEvent::Preedit("候选".into())),
            candidate_key(egui::Key::ArrowLeft, false),
        ],
    );
    assert!(!ctx.input(|input| input.key_down(egui::Key::ArrowLeft)));
    let mut raw = RawInput {
        events: vec![
            candidate_key(egui::Key::Enter, false),
            candidate_key(egui::Key::Tab, false),
            candidate_key(egui::Key::Escape, false),
            candidate_key(egui::Key::ArrowDown, false),
        ],
        ..Default::default()
    };
    let releases = raw.events.clone();
    eframe::App::raw_input_hook(&mut app, &ctx, &mut raw);
    assert_eq!(raw.events, releases);
    native_frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Commit("完成".into()))],
    );
    assert!(app
        .manuscript
        .creation
        .as_ref()
        .unwrap()
        .chapter_title
        .contains("完成"));
}

#[test]
fn a_nonmodal_preferences_window_does_not_disable_protection_after_focus_returns_to_body() {
    let (ctx, mut app) = blank();
    native_body(&ctx, &mut app, true);
    let body_focus = ctx.memory(|memory| memory.focused()).unwrap();
    native_click(&ctx, &mut app, "外观", None);
    native_settle(&ctx, &mut app);
    assert!(app.personal.preferences_open);
    assert_ne!(ctx.memory(|memory| memory.focused()), Some(body_focus));
    // 已显示窗口的首次焦点安置结束后，明确回到仍在显示的原 TextEdit。
    ctx.memory_mut(|memory| memory.request_focus(body_focus));
    native_frame(&ctx, &mut app, vec![]);
    assert!(ctx.memory(|memory| memory.has_focus(body_focus)));
    native_frame(
        &ctx,
        &mut app,
        vec![
            Event::Ime(egui::ImeEvent::Enabled),
            Event::Ime(egui::ImeEvent::Preedit("再次候选".into())),
            candidate_key(egui::Key::Tab, true),
        ],
    );
    native_frame(&ctx, &mut app, vec![candidate_key(egui::Key::Tab, false)]);
    assert!(ctx.memory(|memory| memory.has_focus(body_focus)));
    native_frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Commit("再次提交".into()))],
    );
    assert!(app.personal.preferences_open);
    assert!(app.manuscript.writing_buffers[&app.project.entry]
        .source()
        .contains("再次提交"));
}
