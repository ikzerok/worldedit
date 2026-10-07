//! 全部真实工作区走同一外壳与主题；这些是egui绘制测试，不冒称物理平台验收。
use crate::app::{Tab, WorldeditApp};
use crate::theme::{self, AppearancePreferences, PaletteId, StylePreset, ThemeMode};
use egui::{Color32, Event, Pos2, Rect};

fn app() -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let app = WorldeditApp::new(&creation, None);
    (ctx, app)
}

fn render(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    width: f32,
    height: f32,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(width, height))),
            events,
            ..Default::default()
        },
        |ctx| {
            let _theme = theme::configure_appearance(ctx, app.personal.appearance());
            ctx.style_mut(|style| style.animation_time = 0.0);
            app.author_shortcuts(ctx);
            app.top_bar(ctx);
            app.status_bar(ctx);
            if app.personal.settings.navigation
                && !app.personal.settings.focus
                && !app.compact_workspace_navigation(ctx)
            {
                app.sidebar(ctx);
            }
            app.docked_reading(ctx);
            match app.tab {
                Tab::Manuscript => app.manuscript_tab(ctx),
                Tab::Overview => app.overview_tab(ctx),
                Tab::Edit => app.source_tab(ctx),
                Tab::World => app.world_tab(ctx),
                Tab::Characters => app.characters_tab(ctx),
                Tab::Catalog => app.catalog_tab(ctx),
                Tab::CatalogImport => app.catalog_import_tab(ctx),
                Tab::Wiki => app.wiki_tab(ctx),
                Tab::Map => app.map_tab(ctx),
                Tab::Network => app.network_tab(ctx),
                Tab::Timeline | Tab::Graph => {
                    app.event_inspector(ctx);
                    app.canvas_tab(ctx);
                }
                Tab::Review => app.review_tab(ctx),
                Tab::Templates => app.template_manager_tab(ctx),
                Tab::Localization => {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        crate::app::localization_ui::show(
                            ui,
                            &mut app.project,
                            &mut app.localization_ui,
                        );
                    });
                }
                Tab::CheckpointHistory => app.checkpoint_history_tab(ctx),
                Tab::Play => app.play_tab(ctx),
            }
            app.reading_window(ctx);
            app.navigation_drawer_window(ctx);
            app.capture_edit_focus(ctx);
        },
    )
}

fn text_position(output: &egui::FullOutput, label: &str) -> Option<Pos2> {
    fn find(shape: &egui::Shape, label: &str) -> Option<Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, label)),
            _ => None,
        }
    }
    output.shapes.iter().find_map(|shape| {
        let point = find(&shape.shape, label)?;
        shape.clip_rect.contains(point).then_some(point)
    })
}

fn colors(shape: &egui::Shape, all: &mut Vec<Color32>) {
    match shape {
        egui::Shape::Rect(rect) => {
            all.push(rect.fill);
            all.push(rect.stroke.color);
        }
        egui::Shape::Text(text) => all.extend(
            text.galley
                .job
                .sections
                .iter()
                .map(|section| section.format.color),
        ),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                colors(shape, all);
            }
        }
        _ => {}
    }
}

#[test]
fn every_workspace_renders_in_all_new_palette_style_pairs_without_old_skin_colors() {
    let tabs = [
        Tab::Manuscript,
        Tab::Overview,
        Tab::Edit,
        Tab::World,
        Tab::Characters,
        Tab::Catalog,
        Tab::CatalogImport,
        Tab::Wiki,
        Tab::Map,
        Tab::Network,
        Tab::Timeline,
        Tab::Graph,
        Tab::Review,
        Tab::Templates,
        Tab::Localization,
        Tab::CheckpointHistory,
        Tab::Play,
    ];
    let legacy = [
        Color32::from_rgb(22, 23, 27),
        Color32::from_rgb(29, 30, 35),
        Color32::from_rgb(38, 40, 46),
        Color32::from_rgb(248, 249, 252),
        Color32::from_rgb(242, 244, 248),
        Color32::from_rgb(143, 183, 248),
    ];
    for palette in PaletteId::ALL {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
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
                let baseline = app.project.content_baseline();
                for tab in tabs {
                    app.tab = tab;
                    for _ in 0..2 {
                        render(&ctx, &mut app, 1280.0, 800.0, Vec::new());
                    }
                    let output = render(&ctx, &mut app, 1280.0, 800.0, Vec::new());
                    for label in ["保存全部", "工程", "视图", "编辑", "外观"] {
                        assert!(
                            text_position(&output, label).is_some(),
                            "{palette:?}/{mode:?}/{style:?}/{tab:?}: {label}"
                        );
                    }
                    let mut actual = Vec::new();
                    for shape in &output.shapes {
                        colors(&shape.shape, &mut actual);
                    }
                    for old in legacy {
                        assert!(
                            !actual.contains(&old),
                            "legacy color {old:?} in {palette:?}/{mode:?}/{style:?}/{tab:?}"
                        );
                    }
                    assert_eq!(
                        app.project.content_baseline(),
                        baseline,
                        "rendering never mutates a project"
                    );
                }
            }
        }
    }
}

#[test]
fn narrow_shell_keeps_all_global_decision_points_reachable_and_does_not_change_layout_preferences()
{
    for (width, height, scale) in [(800.0, 600.0, 1.0), (400.0, 300.0, 2.0)] {
        let (ctx, mut app) = app();
        app.personal.settings.ui_scale = scale;
        let navigation = app.personal.settings.navigation;
        let width_preference = app.personal.settings.navigation_width;
        for _ in 0..3 {
            render(&ctx, &mut app, width, height, Vec::new());
        }
        let output = render(&ctx, &mut app, width, height, Vec::new());
        for label in ["保存全部", "导出与发布", "工程", "视图", "编辑", "外观"] {
            let point = text_position(&output, label)
                .unwrap_or_else(|| panic!("{width}/{scale}: missing {label}"));
            assert!(point.x >= 0.0 && point.x < width && point.y >= 0.0 && point.y < height);
        }
        if width == 400.0 {
            assert!(text_position(&output, "导航").is_some());
        }
        assert_eq!(app.personal.settings.navigation, navigation);
        assert_eq!(app.personal.settings.navigation_width, width_preference);
    }
}

#[test]
fn compact_reference_drawer_cancel_keeps_pins_docking_width_and_project() {
    let (ctx, mut app) = app();
    let id = app
        .reading_panels
        .pin(worldline_core::TargetRef::new("event", "start"))
        .unwrap();
    app.selected_reading_panel = Some(id);
    let baseline = app.project.content_baseline();
    let width = app.personal.settings.reference_width;
    for _ in 0..3 {
        render(&ctx, &mut app, 640.0, 600.0, Vec::new());
    }
    let output = render(&ctx, &mut app, 640.0, 600.0, Vec::new());
    let point = text_position(&output, "参考 1").unwrap();
    for pressed in [true, false] {
        render(
            &ctx,
            &mut app,
            640.0,
            600.0,
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
    render(&ctx, &mut app, 640.0, 600.0, Vec::new());
    assert!(app.compact_reference_open(&ctx));
    render(
        &ctx,
        &mut app,
        640.0,
        600.0,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: Some(egui::Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert!(!app.compact_reference_open(&ctx));
    assert_eq!(app.reading_panels.ids(), vec![id]);
    assert!(app.personal.settings.dock_references);
    assert_eq!(app.personal.settings.reference_width, width);
    assert_eq!(app.project.content_baseline(), baseline);
}

mod focus;
