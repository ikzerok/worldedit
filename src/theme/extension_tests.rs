use super::*;
#[test]
fn focus_and_ledger_geometry_are_structural_and_do_not_mutate_layout_preferences() {
    let focus = resolve(
        &AppearancePreferences {
            style: StylePreset::Focus,
            ..Default::default()
        },
        None,
    );
    let ledger = resolve(
        &AppearancePreferences {
            style: StylePreset::Ledger,
            ..Default::default()
        },
        None,
    );
    let wide_focus = document_geometry(&focus, 1200.0, 760.0);
    assert_eq!(wide_focus.outer_width, 760.0);
    assert_eq!(wide_focus.inset_x, 220.0);
    assert_eq!(wide_focus.rail_width, 0.0);
    let wide_ledger = document_geometry(&ledger, 1200.0, 760.0);
    assert_eq!(
        (wide_ledger.rail_width, wide_ledger.rail_gap),
        (112.0, 16.0)
    );
    assert!(wide_ledger.content_width >= 480.0);
    assert_eq!(wide_ledger.inset_x, 0.0);
    for width in [1.0, 100.0, 320.0, 400.0, 639.0] {
        let g = document_geometry(&ledger, width, 760.0);
        assert_eq!(g.rail_width, 0.0);
        assert!(g.content_width > 0.0 && g.content_width <= width);
        assert!(g.inset_x + g.outer_width <= width);
    }
    assert!(document_geometry(&ledger, 640.0, 760.0).rail_width > 0.0);
    assert_eq!(focus.preferences.style, StylePreset::Focus);
    assert_eq!(ledger.preferences.style, StylePreset::Ledger);
}
#[test]
fn ledger_really_moves_its_named_title_above_content_when_narrow() {
    for width in [400.0, 1100.0] {
        let ctx = egui::Context::default();
        let p = AppearancePreferences {
            style: StylePreset::Ledger,
            reduce_motion: true,
            ..Default::default()
        };
        let mut body_rect = egui::Rect::NOTHING;
        let output = ctx.run(
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
                    document_surface_titled(ui, "Record", 760.0, |ui| {
                        let response = ui.label("The author content is unchanged");
                        body_rect = egui::Rect::from_min_size(
                            response.rect.min,
                            egui::vec2(ui.available_width(), response.rect.height()),
                        );
                    });
                });
            },
        );
        let title = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(t) if t.galley.text() == "Record" => {
                    Some(t.galley.rect.translate(t.pos.to_vec2()))
                }
                _ => None,
            })
            .expect("actual side or top title");
        if width > 640.0 {
            assert!(title.right() < body_rect.left());
            assert!(body_rect.width() >= 480.0);
        } else {
            assert!(title.bottom() < body_rect.top());
            assert!(body_rect.width() > width * 0.75);
        }
    }
}
#[test]
fn all_five_styles_keep_original_three_document_geometry_and_two_new_structures() {
    let cases = [
        (StylePreset::Studio, 0.0, 16.0, 0.0),
        (StylePreset::Manuscript, 220.0, 32.0, 0.0),
        (StylePreset::Technical, 0.0, 10.0, 0.0),
        (StylePreset::Focus, 220.0, 20.0, 0.0),
        (StylePreset::Ledger, 0.0, 16.0, 112.0),
    ];
    for (style, inset, padding, rail) in cases {
        let p = AppearancePreferences {
            style,
            ..Default::default()
        };
        let t = resolve(&p, None);
        let g = document_geometry(&t, 1200.0, 760.0);
        assert_eq!(g.inset_x, inset);
        assert_eq!(g.padding.x, padding);
        assert_eq!(g.rail_width, rail);
        assert_eq!(
            g.outer_width,
            if style == StylePreset::Technical {
                1200.0
            } else {
                760.0
            }
        );
        let ctx = egui::Context::default();
        let _theme = configure_appearance(&ctx, &p);
        assert_eq!(focus_width(), 2.0);
    }
}
