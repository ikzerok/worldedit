//! 原生复现问题的egui帧回归：Tab可见性与上层输入不被来源建议遮挡。
use super::tests::app;
use super::*;
use crate::app::Tab;
use std::path::Path;

fn fixture() -> (egui::Context, WorldeditApp, String) {
    let (ctx, mut app) = app();
    app.project
        .set_text(
            &app.active_file.clone(),
            "world glass_tide as \"玻璃潮\"\n".into(),
        )
        .unwrap();
    for path in ["north/notes_00.wl", "south/notes_00.wl"] {
        let file = app.project.add_file(Path::new(path)).unwrap();
        app.project
            .set_text(
                &file,
                "character repeated_bell as \"需核对来源的长中文名字 🧭\"\n".into(),
            )
            .unwrap();
    }
    app.recompile();
    let report = app.project.problems_report(&Default::default()).unwrap();
    let id = report
        .entries
        .iter()
        .find(|entry| entry.code == "A104" && entry.related_count > 0)
        .unwrap()
        .id
        .clone();
    app.problems.observation = Some(report.source_observation.clone());
    app.problems.install(report, app.version);
    app.problems.schedule.observe(app.version, 0.);
    app.problems.schedule.cancel();
    app.problems.select(id.clone());
    (ctx, app, id)
}
fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| {
            eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest());
        },
    )
}
fn press(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    key: egui::Key,
    modifiers: egui::Modifiers,
) -> Vec<egui::WidgetInfo> {
    let mut focused = Vec::new();
    for pressed in [true, false] {
        let output = frame(
            ctx,
            app,
            size,
            vec![egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            }],
        );
        for event in output.platform_output.events {
            if let egui::output::OutputEvent::FocusGained(info) = event {
                focused.push(info);
            }
        }
    }
    focused
}
fn visible_focus(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2, label: &str) {
    for _ in 0..16 {
        frame(ctx, app, size, vec![]);
    }
    let output = frame(ctx, app, size, vec![]);
    let id = ctx.memory(|memory| memory.focused()).unwrap();
    let response = ctx.read_response(id).unwrap();
    assert!(
        output.shapes.iter().any(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) =>
                text.galley.text() == label
                    && response.rect.contains(text.pos)
                    && shape.clip_rect.contains_rect(response.rect),
            _ => false,
        }),
        "键盘按钮{label}未完整滚入: {:?}",
        response.rect
    );
}

#[test]
fn tab_reveals_primary_and_related_actions_then_enter_and_back_work_in_short_large_type_pane() {
    for size in [egui::vec2(1188., 848.), egui::vec2(1040., 660.)] {
        for font in [16., 28.] {
            let (ctx, mut app, id) = fixture();
            let original = app.active_file.clone();
            let target = app
                .project
                .problem_location(app.problems.report.as_ref().unwrap(), &id, Some(0))
                .unwrap()
                .path
                .unwrap();
            app.personal.settings.body_size = font;
            app.open_problems(&ctx);
            app.problems.narrow_detail = size.x < 1180.;
            ctx.data_mut(|data| {
                data.insert_persisted(
                    egui::Id::new("project-problems"),
                    egui::containers::panel::PanelState {
                        rect: egui::Rect::from_min_size(
                            egui::pos2(0., size.y - 220.),
                            egui::vec2(size.x, 220.),
                        ),
                    },
                )
            });
            for _ in 0..4 {
                frame(&ctx, &mut app, size, vec![]);
            }
            let mut locations = 0;
            let mut copied = 0;
            let mut seen_labels = Vec::new();
            for _ in 0..80 {
                let focus = press(&ctx, &mut app, size, egui::Key::Tab, egui::Modifiers::NONE);
                for item in focus {
                    if let Some(label) = item.label {
                        seen_labels.push(label.clone());
                        if matches!(label.as_str(), "复制问题" | "复制位置" | "定位来源")
                        {
                            visible_focus(&ctx, &mut app, size, &label);
                            if label == "复制位置" {
                                copied += 1;
                            }
                            if label == "定位来源" {
                                locations += 1;
                            }
                        }
                    }
                }
                if locations == 2 {
                    break;
                }
            }
            assert_eq!(
                locations, 2,
                "主/related定位均应可Tab到达，size={size:?} font={font}; seen={seen_labels:?}; selected={:?} stale={} error={:?} narrow={}", app.problems.selected, app.problems.stale(app.version), app.problems.error, app.problems.narrow_detail
            );
            assert!(copied >= 2);
            press(
                &ctx,
                &mut app,
                size,
                egui::Key::Enter,
                egui::Modifiers::NONE,
            );
            assert_eq!(app.active_file, app.project.root.join(target));
            assert_eq!(app.tab, Tab::Edit);
            press(
                &ctx,
                &mut app,
                size,
                egui::Key::ArrowLeft,
                egui::Modifiers::ALT,
            );
            assert_eq!(app.active_file, original);
            assert_eq!(app.tab, Tab::Manuscript);
            assert_eq!(app.problems.selected, Some(id));
        }
    }
}

fn rendered_contains(shape: &egui::epaint::Shape, needle: &str) -> bool {
    match shape {
        egui::epaint::Shape::Text(text) => text.galley.text().contains(needle),
        egui::epaint::Shape::Vec(shapes) => {
            shapes.iter().any(|shape| rendered_contains(shape, needle))
        }
        _ => false,
    }
}
#[test]
fn programmatic_problem_selection_never_paints_source_suggestion_above_commands_or_search() {
    for search in [false, true] {
        let (ctx, mut app, _) = fixture();
        let size = egui::vec2(1188., 848.);
        app.locate_problem(&ctx, None);
        for _ in 0..4 {
            frame(&ctx, &mut app, size, vec![]);
        }
        let id = egui::Id::new(("source", &app.active_file));
        let before = egui::TextEdit::load_state(&ctx, id)
            .unwrap()
            .cursor
            .char_range()
            .unwrap();
        assert!(!before.is_empty());
        let (key, modifiers) = if search {
            (egui::Key::F, egui::Modifiers::COMMAND)
        } else {
            (
                egui::Key::P,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            )
        };
        press(&ctx, &mut app, size, key, modifiers);
        for _ in 0..5 {
            let output = frame(&ctx, &mut app, size, vec![]);
            assert!(
                !output
                    .shapes
                    .iter()
                    .any(|shape| rendered_contains(&shape.shape, "从选中文本建档")),
                "问题定位选区不能覆盖上层输入"
            );
        }
        assert!(if search {
            app.search_open
        } else {
            app.command_palette.open
        });
        assert_eq!(
            egui::TextEdit::load_state(&ctx, id)
                .unwrap()
                .cursor
                .char_range()
                .unwrap(),
            before,
            "隐藏建议不丢原选区"
        );
    }
}
