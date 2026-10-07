//! 实际绘制的已选状态细边；文字与必要边界分别核对，不冒认完整无障碍。
use super::tests::app;
use crate::theme;

fn contrast(a: egui::Color32, b: egui::Color32) -> f32 {
    fn light(color: egui::Color32) -> f32 {
        let linear = |value: u8| {
            let value = f32::from(value) / 255.;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
    }
    let (a, b) = (light(a), light(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[test]
fn selected_row_keeps_a_three_pixel_accent_boundary_without_keyboard_focus() {
    for mode in [theme::ThemeMode::Dark, theme::ThemeMode::Light] {
        for selected in [false, true] {
            let (ctx, app) = app();
            let _theme = theme::configure(&ctx, mode);
            let problem = app.problems.report.as_ref().unwrap().entries[0].clone();
            let mut row = egui::Rect::NOTHING;
            let output = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    row = super::view::problem_row(ui, &problem, selected, 16., 48.).rect;
                });
            });
            let boundary = output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Rect(shape)
                    if shape.fill == theme::ACCENT()
                        && (shape.rect.width() - 3.).abs() < 0.01
                        && row.contains_rect(shape.rect) =>
                {
                    Some(shape)
                }
                _ => None,
            });
            assert_eq!(boundary.is_some(), selected);
            if let Some(boundary) = boundary {
                assert!(contrast(boundary.fill, theme::PANEL()) >= 3.0);
                assert!(contrast(boundary.fill, ctx.style().visuals.selection.bg_fill) >= 3.0);
            }
        }
    }
}
