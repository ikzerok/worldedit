use super::*;
use crate::theme::{self, AppearancePreferences, PaletteId, StylePreset, ThemeMode};
use egui::Color32;

fn painted_colors(shape: &egui::Shape, label: &str, colors: &mut Vec<Color32>) {
    match shape {
        egui::Shape::Text(text) if text.galley.text() == label => colors.extend(
            text.galley
                .job
                .sections
                .iter()
                .map(|section| section.format.color),
        ),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                painted_colors(shape, label, colors);
            }
        }
        _ => {}
    }
}

#[test]
fn localization_statuses_and_parts_use_current_context_semantics_in_every_palette_and_style() {
    let (project, mut state) = super::workbench_tests::fixture(1);
    state.string_ids = "line0".into();
    let plan = project
        .preview_localization_export(&state.selection())
        .unwrap();
    for palette in PaletteId::ALL {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            for style in [
                StylePreset::Studio,
                StylePreset::Manuscript,
                StylePreset::Technical,
                StylePreset::Focus,
                StylePreset::Ledger,
            ] {
                let preferences = AppearancePreferences {
                    palette,
                    theme: mode,
                    style,
                    reduce_motion: true,
                    ..Default::default()
                };
                let ctx = egui::Context::default();
                let output = ctx.run(Default::default(), |ctx| {
                    let _theme = theme::configure_appearance(ctx, &preferences);
                    egui::CentralPanel::default().show(ctx, |ui| {
                        show_parts(
                            ui,
                            &[
                                LocalizationPart::Placeholder {
                                    token: "{count}".into(),
                                },
                                LocalizationPart::Link {
                                    token: "entity:a".into(),
                                    label: "linked record".into(),
                                },
                            ],
                        );
                        show_diagnostics(
                            ui,
                            &[LocalizationDiagnostic {
                                code: "INVALID".into(),
                                message: "blocked import".into(),
                                id: None,
                                source: None,
                            }],
                            &mut None,
                            navigation::Container::Export(&plan),
                        );
                        for status in [Ok("applied locally".into()), Err("retained input".into())] {
                            show_status(
                                ui,
                                &LocalizationUiState {
                                    status: Some(status),
                                    ..Default::default()
                                },
                            );
                        }
                    });
                });
                let c = theme::resolved(&ctx).colors;
                for (label, expected) in [
                    ("{count}", c.info),
                    ("linked record", c.info),
                    ("INVALID · blocked import", c.danger),
                    ("applied locally", c.success),
                    ("retained input", c.danger),
                ] {
                    let mut actual = Vec::new();
                    for shape in &output.shapes {
                        painted_colors(&shape.shape, label, &mut actual);
                    }
                    assert!(
                        !actual.is_empty(),
                        "{palette:?}/{mode:?}/{style:?}: {label}"
                    );
                    assert!(
                        actual.iter().all(|color| *color == expected),
                        "{label}: {actual:?} != {expected:?}"
                    );
                }
            }
        }
    }
}
