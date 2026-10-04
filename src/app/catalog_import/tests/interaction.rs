use super::*;

pub(super) fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: Vec2,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| {
            crate::theme::configure(ctx, app.personal.settings.theme);
            app.author_shortcuts(ctx);
            app.top_bar(ctx);
            app.status_bar(ctx);
            app.sidebar(ctx);
            app.catalog_import_tab(ctx);
        },
    )
}
pub(super) fn point(shape: &egui::Shape, label: &str) -> Option<Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text == label => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| point(shape, label)),
        _ => None,
    }
}
pub(super) fn text(shape: &egui::Shape, result: &mut String) {
    match shape {
        egui::Shape::Text(item) => {
            result.push_str(&item.galley.job.text);
            result.push('\n');
        }
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                text(shape, result);
            }
        }
        _ => {}
    }
}
pub(super) fn rendered(output: &egui::FullOutput) -> String {
    let mut result = String::new();
    for shape in &output.shapes {
        text(&shape.shape, &mut result);
    }
    result
}
pub(super) fn click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let size = egui::vec2(1188.0, 848.0);
    for _ in 0..3 {
        frame(ctx, app, size, vec![]);
    }
    let output = frame(ctx, app, size, vec![]);
    let p = output
        .shapes
        .iter()
        .find_map(|shape| point(&shape.shape, label).filter(|p| shape.clip_rect.contains(*p)))
        .unwrap_or_else(|| panic!("找不到可见按钮 {label}: {}", rendered(&output)));
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            size,
            vec![
                Event::PointerMoved(p),
                Event::PointerButton {
                    pos: p,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}
pub(super) fn key(ctx: &egui::Context, app: &mut WorldeditApp, key: Key) -> egui::FullOutput {
    frame(
        ctx,
        app,
        egui::vec2(1188.0, 848.0),
        vec![Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    )
}
