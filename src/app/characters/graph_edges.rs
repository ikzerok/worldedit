//! 平行边按出现顺序分道；每条仍保留独立core来源。
use super::{graph::Edge, state};
use crate::{
    theme::*,
    visual::{draw_arrow, truncated},
};
use egui::{Pos2, Rect, Stroke, Vec2};
use std::collections::BTreeMap;
use worldline_core::{world_context::WorldContextKind as Kind, RelationDirection};
pub(super) fn draw_edges(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    canvas: Rect,
    rects: &BTreeMap<String, Rect>,
    edges: &[Edge],
) -> Option<(String, u32, u32)> {
    let mut source = None;
    let mut labels = Vec::new();
    let mut lanes: BTreeMap<(String, String), usize> = BTreeMap::new();
    for (index, edge) in edges.iter().enumerate() {
        let (from, to) = (state::key(&edge.from), state::key(&edge.to));
        let (Some(a), Some(b)) = (rects.get(&from), rects.get(&to)) else {
            continue;
        };
        let pair = if from <= to { (from, to) } else { (to, from) };
        let lane = lanes.entry(pair).or_default();
        let offset = *lane as f32 * 28.0;
        *lane += 1;
        let start = a.center();
        let end = b.center();
        let direction = (end - start).normalized();
        let midpoint = if start.distance(end) < 1.0 {
            start + Vec2::new(0.0, -62.0 - offset)
        } else {
            start.lerp(end, 0.5) + Vec2::new(-direction.y, direction.x) * offset
        };
        let caption = truncated(
            &format!("{}·{}", state::kind_label(&edge.kind), edge.label),
            14,
        );
        let galley =
            painter.layout_no_wrap(caption.clone(), egui::FontId::proportional(11.0), TEXT());
        let label_size = (galley.size() + Vec2::new(12.0, 8.0)).max(Vec2::new(96.0, 24.0));
        let hit =
            super::graph_layout::edge_label_rect(midpoint, label_size, canvas, rects, &labels);
        labels.push(hit);
        let midpoint = hit.center();
        let color = if edge.kind == Kind::TextMention {
            MUTED()
        } else {
            ACCENT()
        };
        let solid = matches!(
            edge.kind,
            Kind::FormalRelation | Kind::LegacyCharacterRelation
        );
        line(painter, start, midpoint, color, solid);
        line(painter, midpoint, end, color, solid);
        if edge.direction == RelationDirection::Directed && midpoint.distance(end) > 1.0 {
            draw_arrow(painter, midpoint, midpoint.lerp(end, 0.5), color);
        }
        painter.rect_filled(hit, 3, PANEL());
        painter.text(
            midpoint,
            egui::Align2::CENTER_CENTER,
            caption,
            egui::FontId::proportional(11.0),
            TEXT(),
        );
        if ui
            .interact(
                hit.intersect(canvas),
                ui.id().with(("context-edge", index)),
                egui::Sense::click(),
            )
            .on_hover_text(format!(
                "{} · {}\n{}:{} · {}\n点击定位此条真实来源",
                state::kind_label(&edge.kind),
                edge.label,
                edge.source.file,
                edge.source.line,
                state::precision_label(edge.source.precision)
            ))
            .clicked()
        {
            source = Some((
                edge.source.file.clone(),
                edge.source.line,
                edge.source.column.unwrap_or(1),
            ));
        }
    }
    source
}
fn line(painter: &egui::Painter, a: Pos2, b: Pos2, color: egui::Color32, solid: bool) {
    if solid {
        painter.line_segment([a, b], Stroke::new(1.5_f32, color));
        return;
    }
    let steps = (a.distance(b) / 10.0).max(1.0) as usize;
    for i in (0..steps).step_by(2) {
        painter.line_segment(
            [
                a.lerp(b, i as f32 / steps as f32),
                a.lerp(b, ((i + 1) as f32 / steps as f32).min(1.0)),
            ],
            Stroke::new(1.3_f32, color),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldline_core::world_context::{WorldContextPrecision, WorldContextSource};
    #[test]
    fn parallel_records_have_independent_visible_labels_and_real_source_lines() {
        let ctx = egui::Context::default();
        let edges = (0..3)
            .map(|i| Edge {
                from: worldline_core::TargetRef::new("character", "a"),
                to: worldline_core::TargetRef::new("character", "b"),
                label: format!("关系{i}"),
                kind: Kind::LegacyCharacterRelation,
                direction: RelationDirection::Directed,
                source: WorldContextSource {
                    file: "人物/真实来源.wl".into(),
                    line: i + 2,
                    column: None,
                    precision: WorldContextPrecision::Line,
                },
            })
            .collect::<Vec<_>>();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let canvas = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0));
                let rects = BTreeMap::from([
                    (
                        "character:a".into(),
                        Rect::from_center_size(Pos2::new(100.0, 180.0), Vec2::splat(40.0)),
                    ),
                    (
                        "character:b".into(),
                        Rect::from_center_size(Pos2::new(500.0, 180.0), Vec2::splat(40.0)),
                    ),
                ]);
                let painter = ui.painter_at(canvas);
                let _ = draw_edges(ui, &painter, canvas, &rects, &edges);
            });
        });
        let mut y = Vec::new();
        for shape in out.shapes {
            if let egui::Shape::Text(text) = shape.shape {
                if text.galley.job.text.starts_with("旧式人物关系·关系") {
                    y.push(text.pos.y);
                }
            }
        }
        assert_eq!(y.len(), 3);
        assert!(y.windows(2).all(|p| (p[1] - p[0]).abs() >= 20.0));
    }
}
