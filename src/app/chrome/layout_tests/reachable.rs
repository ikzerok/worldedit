use super::*;

fn click(ctx: &egui::Context, app: &mut WorldeditApp, point: Pos2, width: f32, height: f32) {
    for pressed in [true, false] {
        render(
            ctx,
            app,
            width,
            height,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

/// 低频入口必须常驻或一次展开后真实可点，不能简单删除原可见性断言。
pub(super) fn global_actions(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    width: f32,
    height: f32,
    output: &egui::FullOutput,
) {
    let save = text_position(output, "保存全部").expect("保存全部入口");
    assert!(
        ctx.screen_rect().contains(save),
        "{width}/{height}: 保存全部离屏"
    );
    let expanded;
    let shown = if let Some(more) = text_position(output, "更多") {
        assert!(
            ctx.screen_rect().contains(more),
            "{width}/{height}: 更多入口离屏"
        );
        click(ctx, app, more, width, height);
        render(ctx, app, width, height, vec![]);
        expanded = render(ctx, app, width, height, vec![]);
        &expanded
    } else {
        output
    };
    for label in [
        "▶ 试玩",
        "搜索",
        "导出与发布",
        "工程",
        "视图",
        "编辑",
        "外观",
    ] {
        let point = text_position(shown, label)
            .unwrap_or_else(|| panic!("{width}/{height}: unreachable {label}"));
        assert!(
            ctx.screen_rect().contains(point),
            "{width}/{height}: offscreen {label}: {point:?}"
        );
    }
    if text_position(output, "更多").is_some() {
        render(
            ctx,
            app,
            width,
            height,
            vec![Event::Key {
                key: egui::Key::Escape,
                physical_key: Some(egui::Key::Escape),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        render(ctx, app, width, height, vec![]);
    }
}

#[test]
fn folded_global_actions_can_activate_appearance_and_play() {
    for size in [(400.0, 300.0), (1280.0, 760.0)] {
        let (ctx, mut app) = app();
        app.personal.settings.style = StylePreset::Focus;
        for _ in 0..3 {
            render(&ctx, &mut app, size.0, size.1, vec![]);
        }
        for (label, is_appearance) in [("外观", true), ("▶ 试玩", false)] {
            let output = render(&ctx, &mut app, size.0, size.1, vec![]);
            let more = text_position(&output, "更多").unwrap();
            assert!(ctx.screen_rect().contains(more));
            click(&ctx, &mut app, more, size.0, size.1);
            render(&ctx, &mut app, size.0, size.1, vec![]);
            let output = render(&ctx, &mut app, size.0, size.1, vec![]);
            let point = text_position(&output, label).unwrap();
            assert!(ctx.screen_rect().contains(point));
            click(&ctx, &mut app, point, size.0, size.1);
            if is_appearance {
                assert!(app.personal.preferences_open);
                app.personal.preferences_open = false;
            } else {
                assert_eq!(app.tab, Tab::Play);
            }
        }
    }
}
