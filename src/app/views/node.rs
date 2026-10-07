use super::WIDTH;
use crate::theme::*;
use crate::visual::truncated;
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
    let color = if selected {
        ACCENT()
    } else if entry {
        BLUE()
    } else {
        CONTROL_BORDER()
    };
    let radius = shapes().control;
    let fill = if selected {
        SELECTION()
    } else if hovered {
        HOVER()
    } else {
        DOCUMENT()
    };
    painter.rect_filled(rect, radius, fill);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(if selected { 2.0_f32 } else { 1.0_f32 }, color),
        egui::StrokeKind::Inside,
    );
    match style_preset() {
        StylePreset::Technical => {
            painter.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::splat(1.0),
                    Vec2::new((rect.width() - 2.0).max(0.0), 26.0 * zoom),
                ),
                0,
                CHROME(),
            );
            painter.hline(
                rect.x_range(),
                rect.top() + 27.0 * zoom,
                Stroke::new(1.0_f32, BORDER()),
            );
        }
        StylePreset::Studio => {
            painter.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::new(0.0, 8.0 * zoom),
                    Vec2::new(3.0, (rect.height() - 16.0 * zoom).max(0.0)),
                ),
                0,
                color,
            );
        }
        StylePreset::Focus => {}
        StylePreset::Ledger => {
            let tag = Rect::from_min_size(
                rect.min + Vec2::new(8.0, 7.0) * zoom,
                Vec2::new((rect.width() * 0.55).min(132.0 * zoom), 22.0 * zoom),
            );
            painter.rect_filled(tag, 0, CHROME());
            painter.line_segment(
                [tag.right_top(), tag.right_bottom()],
                Stroke::new(1.0_f32, CONTROL_BORDER()),
            );
        }
        StylePreset::Manuscript => {
            painter.hline(
                (rect.left() + 15.0 * zoom)..=(rect.right() - 15.0 * zoom),
                rect.top() + 31.0 * zoom,
                Stroke::new(1.0_f32, BORDER()),
            );
        }
    }
    let origin = rect.min + Vec2::new(15.0, 10.0) * zoom;
    painter.text(
        origin,
        egui::Align2::LEFT_TOP,
        if let NodeHeading::Caption(caption) = heading {
            caption.to_owned()
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
        egui::FontId::proportional(11.0 * zoom),
        if entry { BLUE() } else { MUTED() },
    );
    painter.text(
        origin + Vec2::new(0.0, 25.0) * zoom,
        egui::Align2::LEFT_TOP,
        truncated(node.summary.as_deref().unwrap_or(&node.name), 17),
        egui::FontId::proportional(16.0 * zoom),
        if matches { TEXT() } else { MUTED() },
    );
    painter.text(
        origin + Vec2::new(0.0, 49.0) * zoom,
        egui::Align2::LEFT_TOP,
        truncated(&node.name, 28),
        egui::FontId::monospace(11.0 * zoom),
        MUTED(),
    );
    let file = std::path::Path::new(&node.file)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    painter.text(
        origin + Vec2::new(0.0, 73.0) * zoom,
        egui::Align2::LEFT_TOP,
        format!("{} · {} 人物", truncated(&file, 20), node.characters.len()),
        egui::FontId::proportional(10.0 * zoom),
        MUTED(),
    );
}
