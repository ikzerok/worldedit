use super::*;
use egui::{Event, Pos2, Rect};

fn luminance(color: Color32) -> f32 {
    let channel = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}
fn contrast(a: Color32, b: Color32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
fn draw(
    ctx: &egui::Context,
    checked: &mut bool,
    enabled: bool,
    events: Vec<Event>,
) -> (egui::FullOutput, egui::Response) {
    let mut response = None;
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(520.0, 240.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                card().show(ui, |ui| {
                    response =
                        Some(ui.add_enabled(enabled, egui::Checkbox::new(checked, "解决批注")));
                });
            });
        },
    );
    (output, response.unwrap())
}
fn checkbox_rect(shape: &egui::Shape, within: Rect) -> Option<&egui::epaint::RectShape> {
    match shape {
        egui::Shape::Rect(rect)
            if within.expand(2.0).contains_rect(rect.rect)
                && (10.0..28.0).contains(&rect.rect.width())
                && (rect.rect.width() - rect.rect.height()).abs() < 0.5 =>
        {
            Some(rect)
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| checkbox_rect(shape, within)),
        _ => None,
    }
}
fn outline(output: &egui::FullOutput, response: &egui::Response) -> Stroke {
    output
        .shapes
        .iter()
        .find_map(|s| checkbox_rect(&s.shape, response.rect))
        .expect("未选checkbox必须实际绘制边界")
        .stroke
}

#[test]
fn dark_and_light_control_states_keep_outlines_and_distinct_focus_widths() {
    for mode in [ThemeMode::Dark, ThemeMode::Light] {
        let ctx = egui::Context::default();
        configure(&ctx, mode);
        let visuals = ctx.style().visuals.clone();
        assert!(contrast(visuals.widgets.inactive.bg_stroke.color, CARD()) >= 3.0);
        assert_eq!(visuals.widgets.inactive.bg_stroke.width, 1.0);
        assert_eq!(visuals.widgets.hovered.bg_stroke.width, 1.5);
        assert_eq!(visuals.widgets.active.bg_stroke.width, 2.0);
        assert!(visuals.widgets.noninteractive.bg_stroke.width > 0.0);
        let mut checked = false;
        let (output, response) = draw(&ctx, &mut checked, true, vec![]);
        let inactive = outline(&output, &response);
        assert_eq!(inactive.width, 1.0);
        let (output, response) = draw(
            &ctx,
            &mut checked,
            true,
            vec![Event::PointerMoved(response.rect.center())],
        );
        assert_eq!(outline(&output, &response).width, 1.5);
        ctx.memory_mut(|m| m.request_focus(response.id));
        let (output, response) = draw(&ctx, &mut checked, true, vec![Event::PointerGone]);
        assert!(response.has_focus());
        assert_eq!(outline(&output, &response).width, 2.0);
        let (output, response) = draw(&ctx, &mut checked, false, vec![]);
        let disabled = outline(&output, &response);
        assert!(disabled.width >= 1.0 && disabled.color.a() > 0);
        assert!(disabled.color.a() < inactive.color.a(), "禁用态仍应弱化");
        assert!(!response.enabled());
        assert!(!checked);
    }
}

#[test]
fn disabled_checkbox_ignores_clicks_and_checked_state_has_noncolor_mark() {
    for mode in [ThemeMode::Dark, ThemeMode::Light] {
        let ctx = egui::Context::default();
        configure(&ctx, mode);
        let mut checked = false;
        let (_, response) = draw(&ctx, &mut checked, false, vec![]);
        let point = response.rect.center();
        for pressed in [true, false] {
            draw(
                &ctx,
                &mut checked,
                false,
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
        assert!(!checked);
        checked = true;
        let (output, response) = draw(&ctx, &mut checked, true, vec![Event::PointerGone]);
        fn has_mark(shape: &egui::Shape, within: Rect) -> bool {
            match shape {
                egui::Shape::Path(path) => {
                    path.points.len() == 3 && path.points.iter().all(|p| within.contains(*p))
                }
                egui::Shape::Vec(shapes) => shapes.iter().any(|s| has_mark(s, within)),
                _ => false,
            }
        }
        assert!(
            output
                .shapes
                .iter()
                .any(|s| has_mark(&s.shape, response.rect)),
            "已勾选不能只靠颜色"
        );
    }
}
