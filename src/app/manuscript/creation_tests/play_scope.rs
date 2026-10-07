use super::*;

fn dialog_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 900.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            let _theme = crate::theme::configure_appearance(ctx, app.personal.appearance());
            app.play_scope_dialog(ctx);
        },
    )
}

fn dialog_settle(ctx: &egui::Context, app: &mut WorldeditApp) -> egui::FullOutput {
    // 新 Modal 的测量帧不保证产生最终文字图元；与真实点击助手采用相同稳定帧规则。
    for _ in 0..3 {
        dialog_frame(ctx, app, vec![]);
    }
    dialog_frame(ctx, app, vec![])
}

fn dialog_click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for _ in 0..3 {
        dialog_frame(ctx, app, vec![]);
    }
    let output = dialog_frame(ctx, app, vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| {
            let mut texts = Vec::new();
            text_shapes(&shape.shape, &mut texts);
            texts
                .into_iter()
                .find(|text| text.galley.text() == label)
                .map(|text| text.pos + text.galley.rect.center().to_vec2())
        })
        .unwrap_or_else(|| panic!("missing {label}: {}", labels(&output)));
    for pressed in [true, false] {
        dialog_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

#[test]
fn a_new_event_form_alone_requires_run_confirmation_and_cancel_return_keep_it() {
    let (ctx, mut app) = blank();
    titles(&ctx, &mut app);
    click(&ctx, &mut app, "新建空白正文");
    let baseline = app.project.content_baseline();
    let inputs = app.unapplied_play_inputs();
    assert_eq!(inputs.len(), 1);
    assert!(inputs[0].source.contains("待创建正文"));
    app.start_play();
    assert!(app.play.is_none() && app.play_confirmation.is_some());
    dialog_click(&ctx, &mut app, "取消运行");
    assert!(app.play.is_none() && app.play_confirmation.is_none());
    assert_eq!(
        app.manuscript.creation.as_ref().unwrap().chapter_title,
        "潮汐初起"
    );
    app.start_play();
    dialog_click(&ctx, &mut app, "返回处理");
    assert_eq!(app.tab, Tab::Manuscript);
    assert!(app.play.is_none() && app.play_confirmation.is_none());
    assert_eq!(
        app.manuscript.creation.as_ref().unwrap().book_title,
        "海边的灯"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn unchanged_creation_title_cannot_reuse_confirmation_after_execution_fields_change() {
    let (ctx, mut app) = blank();
    titles(&ctx, &mut app);
    click(&ctx, &mut app, "新建空白正文");
    app.start_play();
    let shown = app.play_confirmation.take().unwrap();
    let inputs = app.unapplied_play_inputs();
    app.manuscript.creation.as_mut().unwrap().storyline = "other_storyline".into();
    assert_eq!(
        app.unapplied_play_inputs(),
        inputs,
        "same displayed title and identity"
    );
    app.confirm_play_scope(&ctx, shown);
    assert!(app.play.is_none() && app.play_confirmation.is_some());
    assert!(labels(&dialog_settle(&ctx, &mut app)).contains("草稿或运行设置已变化"));
    let shown = app.play_confirmation.take().unwrap();
    app.manuscript.creation.as_mut().unwrap().source_path = "other.wl".into();
    app.confirm_play_scope(&ctx, shown);
    assert!(app.play.is_none() && app.play_confirmation.is_some());
    dialog_click(&ctx, &mut app, "取消运行");
    assert_eq!(
        app.manuscript.creation.as_ref().unwrap().source_path,
        "other.wl"
    );
    assert_eq!(
        app.manuscript.creation.as_ref().unwrap().chapter_title,
        "潮汐初起"
    );
}

#[test]
fn retained_input_alone_is_visible_to_run_scope_and_changed_text_requires_reconfirmation() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    app.project.save().unwrap();
    let original = std::fs::read(&app.project.entry).unwrap();
    let mut changed = original.clone();
    changed.extend_from_slice(b"\n// external change\n");
    std::fs::write(&app.project.entry, changed).unwrap();
    retention::write_empty(&ctx, &mut app, "未插入甲稿");
    assert!(!app.manuscript.writing_buffers()[0].is_changed());
    let inputs = app.unapplied_play_inputs();
    assert_eq!(inputs.len(), 1);
    assert!(inputs[0].source.contains("未插入正文"));
    app.start_play();
    assert!(app.play.is_none() && app.play_confirmation.is_some());
    let shown = app.play_confirmation.take().unwrap();
    // 失败稿只供复制/明确清除，不能隐式作为新的 core 正文回填。
    click(&ctx, &mut app, "清除保留输入");
    click(&ctx, &mut app, "确认清除保留输入");
    retention::write_empty(&ctx, &mut app, "未插入甲稿追加乙稿");
    assert_eq!(
        app.unapplied_play_inputs(),
        inputs,
        "same target/path/offset labels"
    );
    app.confirm_play_scope(&ctx, shown);
    assert!(app.play.is_none() && app.play_confirmation.is_some());
    assert!(labels(&dialog_settle(&ctx, &mut app)).contains("草稿或运行设置已变化"));
    dialog_click(&ctx, &mut app, "返回处理");
    let output = labels(&settle(&ctx, &mut app));
    assert!(
        output.contains("未插入甲稿") && output.contains("追加乙稿"),
        "{output}"
    );
    assert!(!app.manuscript.writing_buffers()[0].is_changed());
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn existing_source_intent_and_pure_arrangement_keep_the_nonruntime_policy() {
    let (ctx, mut app) = blank();
    titles(&ctx, &mut app);
    click(&ctx, &mut app, "明确使用现有起点 start");
    assert!(app.manuscript.has_unsubmitted_work());
    assert!(app.unapplied_play_inputs().is_empty());
    app.start_play();
    assert!(app.play.is_some() && app.play_confirmation.is_none());
    assert_eq!(
        app.manuscript.creation.as_ref().unwrap().chapter_title,
        "潮汐初起"
    );
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    let book = app.manuscript.books.get_mut("book").unwrap();
    book.draft.title = "只改阅读编排".into();
    book.changed = true;
    assert!(app.manuscript.has_unsubmitted_work());
    assert!(app.unapplied_play_inputs().is_empty());
    app.start_play();
    assert!(app.play.is_some() && app.play_confirmation.is_none());
    assert_eq!(app.manuscript.books["book"].draft.title, "只改阅读编排");
}
