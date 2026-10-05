//! 真实 egui 输入与几何回归；默认/prototype 共用，不代替物理桌面、IME 或读屏验收。
use super::*;
use egui::{Event, Key, Modifiers, Rect, Vec2};
fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: Vec2,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| {
            app.author_shortcuts(ctx);
            app.command_window(ctx);
            app.entity_source_move_window(ctx);
            app.capture_edit_focus(ctx);
        },
    )
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
fn press(ctx: &egui::Context, app: &mut WorldeditApp, key_value: Key, modifiers: Modifiers) {
    frame(
        ctx,
        app,
        egui::vec2(800.0, 600.0),
        vec![key(key_value, modifiers)],
    );
    let mut released = key(key_value, modifiers);
    if let Event::Key { pressed, .. } = &mut released {
        *pressed = false;
    }
    frame(ctx, app, egui::vec2(800.0, 600.0), vec![released]);
}
fn settle(ctx: &egui::Context, app: &mut WorldeditApp, size: Vec2) -> egui::FullOutput {
    for _ in 0..8 {
        frame(ctx, app, size, vec![]);
    }
    frame(ctx, app, size, vec![])
}
fn visible(output: &egui::FullOutput, label: &str) -> Option<Rect> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(text) if text.galley.text() == label => {
            let rect = text.galley.rect.translate(text.pos.to_vec2());
            shape.clip_rect.contains_rect(rect).then_some(rect)
        }
        _ => None,
    })
}
#[test]
fn entity_move_keyboard_command_targets_preview_apply_and_escape_contract() {
    let (ctx, mut app) = fixture();
    app.edit_entity(Some("lighthouse"));
    app.open_commands(&ctx, true);
    app.command_palette.query = "移到其他源码".into();
    settle(&ctx, &mut app, egui::vec2(800.0, 600.0));
    press(&ctx, &mut app, Key::Enter, Modifiers::NONE);
    settle(&ctx, &mut app, egui::vec2(800.0, 600.0));
    let form = app
        .entity_source_move_form
        .as_ref()
        .expect("命令打开同一移源窗口");
    assert!(form
        .target_focus_ids
        .contains(&ctx.memory(|memory| memory.focused()).unwrap()));
    press(&ctx, &mut app, Key::ArrowDown, Modifiers::NONE);
    assert_eq!(
        app.entity_source_move_form.as_ref().unwrap().destination,
        app.project.root.join(TARGET)
    );
    press(&ctx, &mut app, Key::Enter, Modifiers::NONE);
    assert!(app.entity_source_move_form.as_ref().unwrap().plan.is_some());
    assert!(app.history.is_empty(), "第一次 Enter 仅预览");
    press(&ctx, &mut app, Key::Enter, Modifiers::NONE);
    assert!(
        app.entity_source_move_form.is_none(),
        "第二次 Enter 明确应用获焦按钮"
    );
    assert_eq!(app.history.len(), 1);
    assert_source(&app, TARGET);
    app.undo(false);
    assert_source(&app, SOURCE);
    let _ = fs::remove_dir_all(app.project.root);
}
#[test]
fn entity_move_tab_exits_target_list_and_cancel_preserves_clean_form_and_bytes() {
    let (ctx, mut app) = fixture();
    app.edit_entity(Some("lighthouse"));
    app.begin_entity_source_move("lighthouse");
    let baseline = app.project.content_baseline();
    settle(&ctx, &mut app, egui::vec2(800.0, 600.0));
    press(&ctx, &mut app, Key::Tab, Modifiers::NONE);
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        app.entity_source_move_form.as_ref().unwrap().preview_focus
    );
    press(&ctx, &mut app, Key::Tab, Modifiers::NONE);
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        app.entity_source_move_form.as_ref().unwrap().cancel_focus
    );
    press(&ctx, &mut app, Key::Enter, Modifiers::NONE);
    assert!(app.entity_source_move_form.is_none());
    assert!(app.entity_editor.is_some());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    let _ = fs::remove_dir_all(app.project.root);
}
#[test]
fn entity_move_escape_closes_only_top_layer_and_ime_escape_does_not_cancel() {
    let (ctx, mut app) = fixture();
    app.edit_entity(Some("lighthouse"));
    app.entity_source_move_form = Some(planned(&mut app));
    settle(&ctx, &mut app, egui::vec2(800.0, 600.0));
    let baseline = app.project.content_baseline();
    app.open_commands(&ctx, true);
    settle(&ctx, &mut app, egui::vec2(800.0, 600.0));
    press(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    assert!(!app.command_palette.open);
    assert!(app.entity_source_move_form.is_some());
    app.ime_composing = true;
    press(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    assert!(app.entity_source_move_form.is_some());
    app.ime_composing = false;
    press(&ctx, &mut app, Key::Escape, Modifiers::NONE);
    assert!(app.entity_source_move_form.is_none());
    assert!(app.entity_editor.is_some());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    let _ = fs::remove_dir_all(app.project.root);
}
#[test]
fn entity_move_long_paths_preview_and_stale_error_keep_actions_visible() {
    for (size, scale) in [
        (egui::vec2(800.0, 600.0), 1.0),
        (egui::vec2(440.0, 480.0), 1.0),
        (egui::vec2(800.0, 600.0), 1.5),
    ] {
        let (ctx, mut app) = fixture();
        ctx.style_mut(|style| {
            for font in style.text_styles.values_mut() {
                font.size *= scale;
            }
        });
        let source = app.project.root.join(SOURCE);
        app.project
            .set_text(
                &source,
                SOURCE_TEXT.replace("海边旧灯", &"海边旧灯与需要完整核对的长原文🙂".repeat(120)),
            )
            .unwrap();
        app.recompile();
        let mut form = planned(&mut app);
        form.display = "北雾灯塔与夜间旧航线的长中文资料名称".repeat(8);
        app.entity_source_move_form = Some(form);
        let output = settle(&ctx, &mut app, size);
        let cancel = visible(&output, "取消移源").expect("长路径与原文不能挤走取消");
        assert!(visible(&output, "预览实体移源").is_some());
        assert!(visible(&output, "应用实体移源").is_some());
        let window = ctx
            .memory(|memory| memory.area_rect(egui::Id::new("entity-source-move")))
            .unwrap();
        assert!(Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(window));
        let entry = app.project.entry.clone();
        app.project
            .set_text(
                &entry,
                format!("{}\n// 当前更新", app.project.document(&entry).unwrap()),
            )
            .unwrap();
        let mut form = app.entity_source_move_form.take().unwrap();
        assert!(!app.apply_entity_source_move(&mut form));
        app.entity_source_move_form = Some(form);
        let output = settle(&ctx, &mut app, size);
        assert_eq!(visible(&output, "取消移源"), Some(cancel));
        assert!(visible(&output, "应用实体移源").is_some());
        assert!(app.history.is_empty());
        press(&ctx, &mut app, Key::Escape, Modifiers::NONE);
        assert!(app.entity_source_move_form.is_none());
        let _ = fs::remove_dir_all(app.project.root);
    }
}

#[test]
fn entity_move_ime_commit_enter_cannot_activate_focused_apply_or_cancel() {
    for action in ["apply", "cancel"] {
        let (ctx, mut app) = fixture();
        app.entity_source_move_form = Some(planned(&mut app));
        settle(&ctx, &mut app, egui::vec2(800.0, 600.0));
        let form = app.entity_source_move_form.as_ref().unwrap();
        let focus = if action == "apply" {
            form.apply_focus
        } else {
            form.cancel_focus
        }
        .unwrap();
        ctx.memory_mut(|memory| memory.request_focus(focus));
        let baseline = app.project.content_baseline();
        frame(
            &ctx,
            &mut app,
            egui::vec2(800.0, 600.0),
            vec![
                Event::Ime(egui::ImeEvent::Commit("中文候选提交".into())),
                key(Key::Enter, Modifiers::NONE),
            ],
        );
        assert!(
            app.entity_source_move_form.is_some(),
            "IME must not trigger {action}"
        );
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(focus),
            "IME保护不得永久丢掉动作焦点"
        );
        assert!(app.history.is_empty());
        let _ = fs::remove_dir_all(app.project.root);
    }
}
