use super::*;
const ACCENTS: [AccentChoice; 6] = [
    AccentChoice::Palette,
    AccentChoice::Pine,
    AccentChoice::Plum,
    AccentChoice::Copper,
    AccentChoice::Indigo,
    AccentChoice::IceCyan,
];
fn luminance(c: Color32) -> f64 {
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
fn contrast(a: Color32, b: Color32) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
fn assert_ratio(fg: Color32, bg: Color32, min: f64, context: &str) {
    assert!(
        contrast(fg, bg) >= min,
        "{context}: {fg:?} on {bg:?} = {} < {min}",
        contrast(fg, bg)
    );
}
#[test]
fn every_palette_accent_and_contrast_overlay_passes_actual_surface_pairs() {
    for palette in PaletteId::ALL {
        for theme in [ThemeMode::Light, ThemeMode::Dark] {
            for accent in ACCENTS {
                for high_contrast in [false, true] {
                    let t = resolve(
                        &AppearancePreferences {
                            palette,
                            theme,
                            accent,
                            high_contrast,
                            ..Default::default()
                        },
                        None,
                    );
                    let c = t.colors;
                    let surfaces = [
                        c.workspace,
                        c.chrome,
                        c.panel,
                        c.document,
                        c.raised,
                        c.selection,
                        c.hover,
                        c.invalid_background,
                    ];
                    let context = format!("{palette:?}/{theme:?}/{accent:?}/{high_contrast}");
                    for bg in surfaces {
                        for fg in [
                            c.text,
                            c.secondary,
                            c.disabled,
                            c.danger,
                            c.warning,
                            c.success,
                            c.info,
                            c.accent,
                        ] {
                            assert_ratio(fg, bg, 4.5, &context);
                        }
                        for boundary in [c.control_border, c.focus, c.accent] {
                            assert_ratio(boundary, bg, 3.0, &context);
                        }
                    }
                    assert_ratio(c.on_accent, c.accent, 4.5, &context);
                    assert_ratio(c.on_accent, c.accent_hover, 4.5, &context);
                    assert_ratio(c.selection_text, c.selection, 4.5, &context);
                    for bg in [c.document, c.panel, c.selection] {
                        for fg in [
                            t.syntax.keyword,
                            t.syntax.string,
                            t.syntax.reference,
                            t.syntax.number,
                            t.syntax.tag,
                            t.syntax.comment,
                            t.syntax.plain,
                        ] {
                            assert_ratio(fg, bg, 4.5, &context);
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn all_palette_style_density_scale_requests_are_independent_finite_and_complete() {
    let mut count = 0;
    for palette in PaletteId::ALL {
        for theme in [ThemeMode::Light, ThemeMode::Dark] {
            for style in [
                StylePreset::Studio,
                StylePreset::Manuscript,
                StylePreset::Technical,
                StylePreset::Focus,
                StylePreset::Ledger,
            ] {
                for (density, height) in [
                    (Density::Compact, 26.0),
                    (Density::Standard, 30.0),
                    (Density::Spacious, 36.0),
                ] {
                    for ui_scale in [0.8, 1.0, 1.25, 2.0] {
                        let p = AppearancePreferences {
                            palette,
                            theme,
                            style,
                            density,
                            ui_scale,
                            ..Default::default()
                        };
                        let t = resolve(&p, None);
                        assert_eq!(t.preferences, p);
                        assert_eq!(t.metrics.row_height, height);
                        assert_eq!(t.metrics.control_height, height);
                        assert!(t.metrics.panel_margin.is_finite());
                        assert_eq!(t.type_roles.ui.size, BODY_SIZE);
                        assert_ne!(t.colors.workspace, t.colors.document);
                        assert_eq!(t, resolve(&p, None));
                        count += 1;
                    }
                }
            }
        }
    }
    assert_eq!(count, PaletteId::ALL.len() * 2 * 5 * 3 * 4);
}
#[test]
fn context_cache_scope_interleaving_and_early_return_never_leak_theme() {
    let a = egui::Context::default();
    let b = egui::Context::default();
    let pa = AppearancePreferences {
        theme: ThemeMode::Light,
        ..Default::default()
    };
    let pb = AppearancePreferences {
        theme: ThemeMode::Dark,
        palette: PaletteId::Plum,
        style: StylePreset::Technical,
        ..Default::default()
    };
    let default = BG();
    {
        let _a = configure_appearance(&a, &pa);
        let color_a = BG();
        let revision = resolved(&a).revision;
        {
            let _b = configure_appearance(&b, &pb);
            assert_eq!(BG(), resolved(&b).colors.workspace);
            assert_ne!(BG(), color_a);
            assert_eq!(resolved(&a).revision, revision);
            assert_eq!(a.style().visuals.panel_fill, resolved(&a).colors.panel);
        }
        assert_eq!(BG(), color_a);
        fn early_return(ctx: &egui::Context, p: &AppearancePreferences) {
            let _theme = configure_appearance(ctx, p);
            if !is_light() {
                return;
            }
            panic!("fixture must be dark");
        }
        early_return(&b, &pb);
        assert_eq!(BG(), color_a);
        // Reapplying an unchanged theme must not replace unrelated Context style edits.
        a.style_mut(|s| s.spacing.tooltip_width = 387.0);
        let _same = configure_appearance(&a, &pa);
        assert_eq!(a.style().spacing.tooltip_width, 387.0);
    }
    assert_eq!(BG(), default);
    assert_eq!(resolved(&b).preferences, pb);
}
#[test]
fn every_appearance_axis_invalidates_the_resolved_revision_and_system_falls_back_light() {
    let p = AppearancePreferences::default();
    let base = resolve(&p, None);
    assert!(base.light);
    let mut variants = Vec::new();
    for mutate in [
        (|p: &mut AppearancePreferences| p.palette = PaletteId::Copper)
            as fn(&mut AppearancePreferences),
        |p| p.style = StylePreset::Manuscript,
        |p| p.density = Density::Compact,
        |p| p.accent = AccentChoice::Plum,
        |p| p.body_family = BodyFamily::Mono,
        |p| p.body_size = 21.0,
        |p| p.source_size = 22.0,
        |p| p.ui_scale = 1.25,
        |p| p.line_spacing = 1.7,
        |p| p.reading_width = 950.0,
        |p| p.high_contrast = true,
        |p| p.reduce_motion = true,
    ] {
        let mut changed = p;
        mutate(&mut changed);
        variants.push(resolve(&changed, None));
    }
    variants.push(resolve(&p, Some(egui::Theme::Dark)));
    for theme in variants {
        assert_ne!(theme.revision, base.revision);
    }
}
#[test]
fn each_style_has_real_document_geometry_and_readable_focus_at_small_widths() {
    for width in [320.0, 800.0] {
        let mut left_edges = Vec::new();
        for style in [
            StylePreset::Studio,
            StylePreset::Manuscript,
            StylePreset::Technical,
        ] {
            let ctx = egui::Context::default();
            let p = AppearancePreferences {
                style,
                ..Default::default()
            };
            let mut rect = egui::Rect::NOTHING;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 600.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    let _theme = configure_appearance(ctx, &p);
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let response = document_surface(ui, 480.0, |ui| {
                            rect = ui.max_rect();
                            ui.label("A continuous page");
                        });
                        assert!(response.response.rect.width() <= width);
                    });
                },
            );
            assert!(rect.width().is_finite() && rect.width() > 0.0);
            left_edges.push(rect.left());
        }
        assert!(
            left_edges[1] > left_edges[0],
            "Manuscript needs a real page margin"
        );
        assert!(
            left_edges[0] > left_edges[2],
            "Technical has edge-aligned content"
        );
    }
}
