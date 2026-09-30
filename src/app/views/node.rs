use super::WIDTH;
use crate::theme::*;
use crate::visual::{lighten, truncated};
use egui::{Rect, Stroke, Vec2};
use worldline_core::GraphNode;
pub(in crate::app) enum NodeHeading<'a> {
    Entry,
    Sequence,
    Caption(&'a str),
}

pub(in crate::app) fn draw_node(
    painter: &egui::Painter,
    rect: Rect,
    node: &GraphNode,
    selected: bool,
    hovered: bool,
    heading: NodeHeading<'_>,
    search: &str,
) {
    let zoom = rect.width() / WIDTH;
    let entry = matches!(heading, NodeHeading::Entry);
    let matches = search.is_empty()
        || format!(
            "{} {} {}",
            node.name,
            node.summary.as_deref().unwrap_or(""),
            node.characters.join(" ")
        )
        .to_lowercase()
        .contains(&search.to_lowercase());
    let opacity = if matches { 1.0 } else { 0.35 };
    let color = if selected {
        ACCENT()
    } else if entry {
        BLUE()
    } else {
        BORDER()
    };
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 4.0)),
        10,
        egui::Color32::BLACK.gamma_multiply(0.18),
    );
    painter.rect_filled(
        rect,
        10,
        (if hovered { lighten(CARD()) } else { CARD() }).gamma_multiply(opacity),
    );
    painter.rect_stroke(
        rect,
        10,
        Stroke::new(
            if selected { 1.8_f32 } else { 1.0_f32 },
            color.gamma_multiply(opacity),
        ),
        egui::StrokeKind::Inside,
    );
    let origin = rect.min + Vec2::new(15.0, 13.0) * zoom;
    painter.text(
        origin,
        egui::Align2::LEFT_TOP,
        if let NodeHeading::Caption(caption) = heading {
            caption.to_string()
        } else {
            format!(
                "{:02}  {}",
                node.seq,
                if entry {
                    "入口事件"
                } else if node.is_event {
                    "事件"
                } else {
                    "场景"
                }
            )
        },
        egui::FontId::proportional(10.0 * zoom),
        if entry { BLUE() } else { MUTED() },
    );
    painter.text(
        origin + Vec2::new(0.0, 23.0) * zoom,
        egui::Align2::LEFT_TOP,
        truncated(node.summary.as_deref().unwrap_or(&node.name), 17),
        egui::FontId::proportional(15.0 * zoom),
        TEXT().gamma_multiply(opacity),
    );
    painter.text(
        origin + Vec2::new(0.0, 47.0) * zoom,
        egui::Align2::LEFT_TOP,
        truncated(&node.name, 28),
        egui::FontId::monospace(11.0 * zoom),
        MUTED().gamma_multiply(opacity),
    );
    let file = std::path::Path::new(&node.file)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    painter.text(
        origin + Vec2::new(0.0, 71.0) * zoom,
        egui::Align2::LEFT_TOP,
        format!("{} · {} 人物", truncated(&file, 20), node.characters.len()),
        egui::FontId::proportional(10.0 * zoom),
        MUTED().gamma_multiply(opacity),
    );
}
