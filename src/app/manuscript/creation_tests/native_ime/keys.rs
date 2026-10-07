use super::*;

fn input_text(app: &WorldeditApp, body: bool) -> String {
    if body {
        body_input(app)
    } else {
        app.manuscript
            .creation
            .as_ref()
            .unwrap()
            .chapter_title
            .clone()
    }
}

fn key_event(key: egui::Key, pressed: bool) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn true_preedit_candidate_keys_keep_the_owner_and_commit_in_title_and_body() {
    for body in [false, true] {
        for key in [egui::Key::Enter, egui::Key::Tab, egui::Key::Escape] {
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
                vec![Event::Ime(egui::ImeEvent::Preedit("候选输入".into()))],
            );
            let preedit = input_text(&app, body);
            for pressed in [true, false] {
                native_frame(&ctx, &mut app, vec![key_event(key, pressed)]);
                assert!(
                    ctx.memory(|memory| memory.has_focus(focus)),
                    "body={body}, key={key:?}"
                );
                assert_eq!(input_text(&app, body), preedit, "body={body}, key={key:?}");
            }
            native_frame(
                &ctx,
                &mut app,
                vec![Event::Ime(egui::ImeEvent::Commit("最终输入".into()))],
            );
            let text = input_text(&app, body);
            assert!(
                text.contains("最终输入"),
                "body={body}, key={key:?}: {text}"
            );
            assert!(!text.contains("候选输入"));
            assert_eq!(app.project.content_baseline(), baseline);
            assert!(!app.manuscript.creation_dismissed);
        }
    }
}

#[test]
fn commit_with_enter_does_not_add_a_newline_or_blur_the_text_field() {
    for body in [false, true] {
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
            vec![Event::Ime(egui::ImeEvent::Preedit("候选输入".into()))],
        );
        let expected = input_text(&app, body).replace("候选输入", "最终输入");
        native_frame(
            &ctx,
            &mut app,
            vec![
                Event::Ime(egui::ImeEvent::Commit("最终输入".into())),
                key_event(egui::Key::Enter, true),
            ],
        );
        native_frame(&ctx, &mut app, vec![key_event(egui::Key::Enter, false)]);
        assert_eq!(input_text(&app, body), expected, "body={body}");
        assert!(ctx.memory(|memory| memory.has_focus(focus)));
        assert_eq!(app.project.content_baseline(), baseline);
        for pressed in [true, false] {
            native_frame(&ctx, &mut app, vec![key_event(egui::Key::Tab, pressed)]);
        }
        native_frame(&ctx, &mut app, vec![]);
        assert!(
            !ctx.memory(|memory| memory.has_focus(focus)),
            "normal Tab must work again: body={body}"
        );
        assert_eq!(input_text(&app, body), expected);
    }
}
