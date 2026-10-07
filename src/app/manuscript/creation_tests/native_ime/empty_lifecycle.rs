use super::*;

#[test]
fn inactive_empty_preedit_and_post_commit_cleanup_allow_the_first_return_preview_and_apply_click() {
    for action in ["返回，保留输入", "预览创建计划", "应用正文草稿"] {
        for after_commit in [false, true] {
            for on_press in [false, true] {
                let (ctx, mut app) = blank();
                let body = action == "应用正文草稿";
                if body {
                    native_body(&ctx, &mut app, true);
                } else {
                    native_titles(&ctx, &mut app, true);
                }
                if after_commit {
                    native_frame(
                        &ctx,
                        &mut app,
                        vec![Event::Ime(egui::ImeEvent::Preedit("待提交".into()))],
                    );
                    native_frame(
                        &ctx,
                        &mut app,
                        vec![Event::Ime(egui::ImeEvent::Commit("真实提交".into()))],
                    );
                }
                let baseline = app.project.content_baseline();
                let expected_body = body.then(|| {
                    app.manuscript.writing_buffers[&app.project.entry]
                        .source()
                        .to_owned()
                });
                let expected_title = app
                    .manuscript
                    .creation
                    .as_ref()
                    .map(|form| form.chapter_title.clone());
                native_click(
                    &ctx,
                    &mut app,
                    action,
                    Some((on_press, egui::ImeEvent::Preedit(String::new()))),
                );
                if let Some(source) = expected_body {
                    assert_eq!(
                        app.project.document(&app.project.entry).unwrap(),
                        source,
                        "{action}/{after_commit}/{on_press}"
                    );
                    assert_eq!(app.history.len(), 2);
                } else {
                    let form = app.manuscript.creation.as_ref().unwrap();
                    assert_eq!(form.chapter_title, expected_title.unwrap());
                    assert!(
                        if action == "返回，保留输入" {
                            app.manuscript.creation_dismissed
                        } else {
                            form.reviewing
                        },
                        "{action}/{after_commit}/{on_press}"
                    );
                    assert_eq!(app.project.content_baseline(), baseline);
                    assert!(app.history.is_empty());
                }
            }
        }
    }
}

#[test]
fn empty_preedit_cancelling_a_real_composition_still_blocks_its_mouse_gesture() {
    for body in [false, true] {
        for on_press in [false, true] {
            let (ctx, mut app) = blank();
            if body {
                native_body(&ctx, &mut app, true);
            } else {
                native_titles(&ctx, &mut app, true);
            }
            let baseline = app.project.content_baseline();
            native_frame(
                &ctx,
                &mut app,
                vec![Event::Ime(egui::ImeEvent::Preedit("待取消".into()))],
            );
            let action = if body {
                "应用正文草稿"
            } else {
                "返回，保留输入"
            };
            native_click(
                &ctx,
                &mut app,
                action,
                Some((on_press, egui::ImeEvent::Preedit(String::new()))),
            );
            assert_eq!(app.project.content_baseline(), baseline);
            assert!(!app.manuscript.creation_dismissed);
            if body {
                assert!(!body_input(&app).contains("待取消"));
            } else {
                assert_eq!(
                    app.manuscript.creation.as_ref().unwrap().chapter_title,
                    "剪贴板章名"
                );
            }
            native_click(&ctx, &mut app, action, None);
            if body {
                assert_eq!(app.history.len(), 2);
            } else {
                assert!(app.manuscript.creation_dismissed);
            }
        }
    }
}
