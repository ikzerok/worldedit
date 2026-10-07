use super::*;

fn scroll_menu(ctx: &egui::Context, app: &mut WorldeditApp, position: egui::Pos2, delta: f32) {
    app_frame(
        ctx,
        app,
        vec2(800.0, 600.0),
        vec![
            Event::PointerMoved(position),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, delta),
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..60 {
        app_frame(ctx, app, vec2(800.0, 600.0), vec![]);
    }
}

#[test]
fn scaled_small_discard_menu_preserves_cancel_and_requires_real_confirmation() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    app.personal.settings.style = StylePreset::Focus;
    app.personal.settings.reduce_motion = true;
    app.personal.settings.ui_scale = 2.0;
    app.personal.settings.body_size = 28.0;
    app.manuscript.reader_open = false;
    let draft = "event start\n  暂不应用的正文。\n  -> END\n";
    app.manuscript
        .writing_buffers
        .get_mut(&app.active_file)
        .unwrap()
        .replace_source(draft.into());
    let baseline = app.project.content_baseline();
    let size = vec2(800.0, 600.0);
    settle_app(&ctx, &mut app, size);
    let mut menu_point = None;
    for confirm in [false, true] {
        app_click(&ctx, &mut app, size, "正文工具");
        // The menu may remember its previous scroll position. Scroll within its
        // real viewport, then use fully visible buttons for both decisions.
        let position = *menu_point
            .get_or_insert_with(|| full_label(&settle_app(&ctx, &mut app, size), "写作").center());
        scroll_menu(&ctx, &mut app, position, 1200.0);
        full_label(&settle_app(&ctx, &mut app, size), "丢弃此文件草稿");
        app_click(&ctx, &mut app, size, "丢弃此文件草稿");
        assert_eq!(app.manuscript.writing_buffers()[0].source(), draft);
        scroll_menu(&ctx, &mut app, position, -1200.0);
        app_click(
            &ctx,
            &mut app,
            size,
            if confirm {
                "确认丢弃正文草稿"
            } else {
                "取消丢弃"
            },
        );
        settle_app(&ctx, &mut app, size);
        assert_eq!(app.project.content_baseline(), baseline);
        if !confirm {
            assert_eq!(app.manuscript.writing_buffers()[0].source(), draft);
            app_key(&ctx, &mut app, size, egui::Key::Escape);
        } else {
            assert!(!app.manuscript.writing_buffers()[0].is_changed());
        }
    }
}
