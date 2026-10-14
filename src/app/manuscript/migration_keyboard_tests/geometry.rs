use super::*;

pub(super) fn texts(output: &egui::FullOutput) -> Vec<(String, Rect, Rect)> {
    fn collect(shape: &egui::Shape, clip: Rect, out: &mut Vec<(String, Rect, Rect)>) {
        match shape {
            egui::Shape::Text(t) => out.push((
                t.galley.text().into(),
                t.galley.rect.translate(t.pos.to_vec2()),
                clip,
            )),
            egui::Shape::Vec(shapes) => {
                for s in shapes {
                    collect(s, clip, out)
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &output.shapes {
        collect(&s.shape, s.clip_rect, &mut out)
    }
    out
}
pub(super) fn visible_label(output: &egui::FullOutput, label: &str) -> Option<Rect> {
    texts(output)
        .into_iter()
        .find_map(|(text, rect, clip)| (text == label && clip.contains_rect(rect)).then_some(rect))
}
pub(super) fn labels(output: &egui::FullOutput) -> String {
    texts(output)
        .into_iter()
        .map(|t| t.0)
        .collect::<Vec<_>>()
        .join("\n")
}
pub(super) fn galley_rows(output: &egui::FullOutput) -> Vec<(String, usize, String, bool)> {
    fn collect(shape: &egui::Shape, clip: Rect, out: &mut Vec<(String, usize, String, bool)>) {
        match shape {
            egui::Shape::Text(t) => {
                for (index, row) in t.galley.rows.iter().enumerate() {
                    let rect = row.rect().translate(t.pos.to_vec2());
                    if !row.text().trim().is_empty() {
                        out.push((
                            t.galley.text().to_owned(),
                            index,
                            row.text(),
                            clip.contains_rect(rect),
                        ));
                    }
                }
            }
            egui::Shape::Vec(shapes) => {
                for s in shapes {
                    collect(s, clip, out)
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in &output.shapes {
        collect(&s.shape, s.clip_rect, &mut out)
    }
    out
}
pub(super) fn has_viewport_focus_paint(
    ctx: &egui::Context,
    out: &egui::FullOutput,
    target: Rect,
) -> bool {
    let theme = crate::theme::resolved(ctx);
    fn check(
        shape: &egui::Shape,
        clip: Rect,
        target: Rect,
        color: egui::Color32,
        width: f32,
    ) -> bool {
        match shape {
            egui::Shape::Rect(r) => {
                clip.contains_rect(r.rect)
                    && r.stroke.color == color
                    && r.stroke.width >= width
                    && (r.rect.left() - target.left()).abs() <= width + 1.0
                    && (r.rect.right() - target.right()).abs() <= width + 1.0
                    && (r.rect.top() - target.top()).abs() <= width + 1.0
                    && (r.rect.bottom() - target.bottom()).abs() <= width + 1.0
            }
            egui::Shape::Vec(shapes) => shapes.iter().any(|s| check(s, clip, target, color, width)),
            _ => false,
        }
    }
    out.shapes.iter().any(|s| {
        check(
            &s.shape,
            s.clip_rect,
            target,
            theme.colors.focus,
            theme.focus_width,
        )
    })
}
/// Buttons/checkboxes/headers use click-only Sense; ScrollArea and scrollbars do not.
pub(super) fn control_owns_label(
    ctx: &egui::Context,
    response: &egui::Response,
    label: Rect,
) -> bool {
    let style = ctx.style();
    let spacing = &style.spacing;
    response.sense.senses_click()
        && !response.sense.senses_drag()
        && response.rect.expand(1.0).contains_rect(label)
        && response.rect.height()
            <= (label.height() + 2.0 * spacing.button_padding.y).max(spacing.interact_size.y) + 2.0
}
pub(super) fn has_control_focus_paint(
    ctx: &egui::Context,
    out: &egui::FullOutput,
    target: Rect,
) -> bool {
    let theme = crate::theme::resolved(ctx);
    fn check(
        shape: &egui::Shape,
        clip: Rect,
        target: Rect,
        color: egui::Color32,
        width: f32,
    ) -> bool {
        match shape {
            egui::Shape::Rect(r) => {
                r.stroke.color == color
                    && r.stroke.width >= width
                    && clip.contains_rect(r.rect)
                    && r.rect.contains(target.center())
                    && (r.rect.width() - target.width()).abs() <= 8.0
                    && (r.rect.height() - target.height()).abs() <= 8.0
            }
            egui::Shape::Vec(shapes) => shapes.iter().any(|s| check(s, clip, target, color, width)),
            _ => false,
        }
    }
    out.shapes.iter().any(|s| {
        check(
            &s.shape,
            s.clip_rect,
            target,
            theme.colors.focus,
            theme.focus_width,
        )
    })
}
