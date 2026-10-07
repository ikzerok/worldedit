use super::*;

pub(super) fn two_chapters(ctx: &egui::Context, app: &mut WorldeditApp, separate_file: bool) {
    create_start(ctx, app);
    click(ctx, app, "新建章节");
    settle(ctx, app);
    frame(ctx, app, vec![Event::Text("远处的灯".into())]);
    click(ctx, app, "新建空白正文");
    if separate_file {
        // 设置高级路径测试条件；预览与提交仍按真实按钮、完整 core 计划执行。
        let form = app.manuscript.creation.as_mut().unwrap();
        form.new_file = true;
        form.source_path = "later.wl".into();
        form.invalidate();
    }
    click(ctx, app, "预览创建计划");
    assert!(
        app.manuscript.creation.as_ref().unwrap().error.is_none(),
        "{:?}",
        app.manuscript.creation.as_ref().unwrap().error
    );
    click(ctx, app, "创建并写作");
    app.project.save().unwrap();
    assert_eq!(
        app.project.manuscript_index("book").unwrap().entries.len(),
        2
    );
}

pub(super) fn write_empty(ctx: &egui::Context, app: &mut WorldeditApp, text: &str) {
    settle(ctx, app);
    let (target, path) = app.manuscript.active_writing_target().unwrap();
    let buffer = app.manuscript.writing_buffers.get(&path).unwrap();
    let slot = app
        .project
        .project_writing_buffer(buffer, &target)
        .unwrap()
        .empty_prose_slot
        .unwrap();
    let id = egui::Id::new((
        "writing-prose",
        &path,
        &target.kind,
        &target.id,
        slot.offset(),
    ));
    let pos = ctx.read_response(id).unwrap().rect.center();
    for pressed in [true, false] {
        frame(
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
    // 同一框连续两次点击可能按真实双击语义选中原文字；本助手明确追加，先用 End 折叠选区。
    key(ctx, app, egui::Key::End);
    let cursor = egui::TextEdit::load_state(ctx, id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!(cursor.primary.index, cursor.secondary.index);
    frame(ctx, app, vec![Event::Text(text.into())]);
}

#[test]
fn failed_first_chapter_input_survives_another_chapter_success_or_failure() {
    for separate_file in [false, true] {
        for second_succeeds in [false, true] {
            let (ctx, mut app) = blank();
            two_chapters(&ctx, &mut app, separate_file);
            click(&ctx, &mut app, "潮汐初起");
            let original = std::fs::read(&app.project.entry).unwrap();
            let mut changed = original.clone();
            changed.extend_from_slice(b"\n// external edit\n");
            std::fs::write(&app.project.entry, changed).unwrap();
            write_empty(&ctx, &mut app, "第一章受保护的输入");
            assert!(app.manuscript.writing_view.has_retained_input());
            assert!(!app
                .manuscript
                .writing_buffers
                .get(&app.project.entry)
                .unwrap()
                .source()
                .contains("第一章受保护的输入"));
            if second_succeeds {
                std::fs::write(&app.project.entry, &original).unwrap();
            }
            click(&ctx, &mut app, "远处的灯");
            write_empty(&ctx, &mut app, "第二章独立的输入");
            assert!(
                app.manuscript.writing_view.has_retained_input(),
                "chapter B must never clear A"
            );
            let (_, second_path) = app.manuscript.active_writing_target().unwrap();
            assert_eq!(
                app.manuscript
                    .writing_buffers
                    .get(&second_path)
                    .unwrap()
                    .source()
                    .contains("第二章独立的输入"),
                second_succeeds
            );
            click(&ctx, &mut app, "潮汐初起");
            let output = labels(&settle(&ctx, &mut app));
            assert!(output.contains("第一章受保护的输入"), "{output}");
            if !second_succeeds {
                assert!(
                    output.contains("第二章独立的输入"),
                    "both failed edits need separate recovery"
                );
            }
            std::fs::remove_dir_all(&app.project.root).unwrap();
        }
    }
}

#[test]
fn another_dirty_authoring_form_blocks_creation_preview_and_apply_without_losing_either() {
    let (ctx, mut app) = blank();
    titles(&ctx, &mut app);
    click(&ctx, &mut app, "明确使用现有起点 start");
    click(&ctx, &mut app, "预览创建计划");
    assert!(app.manuscript.creation.as_ref().unwrap().reviewing);
    app.new_event(None);
    app.event_editor.as_mut().unwrap().draft.body = "另一份仍未提交的事件文字\n-> END".into();
    assert!(app.dirty_draft_names().contains(&"事件正文与分支"));
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, "创建并写作");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.project.manuscript_indices().is_empty());
    assert!(app.history.is_empty());
    assert!(app
        .event_editor
        .as_ref()
        .unwrap()
        .draft
        .body
        .contains("另一份仍未提交的事件文字"));
    click(&ctx, &mut app, "修改创建计划");
    click(&ctx, &mut app, "预览创建计划");
    let form = app.manuscript.creation.as_ref().unwrap();
    assert!(!form.reviewing, "blocked preview must not advance the form");
    assert_eq!(form.book_title, "海边的灯");
    assert_eq!(form.chapter_title, "潮汐初起");
    assert!(app
        .event_editor
        .as_ref()
        .unwrap()
        .draft
        .body
        .contains("另一份仍未提交的事件文字"));
    assert!(labels(&settle(&ctx, &mut app)).contains("仍有未提交的事件正文与分支输入"));
}
