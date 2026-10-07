//! 窄宽度绘制与真实展开手势。完整值来自 DTO，不靠 hover 才能查看。
use super::*;

fn draw(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<egui::Event>) -> egui::FullOutput {
    let compared = app.comparison.result.take().unwrap();
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(320.0, 9000.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            let access = navigation::NavigationAccess {
                blocked: None,
                focus: focus::FocusReveal::for_frame(ctx, false),
                ime_focus: focus::ImeFocus::prepare(
                    ctx,
                    egui::Id::new("variable-render-test"),
                    false,
                    false,
                    false,
                ),
            };
            egui::CentralPanel::default().show(ctx, |ui| {
                variables::render(ui, &compared, &mut app.comparison, &access, &mut None);
            });
        },
    );
    app.comparison.result = Some(compared);
    output
}
fn text(output: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    let mut text = Vec::new();
    for shape in &output.shapes {
        labels(&shape.shape, &mut text);
    }
    text
}

#[test]
fn variable_write_long_unicode_value_is_explicitly_truncated_and_expandable_at_narrow_width() {
    let (ctx, mut app) = ready();
    let full = "星🌙e\u{301}".repeat(90);
    app.comparison
        .result
        .as_mut()
        .unwrap()
        .result
        .left
        .variable_writes
        .records[0]
        .after = Value::Str(full.clone());
    for theme in [
        crate::theme::ThemeMode::Dark,
        crate::theme::ThemeMode::Light,
    ] {
        let _theme = crate::theme::configure(&ctx, theme);
        draw(&ctx, &mut app, vec![]);
        let output = draw(&ctx, &mut app, vec![]);
        let text = text(&output);
        assert!(text.iter().any(|(label, _)| label.contains("已截断")));
        assert!(text
            .iter()
            .any(|(label, _)| label == "同值写入：值未改变，仍实际执行了这次写入"));
        for (_, rect) in text.iter().filter(|(label, _)| label == "打开实际写入来源") {
            assert!(
                rect.left() >= 0.0 && rect.right() <= 320.0,
                "narrow source inaccessible: {rect:?}"
            );
        }
    }
    let output = draw(&ctx, &mut app, vec![]);
    let position = text(&output)
        .into_iter()
        .find(|(label, _)| label == "查看写入后完整值")
        .unwrap()
        .1
        .center();
    for pressed in [true, false] {
        draw(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    for _ in 0..24 {
        draw(&ctx, &mut app, vec![]);
    }
    let output = draw(&ctx, &mut app, vec![]);
    assert!(text(&output)
        .iter()
        .any(|(label, _)| label == &format!("{full} · 字符串")));
    let copy = text(&output)
        .into_iter()
        .find(|(label, _)| label == "复制完整值")
        .unwrap()
        .1;
    assert!(copy.left() >= 0.0 && copy.right() <= 320.0 && copy.bottom() < 9000.0);
    let mut copied = false;
    for pressed in [true, false] {
        let output = draw(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(copy.center()),
                egui::Event::PointerButton {
                    pos: copy.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        copied |=
            output.platform_output.commands.iter().any(
                |command| matches!(command, egui::OutputCommand::CopyText(text) if text == &full),
            );
    }
    assert!(copied, "展开后的复制按钮应输出未截断的原始值");
}

#[test]
fn variable_write_missing_legacy_partial_and_omission_states_are_visible_without_hover() {
    let (ctx, mut app) = ready();
    let result = &mut app.comparison.result.as_mut().unwrap().result;
    result.left.variable_writes = Default::default();
    result.right.variable_writes.records[0].source = None;
    result.right.variable_writes.omitted = true;
    result.right.variable_writes.total_writes = 300;
    result.right.complete = false;
    result.right.status = worldline_runtime::RouteStatus::StepBudgetExceeded;
    draw(&ctx, &mut app, vec![]);
    let output = draw(&ctx, &mut app, vec![]);
    let text = text(&output);
    for expected in [
        "旧结果未提供变量写入证据",
        "实际 300 项",
        "遗漏不表示未发生",
        "没有可确认来源",
        "步数预算耗尽",
    ] {
        assert!(
            text.iter().any(|(label, _)| label.contains(expected)),
            "missing {expected}"
        );
    }
    assert!(!text
        .iter()
        .any(|(label, _)| label == "此侧本段没有对 coins 的全局写入"));
}
