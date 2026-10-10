//! Opt-in UI geometry/input primitives for explicit preview readers.
//! No source interpretation, plan validity, delivery confirmation or history lives here.
use crate::theme;

pub fn available(ctx: &egui::Context) -> bool {
    ctx.input(|input| {
        input.focused && !input.events.iter().any(|event| {
            matches!(event, egui::Event::Ime(egui::ImeEvent::Commit(_)))
                || matches!(event, egui::Event::Ime(egui::ImeEvent::Preedit(text)) if !text.is_empty())
        })
    }) && !egui::Popup::is_any_open(ctx)
        && ctx.memory(|memory| {
            memory.top_modal_layer().is_none()
                && !memory.areas().visible_layer_ids().iter()
                    .any(|layer| layer.order == egui::Order::Middle)
        })
}

pub fn reveal(ui: &egui::Ui, response: &egui::Response, keep_visible: bool, frame: bool) {
    if response.enabled() && response.has_focus() && available(ui.ctx()) {
        if (keep_visible || response.gained_focus())
            && !ui.clip_rect().contains_rect(response.rect.expand(2.0))
        {
            response.scroll_to_me(Some(egui::Align::Center));
        }
        if frame {
            outline(ui, response.rect.expand(2.0));
        }
    }
}
pub fn outline(ui: &egui::Ui, rect: egui::Rect) {
    let theme = theme::resolved(ui.ctx());
    ui.painter().rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(theme.focus_width, theme.colors.focus),
        egui::StrokeKind::Inside,
    );
}
/// Map ScrollArea::show_viewport's content-relative viewport into the same Ui's
/// screen coordinates. Unlike a TextEdit content clip this includes its padding.
pub fn viewport(ui: &egui::Ui, relative: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_size(ui.max_rect().min + relative.min.to_vec2(), relative.size())
}
/// A nested reader may be outside its parent viewport. Reveal the complete child
/// domain from the parent Ui, after the child has consumed its own scroll target.
pub fn reveal_domain(ui: &egui::Ui, response: &egui::Response, viewport: egui::Rect) {
    if response.enabled()
        && response.has_focus()
        && available(ui.ctx())
        && !ui.clip_rect().contains_rect(viewport)
    {
        ui.scroll_to_rect(viewport, None);
    }
}
pub struct Reader<'a> {
    pub identity: egui::Id,
    pub label: &'a str,
    pub viewport: egui::Rect,
    pub enabled: bool,
    pub horizontal: bool,
    pub vertical: bool,
    pub step: egui::Vec2,
    pub escape: bool,
}
pub fn reading(ui: &mut egui::Ui, reader: Reader<'_>) -> egui::Response {
    let response = ui
        .push_id(reader.identity, |ui| {
            let response = ui.add_enabled(reader.enabled, egui::Button::new(reader.label).wrap());
            if reader.enabled {
                reveal(ui, &response, false, true);
            }
            response
        })
        .inner;
    if response.has_focus() && reader.enabled && available(ui.ctx()) {
        ui.ctx().memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    horizontal_arrows: reader.horizontal,
                    vertical_arrows: reader.vertical,
                    escape: reader.escape,
                    ..Default::default()
                },
            )
        });
        if ui.input(|input| input.modifiers == egui::Modifiers::NONE) {
            let mut delta = egui::Vec2::ZERO;
            if reader.vertical {
                let up = ui.input_mut(|input| {
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)
                });
                let down = ui.input_mut(|input| {
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
                });
                delta.y = (i32::from(up) - i32::from(down)) as f32 * reader.step.y;
            }
            if reader.horizontal {
                let left = ui.input_mut(|input| {
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
                });
                let right = ui.input_mut(|input| {
                    input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
                });
                delta.x = (i32::from(left) - i32::from(right)) as f32 * reader.step.x;
            }
            if delta != egui::Vec2::ZERO {
                ui.scroll_with_delta(delta);
            }
        }
        let theme = theme::resolved(ui.ctx());
        ui.painter()
            .with_clip_rect(reader.viewport.intersect(ui.clip_rect()))
            .rect_stroke(
                reader.viewport.shrink(theme.focus_width * 0.5),
                2.0,
                egui::Stroke::new(theme.focus_width, theme.colors.focus),
                egui::StrokeKind::Inside,
            );
    }
    response
}
