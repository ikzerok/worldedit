//! 源码选区建议属于编辑焦点；不能浮在命令、搜索或受保护输入之上。
use super::tests::{app, select};
use crate::app::WorldeditApp;
use egui::{Context, Event, Key, Modifiers};

fn frame(ctx: &Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
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
            app.project_search(ctx);
            app.command_window(ctx);
            app.entity_editor_window(ctx);
            app.capture_edit_focus(ctx);
        },
    )
}

fn text_position(output: &egui::FullOutput, wanted: &str) -> Option<egui::Pos2> {
    fn find(shape: &egui::Shape, wanted: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.job.text == wanted => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, wanted)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|shape| find(&shape.shape, wanted))
}

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers,
    }
}

fn selected_source() -> (Context, WorldeditApp, String) {
    let (ctx, mut app, _) = app();
    let manifest = app.project.root.join(".world/project.json");
    app.project.create_authoring_document(&manifest, br#"{"schema_version":1,"language_version":"1.13","required_features":["content.entities.v1","content.object_refs.v1"]}"#.to_vec()).unwrap();
    let source = "event start\n  原文 手选中文🧭 当前稿\n".to_owned();
    app.project
        .set_text(&app.active_file.clone(), source.clone())
        .unwrap();
    app.recompile();
    frame(&ctx, &mut app, vec![]);
    let index = source[..source.find("手选").unwrap()].chars().count();
    select(&ctx, &app, index, index);
    // 通过真实编辑键盘输入扩展选区，避免把直接设双端状态当手选行为。
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![key(Key::ArrowRight, Modifiers::SHIFT)]);
    }
    for _ in 0..2 {
        frame(&ctx, &mut app, vec![]);
    }
    (ctx, app, source)
}

#[test]
fn source_selection_suggestion_keeps_manual_button_and_ctrl_enter_authoring() {
    for keyboard in [false, true] {
        let (ctx, mut app, source) = selected_source();
        let output = frame(&ctx, &mut app, vec![]);
        let position = text_position(&output, "从选中文本建档").expect("正常手选应保留建档建议");
        if keyboard {
            frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::COMMAND)]);
        } else {
            for pressed in [true, false] {
                frame(
                    &ctx,
                    &mut app,
                    vec![
                        Event::PointerMoved(position),
                        Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        },
                    ],
                );
            }
        }
        let form = app.entity_editor.as_ref().unwrap_or_else(|| {
            panic!(
                "建议按钮与CtrlEnter都能建档 keyboard={keyboard} message={:?}",
                app.message
            )
        });
        assert_eq!(form.draft.display, "手选中");
        assert_eq!(
            form.source_selection.as_ref().unwrap().expected_text,
            "手选中"
        );
        assert_eq!(app.project.document(&app.active_file).unwrap(), source);
        assert!(text_position(&frame(&ctx, &mut app, vec![]), "从选中文本建档").is_none());
    }
}

#[test]
fn source_selection_suggestion_is_hidden_on_focus_loss_and_above_editor_layers() {
    let (ctx, mut app, source) = selected_source();
    let id = egui::Id::new(("source", &app.active_file));
    ctx.memory_mut(|memory| memory.surrender_focus(id));
    assert!(text_position(&frame(&ctx, &mut app, vec![]), "从选中文本建档").is_none());
    ctx.memory_mut(|memory| memory.request_focus(id));
    assert!(text_position(&frame(&ctx, &mut app, vec![]), "从选中文本建档").is_some());
    app.open_commands(&ctx, true);
    for _ in 0..3 {
        assert!(text_position(&frame(&ctx, &mut app, vec![]), "从选中文本建档").is_none());
    }
    let output = frame(&ctx, &mut app, vec![]);
    assert!(text_position(&output, "任务命令").is_some());
    assert!(text_position(&output, "从选中文本建档").is_none());
    frame(&ctx, &mut app, vec![key(Key::Escape, Modifiers::NONE)]);
    app.open_search(&ctx, false, false);
    for _ in 0..3 {
        assert!(text_position(&frame(&ctx, &mut app, vec![]), "从选中文本建档").is_none());
    }
    let output = frame(&ctx, &mut app, vec![]);
    assert!(text_position(&output, "查找与替换").is_some());
    assert!(text_position(&output, "从选中文本建档").is_none());
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
}

#[test]
fn source_selection_suggestion_waits_for_ime_and_keeps_selected_text() {
    let (ctx, mut app, source) = selected_source();
    let id = egui::Id::new(("source", &app.active_file));
    let selected = egui::TextEdit::load_state(&ctx, id)
        .unwrap()
        .cursor
        .char_range();
    let output = frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Enabled)]);
    assert!(app.ime_composing);
    assert!(text_position(&output, "从选中文本建档").is_none());
    assert_eq!(
        egui::TextEdit::load_state(&ctx, id)
            .unwrap()
            .cursor
            .char_range(),
        selected
    );
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
}

#[test]
fn diagnostic_program_selection_and_back_never_offer_passive_creation() {
    let (ctx, mut app, source) = selected_source();
    let start = source.find("手选").unwrap();
    let path = app.active_file.clone();
    crate::app::search::request_diagnostic_selection(
        &ctx, path.clone(), source.clone(), start..start + "手选中".len(),
    );
    for _ in 0..3 {
        assert!(text_position(&frame(&ctx, &mut app, vec![]), "从选中文本建档").is_none());
    }
    let position = app.author_location(Some(&ctx));
    assert!(position.source_selection_diagnostic);
    app.remember_author_location(position);
    select(&ctx, &app, 0, 0);
    app.author_back(&ctx);
    for _ in 0..3 {
        assert!(text_position(&frame(&ctx, &mut app, vec![]), "从选中文本建档").is_none());
    }
    assert_eq!(app.project.document(&path).unwrap(), source);
    assert!(app.entity_editor.is_none());
    // 新的Shift选字是明确人工意图，即使问题工具仍在也恢复建议。
    app.personal.settings.diagnostics = true;
    frame(&ctx, &mut app, vec![key(Key::ArrowRight, Modifiers::SHIFT)]);
    assert!(text_position(&frame(&ctx, &mut app, vec![]), "从选中文本建档").is_some());
    frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::COMMAND)]);
    assert!(app.entity_editor.is_some());
    assert_eq!(app.project.document(&path).unwrap(), source);
}

#[test]
fn explicit_ctrl_enter_can_use_diagnostic_selection_without_passive_popup() {
    let (ctx, mut app, source) = selected_source();
    let start = source.find("手选").unwrap();
    crate::app::search::request_diagnostic_selection(
        &ctx, app.active_file.clone(), source.clone(), start..start + "手选中".len(),
    );
    assert!(text_position(&frame(&ctx, &mut app, vec![]), "从选中文本建档").is_none());
    frame(&ctx, &mut app, vec![key(Key::Enter, Modifiers::COMMAND)]);
    assert_eq!(app.entity_editor.as_ref().unwrap().draft.display, "手选中");
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
}
