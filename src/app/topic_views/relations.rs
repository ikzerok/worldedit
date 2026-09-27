use super::{target_label, Navigation, PageChange, TopicViewKind};
use crate::theme::{self, *};
use egui::{Pos2, RichText, Sense, Stroke, Vec2};
use std::collections::BTreeMap;
use worldline_core::catalog::{Catalog, TargetRef};
use worldline_core::{RelationDirection, TopicProjectionResult};

pub(super) fn relation_projection(
    ui: &mut egui::Ui,
    catalog: &Catalog,
    result: &TopicProjectionResult,
    mode: TopicViewKind,
    navigation: &mut Option<Navigation>,
    page_change: &mut Option<PageChange>,
) {
    let title = match mode {
        TopicViewKind::Family => "明确家族与亲属关系",
        TopicViewKind::Organization => "明确组织成员与归属关系",
        TopicViewKind::History => "映射关系",
        TopicViewKind::Network => "明确关系",
    };
    ui.separator();
    ui.label(RichText::new(format!("{title} · {} 条", result.relations.edges.len())).strong());
    ui.label(theme::muted(
        "结构图按查询深度分列，不表示代际；每条原始边单独列出，循环或同端点多边不会被改写。",
    ));
    if result.relations.cycle_hint {
        ui.colored_label(
            GOLD,
            "当前页存在关系环；不推造祖先、后代或传递关系。此提示只覆盖当前页。",
        );
    }
    if result.relations.truncated {
        ui.colored_label(
            GOLD,
            "关系页已截断；继续读取会使用 core continuation，不会丢弃原始关系。",
        );
    }
    if result.relations.nodes.len() > 1 {
        relation_diagram(ui, catalog, result, navigation);
    }
    if result.relations.edges.is_empty() {
        ui.label(theme::muted("此筛选下没有匹配的显式关系。"));
    }
    for edge in &result.relations.edges {
        ui.push_id(("topic-edge", &edge.id), |ui| {
            ui.group(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&edge.role).strong());
                    if ui.link(format!("阅读关系 {}", edge.id)).clicked() {
                        *navigation = Some(Navigation::Read(TargetRef::new("relation", &edge.id)));
                    }
                    if ui
                        .small_button(format!("定位来源 {}:{}", edge.file, edge.line))
                        .clicked()
                    {
                        *navigation = Some(Navigation::Source(edge.file.clone(), edge.line));
                    }
                });
                let connector = if edge.direction == RelationDirection::Directed {
                    "→"
                } else {
                    "—"
                };
                ui.horizontal_wrapped(|ui| {
                    if ui.link(target_label(catalog, &edge.from_ref)).clicked() {
                        *navigation = Some(Navigation::Read(edge.from_ref.clone()));
                    }
                    ui.label(connector);
                    if ui.link(target_label(catalog, &edge.to_ref)).clicked() {
                        *navigation = Some(Navigation::Read(edge.to_ref.clone()));
                    }
                });
                ui.label(theme::muted(format!(
                    "类型 {} · 作者标签 {} · scope [{}]{}",
                    edge.relation_type,
                    edge.label,
                    edge.scope_refs
                        .iter()
                        .map(|scope| format!("{}:{}", scope.kind, scope.id))
                        .collect::<Vec<_>>()
                        .join(", "),
                    edge.source_note
                        .as_deref()
                        .map(|note| format!(" · 来源说明：{note}"))
                        .unwrap_or_default()
                )));
            });
        });
    }
    if result.relations.truncated {
        if let Some(next) = result
            .relations
            .continuation
            .as_ref()
            .map(|continuation| continuation.relation.offset)
        {
            ui.horizontal(|ui| {
                ui.label(format!("下一页关系偏移 {next}"));
                if ui.button("继续读取关系").clicked() {
                    *page_change = Some(PageChange::Relations(next));
                }
            });
        } else {
            ui.label(theme::muted(
                "当前页没有可前进的 continuation；需重新查询或提高预算。",
            ));
        }
    }
}

fn relation_diagram(
    ui: &mut egui::Ui,
    catalog: &Catalog,
    result: &TopicProjectionResult,
    navigation: &mut Option<Navigation>,
) {
    if ui.available_width() < 520.0 {
        ui.label(theme::muted("窄屏使用下方可访问关系列表。"));
        return;
    }
    let nodes = &result.relations.nodes;
    let max_depth = nodes
        .iter()
        .map(|node| node.depth as usize)
        .max()
        .unwrap_or(0);
    let rows = (0..=max_depth)
        .map(|depth| {
            nodes
                .iter()
                .filter(|node| node.depth as usize == depth)
                .count()
        })
        .max()
        .unwrap_or(1);
    let width = ui.available_width().max(520.0);
    let height = (54.0 + rows as f32 * 82.0).max(190.0);
    let (response, painter) = ui.allocate_painter(Vec2::new(width, height), Sense::hover());
    let rect = response.rect;
    let mut positions = BTreeMap::new();
    for depth in 0..=max_depth {
        let layer: Vec<_> = nodes
            .iter()
            .filter(|node| node.depth as usize == depth)
            .collect();
        let x = if max_depth == 0 {
            rect.center().x
        } else {
            rect.left() + 92.0 + (rect.width() - 184.0) * depth as f32 / max_depth as f32
        };
        for (row, node) in layer.iter().enumerate() {
            positions.insert(
                node.target.clone(),
                Pos2::new(x, rect.top() + 42.0 + row as f32 * 82.0),
            );
        }
    }
    for (index, edge) in result.relations.edges.iter().enumerate() {
        let (Some(from), Some(to)) = (positions.get(&edge.from_ref), positions.get(&edge.to_ref))
        else {
            continue;
        };
        painter.line_segment([*from, *to], Stroke::new(1.2_f32, MUTED));
        let offset = (index % 5) as f32 * 11.0 - 22.0;
        painter.text(
            from.lerp(*to, 0.5) + Vec2::new(0.0, offset),
            egui::Align2::CENTER_CENTER,
            &edge.role,
            egui::FontId::proportional(11.0),
            TEXT,
        );
    }
    for node in nodes {
        let Some(center) = positions.get(&node.target) else {
            continue;
        };
        let node_rect = egui::Rect::from_center_size(*center, Vec2::new(164.0, 44.0));
        painter.rect_filled(node_rect, 6.0, CARD);
        if ui
            .interact(
                node_rect,
                ui.id()
                    .with(("topic-node", &node.target.kind, &node.target.id)),
                Sense::click(),
            )
            .clicked()
        {
            *navigation = Some(Navigation::Read(node.target.clone()));
        }
        painter.rect_stroke(
            node_rect,
            6.0,
            Stroke::new(1.2_f32, BLUE),
            egui::StrokeKind::Inside,
        );
        painter.text(
            node_rect.center(),
            egui::Align2::CENTER_CENTER,
            target_label(catalog, &node.target),
            egui::FontId::proportional(11.0),
            TEXT,
        );
    }
}
