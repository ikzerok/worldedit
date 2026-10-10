//! Exact text/outer-control visibility domains used by the synthetic IME fixture.
use super::*;

pub(super) fn visible(
    out: &egui::FullOutput,
    response: &egui::WidgetRect,
    label: &str,
    whole: bool,
) -> bool {
    out.shapes.iter().any(|shape| {
        let mut painted = Vec::new();
        text_shapes(&shape.shape, &mut painted);
        painted.iter().any(|text| {
            let rect = text.galley.rect.translate(text.pos.to_vec2());
            text.galley.text() == label
                && response.rect.contains_rect(rect)
                && shape.clip_rect.contains_rect(rect)
                && (!whole || shape.clip_rect.contains_rect(response.rect))
        })
    })
}
