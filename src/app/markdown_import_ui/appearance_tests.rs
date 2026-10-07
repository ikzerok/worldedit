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
fn markdown_import_blockers_use_current_context_error_color_in_every_palette_and_style() {
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    app.saved_location = false;
    let baseline = app.project.content_baseline();
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
                let mut wizard = Wizard {
                    target_mode: TargetMode::CurrentProject,
                    error: Some("source invalid; input retained".into()),
                    ..Default::default()
                };
                let output = ctx.run(Default::default(), |ctx| {
                    let _theme = theme::configure_appearance(ctx, &preferences);
                    egui::CentralPanel::default()
                        .show(ctx, |ui| wizard.contents(ui, &mut app, ctx));
                });
                let expected = theme::resolved(&ctx).colors.danger;
                for label in [
                    "当前工程尚未保存到工作区；请先另存工程，或选择导入到新工程。",
                    "source invalid; input retained",
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
                assert_eq!(app.project.content_baseline(), baseline);
            }
        }
    }
}
