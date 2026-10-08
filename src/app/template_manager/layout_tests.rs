//! 真实设计器状态的离屏绘制；不宣称物理平台或原生输入法验收。
use super::*;
use crate::theme::{AppearancePreferences, PaletteId, PaletteModeSupport, StylePreset, ThemeMode};

fn app() -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let snapshot = app.snapshot.as_ref().unwrap();
    let projection = app
        .project
        .template_draft(ProjectTemplateDraftSource::New, &snapshot.result)
        .unwrap();
    let projection = app
        .project
        .edit_template_draft(
            &projection.draft,
            &ProjectTemplateDraftEdit::AddField {
                parent_id: None,
                index: 0,
                field_type: ProjectTemplateFieldType::Group,
            },
            &snapshot.result,
        )
        .unwrap();
    app.template_manager.install(projection, true).unwrap();
    app.template_manager.mode = Mode::Design;
    (ctx, app)
}
fn render(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    width: f32,
    height: f32,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, height),
            )),
            ..Default::default()
        },
        |ctx| {
            let _appearance = theme::configure_appearance(ctx, app.personal.appearance());
            app.template_manager_tab(ctx);
        },
    )
}
fn visible(output: &egui::FullOutput, label: &str) -> bool {
    fn point(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| point(shape, label)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .any(|shape| point(&shape.shape, label).is_some_and(|p| shape.clip_rect.contains(p)))
}
#[test]
fn template_designer_renders_all_eighty_five_real_palette_style_combinations() {
    let mut combinations = 0;
    for palette in PaletteId::ALL {
        let modes = match palette.mode_support() {
            PaletteModeSupport::Both => vec![ThemeMode::Light, ThemeMode::Dark],
            PaletteModeSupport::LightOnly => vec![ThemeMode::Light],
            PaletteModeSupport::DarkOnly => vec![ThemeMode::Dark],
        };
        for mode in modes {
            for style in [
                StylePreset::Studio,
                StylePreset::Manuscript,
                StylePreset::Technical,
                StylePreset::Focus,
                StylePreset::Ledger,
            ] {
                let (ctx, mut app) = app();
                app.personal.settings.appearance = AppearancePreferences {
                    palette,
                    theme: mode,
                    style,
                    reduce_motion: true,
                    ..Default::default()
                };
                let before = app.project.content_baseline();
                for _ in 0..3 {
                    render(&ctx, &mut app, 1280.0, 800.0);
                }
                let output = render(&ctx, &mut app, 1280.0, 800.0);
                for label in [
                    "模板管理",
                    "新建空白模板",
                    "预览导入 / 替换",
                    "可视字段",
                    "试填预览",
                    "高级 JSON",
                ] {
                    assert!(
                        visible(&output, label),
                        "{palette:?}/{mode:?}/{style:?}: {label}"
                    );
                }
                assert_eq!(app.project.content_baseline(), before);
                combinations += 1;
            }
        }
    }
    assert_eq!(combinations, 85);
}
#[test]
fn template_designer_narrow_and_two_hundred_percent_keep_primary_actions_and_modes() {
    for (width, height, scale) in [(800.0, 600.0, 1.0), (400.0, 300.0, 2.0)] {
        let (ctx, mut app) = app();
        app.personal.settings.ui_scale = scale;
        app.personal.settings.appearance.reduce_motion = true;
        for _ in 0..3 {
            render(&ctx, &mut app, width, height);
        }
        let output = render(&ctx, &mut app, width, height);
        for label in [
            "新建空白模板",
            "预览导入 / 替换",
            "可视字段",
            "试填预览",
            "高级 JSON",
        ] {
            assert!(visible(&output, label), "{width}×{height}/{scale}: {label}");
        }
    }
}
