use super::*;

fn json_focus(ctx: &egui::Context, app: &mut WorldeditApp) {
    click(ctx, app, 19, "新建空白模板");
    click(ctx, app, 19, "高级 JSON");
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("template-manager-json")));
    let _ = frame(ctx, app, vec![], 19);
}
#[test]
fn template_ime_enabled_and_idle_empty_preedit_allow_first_preview_click() {
    for event in [
        egui::ImeEvent::Enabled,
        egui::ImeEvent::Preedit(String::new()),
    ] {
        let (ctx, mut app) = app();
        json_focus(&ctx, &mut app);
        let _ = frame(&ctx, &mut app, vec![Event::Ime(event)], 19);
        assert!(!app.template_manager.composition_busy());
        click_without_settling(&ctx, &mut app, 19, "预览导入 / 替换");
        assert!(app.template_manager.preview.is_some(), "{:?}", app.io_error);
    }
}
#[test]
fn template_ime_candidate_keys_preserve_focus_input_and_navigation() {
    for key in [egui::Key::Enter, egui::Key::Tab, egui::Key::Escape] {
        let (ctx, mut app) = super::native_input::setup();
        let before = app.project.content_baseline();
        let _ = super::native_input::native_frame(
            &ctx,
            &mut app,
            vec![Event::Ime(egui::ImeEvent::Enabled)],
            false,
        );
        let _ = super::native_input::native_frame(
            &ctx,
            &mut app,
            vec![
                Event::Ime(egui::ImeEvent::Preedit("组合候选".into())),
                Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            false,
        );
        assert!(app.template_manager.composition_busy());
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(egui::Id::new("template-manager-json"))
        );
        assert!(app.template_manager.preview.is_none());
        assert_eq!(app.project.content_baseline(), before);
        super::native_input::native_click(&ctx, &mut app, "新建空白模板");
        assert!(
            app.template_manager.pending.is_none(),
            "组合时不能切换或发起丢弃"
        );
        assert!(app.template_manager.editor.contains("组合候选"));
    }
}
#[test]
fn template_ime_commit_blocks_the_entire_pointer_gesture_then_allows_next_click() {
    let (ctx, mut app) = app();
    json_focus(&ctx, &mut app);
    let _ = frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Enabled)],
        19,
    );
    let output = frame(&ctx, &mut app, vec![], 19);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position(&shape.shape, "预览导入 / 替换"))
        .unwrap();
    for pressed in [true, false] {
        let mut events = vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        if pressed {
            events.insert(0, Event::Ime(egui::ImeEvent::Commit(String::new())));
        }
        let _ = frame(&ctx, &mut app, events, 19);
        assert!(app.template_manager.preview.is_none());
    }
    let _ = frame(&ctx, &mut app, vec![], 19);
    assert!(!app.template_manager.composition_busy());
    click_without_settling(&ctx, &mut app, 19, "预览导入 / 替换");
    assert!(app.template_manager.preview.is_some(), "{:?}", app.io_error);
}
#[test]
fn unrelated_window_ime_does_not_claim_template_definition() {
    let (ctx, mut app) = app();
    json_focus(&ctx, &mut app);
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("other-window-text")));
    let _ = frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Preedit("其他窗口".into()))],
        19,
    );
    assert!(!app.template_manager.composition_busy());
}
