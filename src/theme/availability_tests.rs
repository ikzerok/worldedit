use super::*;
use egui::{Context, FullOutput, Id, Rect, Response, Shape};

fn flattened(output: &FullOutput) -> Vec<&Shape> {
    fn append<'a>(shape: &'a Shape, all: &mut Vec<&'a Shape>) {
        if let Shape::Vec(shapes) = shape {
            for shape in shapes {
                append(shape, all);
            }
        } else {
            all.push(shape);
        }
    }
    let mut all = Vec::new();
    for shape in &output.shapes {
        append(&shape.shape, &mut all);
    }
    all
}
fn over(fg: Color32, bg: Color32) -> Color32 {
    let [r, g, b, a] = fg.to_srgba_unmultiplied();
    let alpha = f32::from(a) / 255.0;
    let mix = |f: u8, b: u8| (f32::from(f) * alpha + f32::from(b) * (1.0 - alpha)).round() as u8;
    Color32::from_rgb(mix(r, bg.r()), mix(g, bg.g()), mix(b, bg.b()))
}
fn contrast(a: Color32, b: Color32) -> f32 {
    fn luminance(c: Color32) -> f32 {
        let f = |v: u8| {
            let n = f32::from(v) / 255.0;
            if n <= 0.04045 {
                n / 12.92
            } else {
                ((n + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
fn text_pair(output: &FullOutput, label: &str) -> (Color32, Color32, f32) {
    let shapes = flattened(output);
    let (index, text) = shapes
        .iter()
        .enumerate()
        .find_map(|(index, shape)| match shape {
            Shape::Text(text) if text.galley.text() == label => Some((index, text)),
            _ => None,
        })
        .expect("real rendered label");
    let mut bg = Color32::TRANSPARENT;
    let mut opaque = false;
    for shape in &shapes[..index] {
        if let Shape::Rect(rect) = shape {
            if rect.rect.contains(text.visual_bounding_rect().center()) {
                opaque |= rect.fill.a() == 255;
                bg = over(rect.fill, bg);
            }
        }
    }
    assert!(opaque, "label needs a real preceding opaque surface");
    let color = text.override_text_color.unwrap_or_else(|| {
        let color = text.galley.job.sections[0].format.color;
        if color == Color32::PLACEHOLDER {
            text.fallback_color
        } else {
            color
        }
    });
    (
        over(color.gamma_multiply(text.opacity_factor), bg),
        bg,
        text.opacity_factor,
    )
}
fn sample(
    ctx: &Context,
    p: &AppearancePreferences,
    enabled: bool,
    surface: Color32,
) -> (FullOutput, Vec<Response>) {
    let mut responses = Vec::new();
    let output = ctx.run(Default::default(), |ctx| {
        let _guard = configure_appearance(ctx, p);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(surface))
            .show(ctx, |ui| {
                let before = ui.style().clone();
                responses.push(add_enabled(ui, enabled, egui::Button::new("Ordinary")));
                assert!(std::sync::Arc::ptr_eq(ui.style(), &before));
                let mut checked = true;
                responses.push(add_enabled(
                    ui,
                    enabled,
                    egui::Checkbox::new(&mut checked, "Checkbox"),
                ));
                responses.push(add_enabled(ui, enabled, primary("Primary")));
                responses.push(add_enabled(
                    ui,
                    enabled,
                    egui::Button::new(RichText::new("Error").color(ERROR())),
                ));
                ui.label("After");
                assert!(ui.is_enabled());
                assert!(std::sync::Arc::ptr_eq(ui.style(), &before));
            });
    });
    (output, responses)
}
#[test]
fn ordinary_disabled_controls_are_distinct_opaque_and_readable_even_in_high_contrast() {
    for palette in PaletteId::ALL {
        for theme in [ThemeMode::Light, ThemeMode::Dark] {
            for high_contrast in [false, true] {
                let p = AppearancePreferences {
                    palette,
                    theme,
                    high_contrast,
                    reduce_motion: true,
                    ..Default::default()
                };
                let c = resolve(&p, None).colors;
                assert_ne!(
                    c.disabled, c.text,
                    "disabled remains an independent readable role"
                );
                for surface in [
                    c.workspace,
                    c.chrome,
                    c.panel,
                    c.document,
                    c.raised,
                    c.hover,
                    c.selection,
                ] {
                    let ctx = Context::default();
                    let (enabled, responses) = sample(&ctx, &p, true, surface);
                    let (disabled, disabled_responses) = sample(&ctx, &p, false, surface);
                    for (a, b) in responses.iter().zip(&disabled_responses) {
                        assert_eq!(
                            (a.id, a.rect),
                            (b.id, b.rect),
                            "color adapter cannot add ID/layout layers"
                        );
                        assert!(a.enabled() && !b.enabled());
                    }
                    for label in ["Ordinary", "Checkbox", "Primary"] {
                        let active = text_pair(&enabled, label);
                        let inactive = text_pair(&disabled, label);
                        assert_eq!(inactive.0, c.disabled, "{palette:?}/{theme:?}/{label}");
                        assert_eq!(inactive.2, 1.0, "no opacity stacking");
                        assert_ne!(
                            active.0, inactive.0,
                            "resting states must differ beyond opacity"
                        );
                        assert!(contrast(inactive.0, inactive.1) >= 4.5);
                    }
                    let (error, bg, alpha) = text_pair(&disabled, "Error");
                    assert_eq!(error, c.danger, "explicit semantic RichText must survive");
                    assert_eq!(alpha, 1.0);
                    assert_eq!(bg, c.hover);
                    assert!(contrast(error, bg) >= 4.5);
                    assert_eq!(
                        text_pair(&disabled, "After").0,
                        c.text,
                        "style restores after widget"
                    );
                    let mut checkbox_boundaries = 0;
                    for shape in flattened(&disabled) {
                        if let Shape::Rect(rect) = shape {
                            if disabled_responses[1].rect.contains_rect(rect.rect)
                                && (10.0..28.0).contains(&rect.rect.width())
                            {
                                checkbox_boundaries += 1;
                                assert!(
                                    contrast(rect.stroke.color, over(rect.fill, surface)) >= 3.0
                                );
                            }
                        }
                    }
                    assert!(
                        checkbox_boundaries > 0,
                        "actual checkbox outline must exist"
                    );
                }
            }
        }
    }
}

fn group_sample(
    ctx: &Context,
    adapter: bool,
    high_contrast: bool,
) -> (FullOutput, Vec<(Id, Rect, bool)>) {
    let mut items = Vec::new();
    let output = ctx.run(Default::default(), |ctx| {
        let _guard = configure_appearance(
            ctx,
            &AppearancePreferences {
                high_contrast,
                ..Default::default()
            },
        );
        egui::CentralPanel::default().show(ctx, |ui| {
            let original = ui.style().clone();
            let contents = |ui: &mut egui::Ui| {
                assert!(!ui.is_enabled());
                let child = |ui: &mut egui::Ui| {
                    let response = if adapter {
                        add_enabled(ui, true, egui::Button::new("Nested"))
                    } else {
                        egui::Ui::add_enabled(ui, true, egui::Button::new("Nested"))
                    };
                    items.push((response.id, response.rect, response.enabled()));
                    ui.label(RichText::new("Semantic").color(ERROR()));
                };
                let nested = if adapter {
                    add_enabled_ui(ui, true, child)
                } else {
                    egui::Ui::add_enabled_ui(ui, true, child)
                };
                items.push((
                    nested.response.id,
                    nested.response.rect,
                    nested.response.enabled(),
                ));
            };
            let group = if adapter {
                add_enabled_ui(ui, false, contents)
            } else {
                egui::Ui::add_enabled_ui(ui, false, contents)
            };
            items.push((
                group.response.id,
                group.response.rect,
                group.response.enabled(),
            ));
            assert!(ui.is_enabled());
            assert!(std::sync::Arc::ptr_eq(ui.style(), &original));
            ui.label("Restored");
        });
    });
    (output, items)
}
#[test]
fn nested_disabled_groups_keep_native_ids_geometry_and_never_reenable_or_double_fade() {
    for high_contrast in [false, true] {
        let baseline = group_sample(&Context::default(), false, high_contrast);
        let ctx = Context::default();
        let adapted = group_sample(&ctx, true, high_contrast);
        assert_eq!(
            baseline.1, adapted.1,
            "reuse exactly the existing scope hierarchy"
        );
        assert!(adapted.1[..2].iter().all(|item| !item.2));
        let c = resolved(&ctx).colors;
        assert_eq!(text_pair(&adapted.0, "Nested").0, c.disabled);
        assert_eq!(text_pair(&adapted.0, "Nested").2, 1.0);
        assert_eq!(text_pair(&adapted.0, "Semantic").0, c.danger);
        assert_eq!(text_pair(&adapted.0, "Semantic").2, 1.0);
        assert_eq!(text_pair(&adapted.0, "Restored").0, c.text);
    }
}
#[test]
fn local_preview_colors_do_not_mix_with_old_context_or_leak_to_neighbor_controls() {
    let ctx = Context::default();
    let old = AppearancePreferences::default();
    let local = resolve(
        &AppearancePreferences {
            palette: PaletteId::Terminal,
            theme: ThemeMode::Light,
            ..Default::default()
        },
        None,
    );
    let output = ctx.run(Default::default(), |ctx| {
        let _guard = configure_appearance(ctx, &old);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(local.colors.document))
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(local.colors.text);
                let before = ui.style().clone();
                let response =
                    add_enabled_with_colors(ui, false, egui::Button::new("Preview"), local.colors);
                assert!(!response.enabled());
                assert!(std::sync::Arc::ptr_eq(ui.style(), &before));
                ui.label("Local neighbor");
            });
    });
    assert_eq!(resolved(&ctx).preferences, old);
    let preview = text_pair(&output, "Preview");
    assert_eq!(preview.0, local.colors.disabled);
    assert_eq!(preview.1, local.colors.hover);
    assert_eq!(preview.2, 1.0);
    assert!(contrast(preview.0, preview.1) >= 4.5);
    assert_eq!(text_pair(&output, "Local neighbor").0, local.colors.text);
}
#[test]
fn permanent_disable_matches_existing_ui_lifetime_and_preserves_selected_identity() {
    let ctx = Context::default();
    let mut enabled = true;
    let output = ctx.run(Default::default(), |ctx| {
        let _guard = configure_appearance(
            ctx,
            &AppearancePreferences {
                high_contrast: true,
                ..Default::default()
            },
        );
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.scope(|ui| {
                disable(ui);
                let response = add_enabled(ui, true, egui::Button::new("Selected").selected(true));
                enabled = response.enabled();
            });
            assert!(ui.is_enabled());
            ui.label("Outside");
        });
    });
    assert!(!enabled);
    let c = resolved(&ctx).colors;
    let (fg, bg, alpha) = text_pair(&output, "Selected");
    assert_eq!(fg, c.disabled);
    assert_eq!(
        bg, c.selection,
        "selection identity remains distinct from availability"
    );
    assert_eq!(alpha, 1.0);
    assert!(contrast(fg, bg) >= 4.5);
    assert_eq!(text_pair(&output, "Outside").0, c.text);
}
