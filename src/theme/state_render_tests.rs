//! 从 egui 实际输出的填充/轮廓/文字取色；透明禁用态先合成到真实背景。
use super::*;
use egui::{Event, Pos2, Rect, Response};
fn contrast(a: Color32, b: Color32) -> f64 {
    fn l(c: Color32) -> f64 {
        let f = |v: u8| {
            let v = f64::from(v) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }
    let (a, b) = (l(a), l(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
fn over(fg: Color32, bg: Color32) -> Color32 {
    let [r, g, b, a] = fg.to_srgba_unmultiplied();
    let alpha = f64::from(a) / 255.0;
    let mix = |front: u8, back: u8| {
        (f64::from(front) * alpha + f64::from(back) * (1.0 - alpha)).round() as u8
    };
    Color32::from_rgb(mix(r, bg.r()), mix(g, bg.g()), mix(b, bg.b()))
}
fn shapes(output: &egui::FullOutput) -> Vec<&egui::Shape> {
    fn gather<'a>(shape: &'a egui::Shape, result: &mut Vec<&'a egui::Shape>) {
        match shape {
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    gather(shape, result);
                }
            }
            shape => result.push(shape),
        }
    }
    let mut result = Vec::new();
    for shape in &output.shapes {
        gather(&shape.shape, &mut result);
    }
    result
}
fn draw(
    ctx: &egui::Context,
    p: &AppearancePreferences,
    events: Vec<Event>,
    enabled: bool,
) -> (egui::FullOutput, Vec<Response>) {
    let mut responses = Vec::new();
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(650.0, 420.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            let _theme = configure_appearance(ctx, p);
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut checked = false;
                responses.push(super::add_enabled(
                    ui,
                    enabled,
                    egui::Checkbox::new(&mut checked, "Check"),
                ));
                responses.push(ui.add(primary("Primary")));
                responses.push(super::add_enabled(ui, enabled, primary("Disabled primary")));
                responses.push(ui.selectable_label(true, "Selected"));
                let mut text = "Invalid value".to_owned();
                ui.scope(|ui| {
                    ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(1.0_f32, ERROR());
                    responses.push(ui.add(egui::TextEdit::singleline(&mut text)));
                });
                ui.colored_label(ERROR(), "Error description");
                ui.colored_label(WARNING(), "Warning description");
                let mut value = 0.5;
                responses.push(super::slider(
                    ui,
                    egui::Slider::new(&mut value, 0.0..=1.0).text("Slider"),
                ));
            });
        },
    );
    (output, responses)
}
fn control_rect<'a>(
    output: &'a egui::FullOutput,
    response: &Response,
    checkbox: bool,
) -> &'a egui::epaint::RectShape {
    shapes(output)
        .into_iter()
        .find_map(|shape| match shape {
            egui::Shape::Rect(rect)
                if response.rect.expand(1.0).contains_rect(rect.rect)
                    && if checkbox {
                        (10.0..28.0).contains(&rect.rect.width())
                            && (rect.rect.width() - rect.rect.height()).abs() < 0.5
                    } else {
                        rect.rect.width() > response.rect.width() * 0.8
                    } =>
            {
                Some(rect)
            }
            _ => None,
        })
        .expect("actual control rectangle")
}
#[test]
fn all_palettes_render_hover_focus_selected_invalid_and_disabled_boundaries() {
    for palette in PaletteId::ALL {
        for theme in [ThemeMode::Light, ThemeMode::Dark] {
            for accent in [
                AccentChoice::Palette,
                AccentChoice::Pine,
                AccentChoice::Plum,
                AccentChoice::Copper,
                AccentChoice::Indigo,
                AccentChoice::IceCyan,
            ] {
                for high_contrast in [false, true] {
                    let p = AppearancePreferences {
                        palette,
                        theme,
                        accent,
                        high_contrast,
                        reduce_motion: true,
                        ..Default::default()
                    };
                    let ctx = egui::Context::default();
                    let c = resolve(&p, None).colors;
                    let (output, responses) = draw(&ctx, &p, vec![], true);
                    for (index, checkbox) in [(0, true), (4, false)] {
                        let rect = control_rect(&output, &responses[index], checkbox);
                        let fill = over(rect.fill, c.panel);
                        assert!(
                            contrast(over(rect.stroke.color, fill), fill) >= 3.0,
                            "{palette:?}/{theme:?} control {index}"
                        );
                    }
                    let rail = shapes(&output)
                        .into_iter()
                        .find_map(|shape| match shape {
                            egui::Shape::Rect(rect)
                                if responses[5].rect.contains_rect(rect.rect)
                                    && rect.rect.width() > 50.0
                                    && rect.rect.height() <= 4.5 =>
                            {
                                Some(rect)
                            }
                            _ => None,
                        })
                        .expect("real slider rail");
                    assert_eq!(rail.fill, c.control_border);
                    assert!(contrast(rail.fill, c.panel) >= 3.0);
                    let selected = control_rect(&output, &responses[3], false);
                    assert_eq!(selected.fill, c.selection);
                    let (hover, responses) = draw(
                        &ctx,
                        &p,
                        vec![Event::PointerMoved(responses[1].rect.center())],
                        true,
                    );
                    assert_eq!(
                        control_rect(&hover, &responses[1], false).fill,
                        c.accent_hover
                    );
                    ctx.memory_mut(|m| m.request_focus(responses[0].id));
                    let (focus, responses) = draw(&ctx, &p, vec![Event::PointerGone], true);
                    let rect = control_rect(&focus, &responses[0], true);
                    assert_eq!(rect.stroke.color, c.focus);
                    assert_eq!(rect.stroke.width, if high_contrast { 3.0 } else { 2.0 });
                    assert!(contrast(rect.stroke.color, rect.fill) >= 3.0);
                    let (disabled, responses) = draw(&ctx, &p, vec![], false);
                    assert!(!responses[0].enabled() && !responses[2].enabled());
                    for (index, checkbox) in [(0, true), (2, false)] {
                        let rect = control_rect(&disabled, &responses[index], checkbox);
                        let fill = over(rect.fill, c.panel);
                        assert!(
                            contrast(over(rect.stroke.color, fill), fill) >= 3.0,
                            "disabled {palette:?}/{theme:?}/{index}"
                        );
                    }
                    // TextShape opacity_factor is the real disabled paint multiplier.
                    let rendered = shapes(&disabled);
                    for (text_index, shape) in rendered.iter().enumerate() {
                        if let egui::Shape::Text(text) = shape {
                            let label = text.galley.text();
                            if [
                                "Check",
                                "Disabled primary",
                                "Selected",
                                "Error description",
                                "Warning description",
                            ]
                            .contains(&label)
                            {
                                let point = text.visual_bounding_rect().center();
                                let mut bg = Color32::TRANSPARENT;
                                let mut has_opaque_surface = false;
                                for previous in &rendered[..text_index] {
                                    if let egui::Shape::Rect(rect) = previous {
                                        if rect.rect.contains(point) {
                                            has_opaque_surface |= rect.fill.a() == 255;
                                            bg = over(rect.fill, bg);
                                        }
                                    }
                                }
                                assert!(
                                    has_opaque_surface,
                                    "text requires a real preceding background"
                                );
                                for section in &text.galley.job.sections {
                                    let fg = text
                                        .override_text_color
                                        .unwrap_or_else(|| {
                                            if section.format.color == Color32::PLACEHOLDER {
                                                text.fallback_color
                                            } else {
                                                section.format.color
                                            }
                                        })
                                        .gamma_multiply(text.opacity_factor);
                                    assert!(
                                        contrast(over(fg, bg), bg) >= 4.5,
                                        "actual text {label}/{palette:?}/{theme:?}/{accent:?}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
