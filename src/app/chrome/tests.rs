use super::*;

#[derive(Clone)]
struct PaintedText {
    text: String,
    rect: egui::Rect,
    elided: bool,
    rows: usize,
}
fn painted(shape: &egui::Shape, texts: &mut Vec<PaintedText>) {
    match shape {
        egui::Shape::Text(shape) => texts.push(PaintedText {
            text: shape.galley.text().to_owned(),
            rect: shape.galley.rect.translate(shape.pos.to_vec2()),
            elided: shape.galley.elided,
            rows: shape.galley.rows.len(),
        }),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                painted(shape, texts);
            }
        }
        _ => {}
    }
}
fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    width: f32,
    tick: usize,
    events: Vec<egui::Event>,
) -> Vec<PaintedText> {
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 848.0),
            )),
            time: Some(tick as f64 * 0.1),
            events,
            ..Default::default()
        },
        |ctx| app.status_bar(ctx),
    );
    let mut texts = Vec::new();
    for shape in output.shapes {
        painted(&shape.shape, &mut texts);
    }
    texts
}

#[test]
fn long_status_receipt_is_single_line_with_full_hover_and_clickable_right_controls() {
    for width in [660.0, 1188.0] {
        for mode in [theme::ThemeMode::Dark, theme::ThemeMode::Light] {
            let ctx = egui::Context::default();
            let creation = eframe::CreationContext::_new_kittest(ctx.clone());
            let mut app = WorldeditApp::new(&creation, None);
            let _theme = theme::configure(&ctx, mode);
            ctx.style_mut(|style| {
                style.animation_time = 0.0;
                style.interaction.tooltip_delay = 0.0;
                style.interaction.show_tooltips_only_when_still = false;
            });
            let message = format!(
                "阅读包已写入 /workspace/{}authoring-reader-site.zip",
                "很长的作品来源目录/".repeat(16)
            );
            app.message = Some(message.clone());
            app.history.push(app.project.clone());
            app.redo.push(app.project.clone());
            let mut texts = Vec::new();
            for tick in 0..3 {
                texts = frame(&ctx, &mut app, width, tick, vec![]);
            }
            let receipt = texts.iter().find(|text| text.text == message).unwrap();
            let undo = texts.iter().find(|text| text.text == "撤销").unwrap().rect;
            let redo = texts.iter().find(|text| text.text == "重做").unwrap().rect;
            let version = texts
                .iter()
                .find(|text| text.text.starts_with("WORLDLINE "))
                .unwrap()
                .rect;
            assert!(receipt.elided, "长回执行内必须截断");
            assert_eq!(receipt.rows, 1);
            assert!(
                receipt.rect.right() < undo.left(),
                "回执不能占到撤销区域：{}",
                width
            );
            assert!(undo.right() < redo.left() && redo.right() < version.left());
            assert!(version.right() <= width - 8.0);
            let point = receipt.rect.center();
            let mut hover = Vec::new();
            for tick in 3..7 {
                hover = frame(
                    &ctx,
                    &mut app,
                    width,
                    tick,
                    vec![egui::Event::PointerMoved(point)],
                );
            }
            assert_eq!(
                hover
                    .iter()
                    .filter(|text| text.text == message && !text.elided)
                    .count(),
                1,
                "完整回执只能出现一份tooltip，不能叠加截断Label自动提示与手动提示"
            );
            for (index, (rect, forward)) in [(undo, false), (redo, true)].into_iter().enumerate() {
                let history = app.history.len();
                for pressed in [true, false] {
                    frame(
                        &ctx,
                        &mut app,
                        width,
                        7 + index * 2 + usize::from(!pressed),
                        vec![
                            egui::Event::PointerMoved(rect.center()),
                            egui::Event::PointerButton {
                                pos: rect.center(),
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                    );
                }
                assert_eq!(
                    app.history.len(),
                    if forward { history + 1 } else { history - 1 },
                    "右侧操作必须可点击"
                );
            }
        }
    }
}
