use super::{
    pos2, scroll_to_visible, vec2, Event, PointerButton, RawInput, Rect, TargetRef, WorldeditApp,
};

pub(super) fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
    mut window: u8,
) -> egui::FullOutput {
    if window == 17 && app.markdown_import_wizard.is_some() {
        window = 25;
    }
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(
                pos2(0.0, 0.0),
                if window == 37 {
                    vec2(760.0, 720.0)
                } else if window == 34 || window == 36 || window == 38 {
                    vec2(1188.0, 848.0)
                } else if window == 35 {
                    vec2(800.0, 600.0)
                } else if window == 33 {
                    vec2(760.0, 620.0)
                } else if window == 32 {
                    vec2(800.0, 600.0)
                } else if window == 31 {
                    vec2(1280.0, 800.0)
                } else if window == 29 {
                    vec2(1280.0, 720.0)
                } else if window == 30 {
                    vec2(800.0, 600.0)
                } else if window == 16 || window == 21 || window == 24 {
                    vec2(700.0, 640.0)
                } else if window == 25 || window == 26 {
                    vec2(1280.0, 1000.0)
                } else if window == 9 {
                    vec2(1040.0, 660.0)
                } else if window == 19 {
                    vec2(2600.0, 2400.0)
                } else {
                    vec2(1700.0, 1400.0)
                },
            )),
            events,
            ..Default::default()
        },
        |ctx| match window {
            0 => app.entity_editor_window(ctx),
            1 => app.relation_editor_window(ctx),
            2 => app.relation_type_editor_window(ctx),
            4 | 37 => app.network_tab(ctx),
            5 => app.target_rename_window(ctx),
            6 => app.preset_editor_window(ctx),
            7 => app.review_tab(ctx),
            16 => app.review_tab(ctx),
            8 | 9 => app.reading_window(ctx),
            11 => app.source_tab(ctx),
            13 => app.manuscript_tab(ctx),
            17 | 19 => app.template_manager_tab(ctx),
            18 => app.sidebar(ctx),
            10 => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    app.reading_content(ui, TargetRef::new("entity", "a"));
                });
            }
            12 => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let target = app.reading_target.clone().unwrap();
                    app.reading_content(ui, target);
                });
            }
            14 => app.catalog_tab(ctx),
            38 => {
                app.top_bar(ctx);
                app.status_bar(ctx);
                app.sidebar(ctx);
                app.review_tab(ctx);
            }
            36 => {
                app.top_bar(ctx);
                app.status_bar(ctx);
                app.sidebar(ctx);
                app.checkpoint_history_tab(ctx);
            }
            34 | 35 => {
                app.top_bar(ctx);
                app.status_bar(ctx);
                app.sidebar(ctx);
                app.catalog_tab(ctx);
            }
            29 | 30 => {
                app.top_bar(ctx);
                app.status_bar(ctx);
                app.sidebar(ctx);
                app.play_tab(ctx);
            }
            20 | 21 => app.play_tab(ctx),
            22 => app.canvas_tab(ctx),
            23 => app.checkpoint_history_tab(ctx),
            31 | 32 => {
                app.top_bar(ctx);
                app.status_bar(ctx);
                app.sidebar(ctx);
                app.manuscript_tab(ctx);
            }
            24 => app.checkpoint_history_tab(ctx),
            33 => {
                app.top_bar(ctx);
                app.markdown_import_window(ctx);
            }
            25 => {
                app.top_bar(ctx);
                app.markdown_import_window(ctx);
            }
            26 => {
                app.top_bar(ctx);
                app.reader_publish_window(ctx);
            }
            _ => app.content_deletion_window(ctx),
        },
    )
}

pub(super) fn text_position(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text == label => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| text_position(shape, label)),
        _ => None,
    }
}
pub(super) fn source_text_position(
    shape: &egui::Shape,
    source: &str,
    needle: &str,
) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text == source => {
            let byte = source.find(needle)?;
            let cursor = source[..byte].chars().count() + needle.chars().count() / 2;
            Some(
                text.pos
                    + text
                        .galley
                        .pos_from_cursor(egui::text::CCursor::new(cursor))
                        .center()
                        .to_vec2(),
            )
        }
        egui::Shape::Vec(shapes) => shapes
            .iter()
            .find_map(|shape| source_text_position(shape, source, needle)),
        _ => None,
    }
}
pub(super) fn click(ctx: &egui::Context, app: &mut WorldeditApp, window: u8, label: &str) {
    for _ in 0..3 {
        let _ = frame(ctx, app, Vec::new(), window);
    }
    let output = frame(ctx, app, Vec::new(), window);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position(&shape.shape, label))
        .unwrap_or_else(|| {
            let mut rendered = String::new();
            for shape in &output.shapes {
                collect_text(&shape.shape, &mut rendered);
            }
            panic!("按钮未显示：{label}；当前文字：{rendered}");
        });
    for pressed in [true, false] {
        let _ = frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            window,
        );
    }
}
pub(super) fn click_without_settling(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    label: &str,
) {
    let output = frame(ctx, app, Vec::new(), window);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position(&shape.shape, label))
        .unwrap_or_else(|| panic!("按钮未显示：{label}"));
    for pressed in [true, false] {
        let _ = frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            window,
        );
    }
}

pub(super) fn replace_text_area(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    placeholder: &str,
    replacement: &str,
) {
    for _ in 0..3 {
        let _ = frame(ctx, app, Vec::new(), window);
    }
    let output = frame(ctx, app, Vec::new(), window);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| clipped_text_position(&shape.shape, placeholder, shape.clip_rect))
        .unwrap_or_else(|| panic!("未显示可编辑文本：{placeholder}"));
    for pressed in [true, false] {
        let _ = frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            window,
        );
    }
    let key = Event::Key {
        key: egui::Key::A,
        physical_key: Some(egui::Key::A),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    };
    let release = Event::Key {
        key: egui::Key::A,
        physical_key: Some(egui::Key::A),
        pressed: false,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    };
    let _ = frame(
        ctx,
        app,
        vec![key, release, Event::Text(replacement.into())],
        window,
    );
}

pub(super) fn drag_numeric_value(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    value_text: &str,
    horizontal_delta: f32,
) {
    let output = frame(ctx, app, Vec::new(), window);
    let start = output
        .shapes
        .iter()
        .find_map(|shape| text_position_contains(&shape.shape, value_text))
        .unwrap_or_else(|| panic!("未显示数值控件：{value_text}"));
    let _ = frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(start),
            Event::PointerButton {
                pos: start,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        window,
    );
    let end = start + vec2(horizontal_delta, 0.0);
    let _ = frame(ctx, app, vec![Event::PointerMoved(end)], window);
    let _ = frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(end),
            Event::PointerButton {
                pos: end,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ],
        window,
    );
}

pub(super) fn click_containing(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    window: u8,
    fragment: &str,
) {
    let _ = scroll_to_visible(ctx, app, window, fragment, -90.0);
    for _ in 0..3 {
        let _ = frame(ctx, app, Vec::new(), window);
    }
    let output = frame(ctx, app, Vec::new(), window);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position_contains(&shape.shape, fragment))
        .unwrap_or_else(|| {
            let mut rendered = String::new();
            for shape in &output.shapes {
                collect_text(&shape.shape, &mut rendered);
            }
            panic!("按钮文字未显示：{fragment}；当前文字：{rendered}")
        });
    for pressed in [true, false] {
        let _ = frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            window,
        );
    }
}

pub(super) fn open_selected_entity_form(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    source: &str,
    selected: &str,
) -> std::path::PathBuf {
    let entry = app.project.entry.clone();
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;
    let byte_start = source.find(selected).unwrap();
    let start = source[..byte_start].chars().count();
    let end = start + selected.chars().count();
    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(end),
        )));
    egui::TextEdit::store_state(ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let _ = frame(ctx, app, Vec::new(), 11);
    click(ctx, app, 11, "从选中文本建档");
    entry
}

fn clipped_text_position(
    shape: &egui::Shape,
    fragment: &str,
    clip: egui::Rect,
) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text.contains(fragment) => {
            let visible = text
                .galley
                .rect
                .translate(text.pos.to_vec2())
                .intersect(clip);
            visible.is_positive().then(|| visible.center())
        }
        egui::Shape::Vec(shapes) => shapes
            .iter()
            .find_map(|shape| clipped_text_position(shape, fragment, clip)),
        _ => None,
    }
}

pub(super) fn text_position_contains(shape: &egui::Shape, fragment: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text.contains(fragment) => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes
            .iter()
            .find_map(|shape| text_position_contains(shape, fragment)),
        _ => None,
    }
}

pub(super) fn collect_text(shape: &egui::Shape, text: &mut String) {
    match shape {
        egui::Shape::Text(shape) => text.push_str(&shape.galley.job.text),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                collect_text(shape, text);
            }
        }
        _ => {}
    }
}
