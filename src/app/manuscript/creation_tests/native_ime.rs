//! 完整 app update 的原生式 IME 生命周期＋真实鼠标按下/松开；无物理系统 IME 冒充。
use super::*;
mod conflicts;
mod empty_lifecycle;
mod keys;
mod projection;
mod raw;

fn native_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    let focused = ctx.data_mut(|data| {
        let key = egui::Id::new("native-ime-test-window-focus");
        let mut focused = data.get_temp::<bool>(key).unwrap_or(true);
        for event in &events {
            if let Event::WindowFocused(value) = event {
                focused = *value;
            }
        }
        data.insert_temp(key, focused);
        focused
    });
    let mut raw = RawInput {
        focused,
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 1500.0))),
        events,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest());
    })
}

fn native_settle(ctx: &egui::Context, app: &mut WorldeditApp) -> egui::FullOutput {
    for _ in 0..3 {
        native_frame(ctx, app, vec![]);
    }
    native_frame(ctx, app, vec![])
}

fn native_position(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) -> egui::Pos2 {
    let output = native_settle(ctx, app);
    output
        .shapes
        .iter()
        .find_map(|shape| {
            let mut texts = Vec::new();
            text_shapes(&shape.shape, &mut texts);
            texts
                .into_iter()
                .filter(|text| text.galley.text() == label)
                .map(|text| text.pos + text.galley.rect.center().to_vec2())
                .find(|pos| shape.clip_rect.contains(*pos))
        })
        .unwrap_or_else(|| panic!("missing {label}: {}", labels(&output)))
}

fn native_click(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    label: &str,
    ime: Option<(bool, egui::ImeEvent)>,
) {
    let pos = native_position(ctx, app, label);
    for pressed in [true, false] {
        let mut events = vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        if let Some((phase, event)) = &ime {
            if *phase == pressed {
                events.push(Event::Ime(event.clone()));
            }
        }
        native_frame(ctx, app, events);
    }
}

fn native_titles(ctx: &egui::Context, app: &mut WorldeditApp, paste: bool) {
    native_click(ctx, app, "开始写作", None);
    native_click(ctx, app, "明确使用现有起点 start", None);
    native_click(ctx, app, "这部作品叫什么？", None);
    native_frame(ctx, app, vec![Event::Ime(egui::ImeEvent::Enabled)]);
    let input = |text: &str| {
        if paste {
            Event::Paste(text.into())
        } else {
            Event::Text(text.into())
        }
    };
    native_frame(ctx, app, vec![input("剪贴板书名")]);
    for pressed in [true, false] {
        native_frame(
            ctx,
            app,
            vec![Event::Key {
                key: egui::Key::Tab,
                physical_key: Some(egui::Key::Tab),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
    native_frame(
        ctx,
        app,
        vec![Event::Ime(egui::ImeEvent::Enabled), input("剪贴板章名")],
    );
    let form = app.manuscript.creation.as_ref().unwrap();
    assert_eq!(form.book_title, "剪贴板书名");
    assert_eq!(form.chapter_title, "剪贴板章名");
}

#[test]
fn pure_enabled_disabled_lifecycle_after_text_or_paste_allows_first_return_and_preview_click() {
    for paste in [false, true] {
        for disabled_on_press in [false, true] {
            for preview in [false, true] {
                let (ctx, mut app) = blank();
                native_titles(&ctx, &mut app, paste);
                let baseline = app.project.content_baseline();
                native_click(
                    &ctx,
                    &mut app,
                    if preview {
                        "预览创建计划"
                    } else {
                        "返回，保留输入"
                    },
                    Some((disabled_on_press, egui::ImeEvent::Disabled)),
                );
                let form = app.manuscript.creation.as_ref().unwrap();
                if preview {
                    assert!(form.reviewing, "paste={paste}, disabled_on_press={disabled_on_press}: first preview click was swallowed");
                } else {
                    assert!(app.manuscript.creation_dismissed, "paste={paste}, disabled_on_press={disabled_on_press}: first return click was swallowed");
                }
                assert_eq!(form.book_title, "剪贴板书名");
                assert_eq!(form.chapter_title, "剪贴板章名");
                assert_eq!(app.project.content_baseline(), baseline);
                assert!(app.history.is_empty());
            }
        }
    }
}

#[test]
fn genuine_preedit_and_commit_continue_to_block_actions_and_preserve_composed_title() {
    for preview in [false, true] {
        let (ctx, mut app) = blank();
        native_titles(&ctx, &mut app, true);
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Preedit("未提交".into()))],
        );
        let composition = app
            .manuscript
            .creation
            .as_ref()
            .unwrap()
            .chapter_title
            .clone();
        assert!(composition.contains("未提交"));
        native_click(
            &ctx,
            &mut app,
            if preview {
                "预览创建计划"
            } else {
                "返回，保留输入"
            },
            None,
        );
        assert!(
            !app.manuscript.creation_dismissed
                && !app.manuscript.creation.as_ref().unwrap().reviewing
        );
        assert_eq!(
            app.manuscript.creation.as_ref().unwrap().chapter_title,
            composition
        );
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Commit("已提交".into()))],
        );
        assert!(app
            .manuscript
            .creation
            .as_ref()
            .unwrap()
            .chapter_title
            .contains("已提交"));
        // 一个新的提交帧不能借已按下的按钮完成创建／返回。
        native_click(
            &ctx,
            &mut app,
            if preview {
                "预览创建计划"
            } else {
                "返回，保留输入"
            },
            Some((false, egui::ImeEvent::Commit("".into()))),
        );
        assert!(
            !app.manuscript.creation_dismissed
                && !app.manuscript.creation.as_ref().unwrap().reviewing
        );
        assert!(app
            .manuscript
            .creation
            .as_ref()
            .unwrap()
            .chapter_title
            .contains("已提交"));
        native_frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Disabled)]);
        native_click(
            &ctx,
            &mut app,
            if preview {
                "预览创建计划"
            } else {
                "返回，保留输入"
            },
            None,
        );
        assert!(if preview {
            app.manuscript.creation.as_ref().unwrap().reviewing
        } else {
            app.manuscript.creation_dismissed
        });
        assert!(app.history.is_empty());
    }
}

fn native_body(ctx: &egui::Context, app: &mut WorldeditApp, paste: bool) {
    create_start(ctx, app);
    app.personal.settings.focus = true;
    app.manuscript.reader_open = false;
    native_click(ctx, app, "从这里写下第一段……", None);
    native_frame(ctx, app, vec![Event::Ime(egui::ImeEvent::Enabled)]);
    native_frame(
        ctx,
        app,
        vec![if paste {
            Event::Paste("剪贴板正文。".into())
        } else {
            Event::Text("剪贴板正文。".into())
        }],
    );
    assert!(app.manuscript.writing_buffers[&app.project.entry]
        .source()
        .contains("剪贴板正文。"));
}

fn body_input(app: &WorldeditApp) -> String {
    let buffer = &app.manuscript.writing_buffers[&app.project.entry];
    let target = TargetRef::new("event", "start");
    let projection = app.project.project_writing_buffer(buffer, &target).unwrap();
    let (offset, text) = if let Some(slot) = &projection.empty_prose_slot {
        (slot.offset(), slot.text())
    } else {
        let block = projection
            .blocks
            .iter()
            .find(|block| block.kind == worldline_core::manuscript::WritingBlockKind::Prose)
            .unwrap();
        (block.range.start, block.text.as_str())
    };
    app.manuscript
        .writing_view
        .prose_text(buffer.path(), &target, offset, text)
}

#[test]
fn pure_lifecycle_after_text_or_paste_allows_first_body_apply_click() {
    for paste in [false, true] {
        for disabled_on_press in [false, true] {
            let (ctx, mut app) = blank();
            native_body(&ctx, &mut app, paste);
            let before = app.project.content_baseline();
            assert!(!app
                .project
                .document(&app.project.entry)
                .unwrap()
                .contains("剪贴板正文。"));
            native_click(
                &ctx,
                &mut app,
                "应用正文草稿",
                Some((disabled_on_press, egui::ImeEvent::Disabled)),
            );
            assert_ne!(app.project.content_baseline(), before, "paste={paste}, disabled_on_press={disabled_on_press}: first body apply click was swallowed");
            assert!(app
                .project
                .document(&app.project.entry)
                .unwrap()
                .contains("剪贴板正文。"));
            assert!(!app
                .manuscript
                .writing_buffers
                .get(&app.project.entry)
                .is_some_and(WritingBuffer::is_changed));
            assert_eq!(app.history.len(), 2);
        }
    }
}

#[test]
fn true_body_composition_keeps_focus_until_commit_and_never_applies_the_commit_frame() {
    for commit_on_press in [false, true] {
        let (ctx, mut app) = blank();
        native_body(&ctx, &mut app, true);
        let before = app.project.content_baseline();
        let focus = ctx.memory(|memory| memory.focused()).unwrap();
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Preedit("候选正文".into()))],
        );
        let composed = body_input(&app);
        assert!(composed.contains("候选正文"));
        native_click(&ctx, &mut app, "应用正文草稿", None);
        assert_eq!(app.project.content_baseline(), before);
        assert_eq!(app.history.len(), 1);
        assert_eq!(body_input(&app), composed);
        assert!(ctx.memory(|memory| memory.has_focus(focus)));
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Commit("提交正文".into()))],
        );
        let body = app.manuscript.writing_buffers[&app.project.entry].source();
        assert!(body.contains("提交正文"), "{body}");
        assert!(!body.contains("候选正文"), "{body}");
        native_click(
            &ctx,
            &mut app,
            "应用正文草稿",
            Some((commit_on_press, egui::ImeEvent::Commit(String::new()))),
        );
        assert_eq!(app.project.content_baseline(), before);
        assert_eq!(app.history.len(), 1);
        native_frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Disabled)]);
        native_click(&ctx, &mut app, "应用正文草稿", None);
        assert!(app
            .project
            .document(&app.project.entry)
            .unwrap()
            .contains("提交正文"));
        assert_eq!(app.history.len(), 2);
    }
}

#[test]
fn a_press_during_composition_cannot_replay_as_an_action_when_released_after_commit() {
    for body in [false, true] {
        let (ctx, mut app) = blank();
        if body {
            native_body(&ctx, &mut app, true);
        } else {
            native_titles(&ctx, &mut app, true);
        }
        let before = app.project.content_baseline();
        let history = app.history.len();
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Preedit("候选".into()))],
        );
        let button = if body {
            "应用正文草稿"
        } else {
            "预览创建计划"
        };
        let pos = native_position(&ctx, &mut app, button);
        native_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Commit("确认文字".into()))],
        );
        native_frame(&ctx, &mut app, vec![]);
        native_frame(
            &ctx,
            &mut app,
            vec![Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(app.project.content_baseline(), before);
        assert_eq!(app.history.len(), history);
        if body {
            assert!(app.manuscript.writing_buffers[&app.project.entry]
                .source()
                .contains("确认文字"));
        } else {
            let form = app.manuscript.creation.as_ref().unwrap();
            assert!(!form.reviewing);
            assert!(form.chapter_title.contains("确认文字"));
        }
        native_click(&ctx, &mut app, button, None);
        if body {
            assert!(app
                .project
                .document(&app.project.entry)
                .unwrap()
                .contains("确认文字"));
            assert_eq!(app.history.len(), history + 1);
        } else {
            assert!(app.manuscript.creation.as_ref().unwrap().reviewing);
        }
    }
}

#[test]
fn cancelling_preedit_with_empty_text_releases_only_the_next_intentional_action() {
    for body in [false, true] {
        let (ctx, mut app) = blank();
        if body {
            native_body(&ctx, &mut app, true);
        } else {
            native_titles(&ctx, &mut app, true);
        }
        let before = app.project.content_baseline();
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Preedit("取消候选".into()))],
        );
        native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Preedit(String::new()))],
        );
        assert_eq!(app.project.content_baseline(), before);
        if body {
            let source = app.manuscript.writing_buffers[&app.project.entry].source();
            assert!(source.contains("剪贴板正文。"));
            assert!(!source.contains("取消候选"));
        } else {
            assert_eq!(
                app.manuscript.creation.as_ref().unwrap().chapter_title,
                "剪贴板章名"
            );
        }
        native_click(
            &ctx,
            &mut app,
            if body {
                "应用正文草稿"
            } else {
                "返回，保留输入"
            },
            None,
        );
        if body {
            assert!(app
                .project
                .document(&app.project.entry)
                .unwrap()
                .contains("剪贴板正文。"));
        } else {
            assert!(app.manuscript.creation_dismissed);
        }
    }
}
