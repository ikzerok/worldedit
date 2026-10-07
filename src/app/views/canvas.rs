use super::super::{Tab, WorldeditApp};
use super::canvas_controls;
use super::node::{draw_node, NodeHeading};
use super::{CELL, HEIGHT, LANE, LEFT, WIDTH};
use crate::theme::{self, *};
use crate::visual::truncated;
use egui::{Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::collections::HashMap;
use worldline_core::GraphNode;

impl WorldeditApp {
    pub(in crate::app) fn canvas_tab(&mut self, ctx: &egui::Context) {
        if self.tab == Tab::Timeline
            && self.snapshot.as_ref().is_some_and(|s| {
                !s.result.analysis.timeline.periods.is_empty()
                    || s.result.analysis.timeline.status
                        == worldline_core::timeline::TimelineStatus::Partial
            })
        {
            self.temporal_tab(ctx);
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let graph = snapshot.result.analysis.graph.clone();
        let anchors = snapshot.result.analysis.anchors.clone();
        let visit_coverage = self
            .replay_debugger
            .result
            .as_ref()
            .map(|result| result.coverage.visited_nodes.clone());
        let timeline = self.tab == Tab::Timeline;
        egui::CentralPanel::default()
            .frame(theme::panel().fill(BG()))
            .show(ctx, |ui| {
                let storyline_count = graph.storyline_order.len();
                let event_count = graph.nodes.iter().filter(|node| node.is_event).count();
                let document_count = self.project.documents.len();
                canvas_controls::header(
                    self,
                    ui,
                    timeline,
                    storyline_count,
                    event_count,
                    document_count,
                    visit_coverage.is_some(),
                );
                let zoom = self.zoom;
                let viewport = ui.available_size() - Vec2::new(0.0, 42.0);
                let mut positions = HashMap::<usize, Pos2>::new();
                let mut lanes: Vec<Vec<usize>> = vec![Vec::new(); graph.storyline_order.len()];
                for (i, node) in graph.nodes.iter().enumerate() {
                    if node.is_event {
                        if let Some(lane) = graph
                            .storyline_order
                            .iter()
                            .position(|(id, _)| id == &node.storyline)
                        {
                            lanes[lane].push(i);
                        }
                    }
                }
                for lane in &mut lanes {
                    lane.sort_by_key(|&i| (graph.nodes[i].seq, &graph.nodes[i].name));
                }
                if timeline {
                    for (l, lane) in lanes.iter().enumerate() {
                        for (k, &i) in lane.iter().enumerate() {
                            positions.insert(
                                i,
                                Pos2::new(LEFT + k as f32 * CELL, 42.0 + l as f32 * LANE),
                            );
                        }
                    }
                } else {
                    let last = graph
                        .depth
                        .iter()
                        .copied()
                        .filter(|&d| d != u32::MAX)
                        .max()
                        .unwrap_or(0)
                        + 1;
                    let mut rows = HashMap::<u32, usize>::new();
                    for (i, node) in graph.nodes.iter().enumerate() {
                        let column = if graph.depth[i] == u32::MAX {
                            last
                        } else {
                            graph.depth[i]
                        };
                        let row = rows.entry(column).or_default();
                        let default =
                            Pos2::new(30.0 + column as f32 * CELL, 40.0 + *row as f32 * 154.0);
                        positions
                            .insert(i, *self.graph_positions.get(&node.name).unwrap_or(&default));
                        *row += 1;
                    }
                }
                let max_count = lanes.iter().map(Vec::len).max().unwrap_or(0);
                let width = if timeline {
                    LEFT + (max_count + 1) as f32 * CELL
                } else {
                    positions
                        .values()
                        .map(|p| p.x + WIDTH + 70.0)
                        .fold(0.0, f32::max)
                };
                let height = if timeline {
                    lanes.len() as f32 * LANE + 34.0
                } else {
                    positions
                        .values()
                        .map(|p| p.y + HEIGHT + 70.0)
                        .fold(0.0, f32::max)
                };
                let mut chosen = None;
                let mut source = None;
                let mut add_lane = None;
                let mut moved = None;
                let mut connect = None;
                egui::ScrollArea::both()
                    .id_salt(if timeline {
                        "timeline-canvas"
                    } else {
                        "graph-canvas"
                    })
                    .auto_shrink([false, false])
                    .max_height((ui.available_height() - 42.0).max(200.0))
                    .show(ui, |ui| {
                        let total = Vec2::new(
                            (width * zoom).max(ui.available_width()),
                            (height * zoom).max(ui.available_height()),
                        );
                        let (canvas, background) = ui.allocate_exact_size(total, Sense::click());
                        let painter = ui.painter_at(canvas);
                        if let Some(target) = &self.focus_event {
                            if let Some((_, pos)) = positions
                                .iter()
                                .find(|(id, _)| graph.nodes[**id].name == *target)
                            {
                                let rect = Rect::from_min_size(
                                    canvas.min + pos.to_vec2() * zoom,
                                    Vec2::new(WIDTH, HEIGHT) * zoom,
                                );
                                ui.scroll_to_rect(rect.expand(22.0), Some(egui::Align::Center));
                                self.focus_event = None;
                            }
                        }
                        painter.rect_filled(canvas, 0, BG());
                        let dot = 26.0 * zoom;
                        for x in 0..(total.x / dot) as i32 {
                            for y in 0..(total.y / dot) as i32 {
                                let pos = canvas.min
                                    + Vec2::new(x as f32 * dot + 8.0, y as f32 * dot + 8.0);
                                if painter.clip_rect().contains(pos) {
                                    painter.circle_filled(pos, 0.8, BORDER().gamma_multiply(0.6));
                                }
                            }
                        }
                        let to_screen = |p: Pos2| canvas.min + p.to_vec2() * zoom;
                        let rects: HashMap<usize, Rect> = positions
                            .iter()
                            .map(|(&i, &p)| {
                                (
                                    i,
                                    Rect::from_min_size(
                                        to_screen(p),
                                        Vec2::new(WIDTH, HEIGHT) * zoom,
                                    ),
                                )
                            })
                            .collect();
                        if timeline {
                            for (li, lane) in lanes.iter().enumerate() {
                                let band = Rect::from_min_size(
                                    to_screen(Pos2::new(0.0, 20.0 + li as f32 * LANE)),
                                    Vec2::new(width * zoom, (LANE - 14.0) * zoom),
                                );
                                painter.rect_filled(band, theme::shapes().document, PANEL());
                                let (id, display) = &graph.storyline_order[li];
                                painter.rect_filled(
                                    Rect::from_min_size(
                                        band.min + Vec2::new(10.0, 20.0) * zoom,
                                        Vec2::new(3.0, 40.0) * zoom,
                                    ),
                                    2,
                                    if li % 2 == 0 { ACCENT() } else { BLUE() },
                                );
                                painter.text(
                                    to_screen(Pos2::new(24.0, 45.0 + li as f32 * LANE)),
                                    egui::Align2::LEFT_TOP,
                                    truncated(display, 9),
                                    egui::FontId::proportional(14.0 * zoom),
                                    TEXT(),
                                );
                                painter.text(
                                    to_screen(Pos2::new(24.0, 73.0 + li as f32 * LANE)),
                                    egui::Align2::LEFT_TOP,
                                    format!("{}  /  {} 事件", truncated(id, 10), lane.len()),
                                    egui::FontId::proportional(10.0 * zoom),
                                    MUTED(),
                                );
                                let add = Rect::from_min_size(
                                    to_screen(Pos2::new(
                                        LEFT + lane.len() as f32 * CELL,
                                        74.0 + li as f32 * LANE,
                                    )),
                                    Vec2::new(96.0, 36.0) * zoom,
                                );
                                if ui
                                    .put(
                                        add,
                                        egui::Button::new(
                                            RichText::new("＋ 添加事件").size(12.0 * zoom),
                                        ),
                                    )
                                    .clicked()
                                {
                                    add_lane = Some(id.clone());
                                }
                            }
                        }
                        super::parallel_edges::draw_groups(
                            self, ui, &graph, &rects, canvas, zoom, timeline,
                        );
                        // 分组按钮先处理键盘/鼠标打开，当前帧即停用底层提示。
                        let show_node_tooltips = !self.command_palette.open
                            && !super::parallel_edges::details_visible(self, ctx);
                        let mut over_node = false;
                        for (&i, rect) in &rects {
                            if !ui.is_rect_visible(*rect) {
                                continue;
                            }
                            let node = &graph.nodes[i];
                            let response = ui.interact(
                                *rect,
                                egui::Id::new(("node", timeline, &node.name)),
                                Sense::click_and_drag(),
                            );
                            over_node |= response.hovered();
                            let selected = self
                                .event_editor
                                .as_ref()
                                .is_some_and(|e| e.draft.id == node.name);
                            draw_node(
                                &painter,
                                *rect,
                                node,
                                selected,
                                response.hovered(),
                                if i == graph.entry as usize {
                                    NodeHeading::Entry
                                } else {
                                    NodeHeading::Sequence
                                },
                                &self.search,
                            );
                            let visit_count = visit_coverage
                                .as_ref()
                                .and_then(|coverage| coverage.get(&node.name))
                                .copied();
                            if let Some(count) = visit_count {
                                draw_visit_marker(&painter, *rect, zoom, count);
                            }
                            if anchors.iter().any(|a| a.node == node.name) {
                                painter.text(
                                    rect.right_bottom() - Vec2::new(12.0, 12.0) * zoom,
                                    egui::Align2::RIGHT_BOTTOM,
                                    "◆",
                                    egui::FontId::proportional(11.0 * zoom),
                                    ACCENT(),
                                );
                            }
                            if show_node_tooltips {
                                response.clone().on_hover_ui(|ui| {
                                    node_hover_ui(ui, node, visit_count);
                                });
                            }
                            if response.clicked() {
                                if let Some(from) = self.link_from.clone() {
                                    if node.is_event && from != node.name {
                                        connect = Some((from, node.name.clone()));
                                    }
                                } else if node.is_event {
                                    chosen = Some(node.name.clone());
                                } else {
                                    source = Some((node.file.clone(), node.line));
                                }
                            }
                            if response.double_clicked() {
                                source = Some((node.file.clone(), node.line));
                            }
                            if response.dragged() && self.link_from.is_none() {
                                if timeline {
                                    self.dragging = Some(node.name.clone());
                                } else {
                                    let position = self
                                        .graph_positions
                                        .entry(node.name.clone())
                                        .or_insert(positions[&i]);
                                    *position += ui.input(|input| input.pointer.delta()) / zoom;
                                    position.x = position.x.max(12.0);
                                    position.y = position.y.max(12.0);
                                }
                            }
                            if node.is_event {
                                let port = Rect::from_center_size(
                                    rect.right_center(),
                                    Vec2::splat(18.0 * zoom),
                                );
                                painter.circle_filled(port.center(), 4.0 * zoom, ACCENT());
                                let port_response = ui.interact(
                                    port,
                                    egui::Id::new(("port", timeline, &node.name)),
                                    Sense::click_and_drag(),
                                );
                                if port_response.drag_started() || port_response.clicked() {
                                    self.link_from = Some(node.name.clone());
                                }
                            }
                        }
                        if let Some(from) = &self.link_from {
                            if let (Some((&i, _)), Some(pointer)) = (
                                positions
                                    .iter()
                                    .find(|(i, _)| graph.nodes[**i].name == *from),
                                ui.input(|i| i.pointer.hover_pos()),
                            ) {
                                painter.line_segment(
                                    [rects[&i].right_center(), pointer],
                                    Stroke::new(1.5_f32, ACCENT()),
                                );
                                if ui.input(|i| i.pointer.any_released()) {
                                    if let Some((&target, _)) = rects.iter().find(|(i, r)| {
                                        r.contains(pointer)
                                            && graph.nodes[**i].is_event
                                            && graph.nodes[**i].name != *from
                                    }) {
                                        connect =
                                            Some((from.clone(), graph.nodes[target].name.clone()));
                                    }
                                }
                            }
                        }
                        if timeline {
                            if let Some(id) = self.dragging.clone() {
                                if let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) {
                                    let local = (pointer - canvas.min) / zoom;
                                    let lane = ((local.y - 20.0) / LANE).floor().max(0.0) as usize;
                                    let slot =
                                        ((local.x - LEFT + CELL * 0.5) / CELL).floor().max(0.0)
                                            as usize;
                                    if let Some((lane_id, _)) = graph.storyline_order.get(lane) {
                                        let x = to_screen(Pos2::new(
                                            LEFT + slot.min(lanes[lane].len()) as f32 * CELL - 10.0,
                                            35.0 + lane as f32 * LANE,
                                        ));
                                        painter.line_segment(
                                            [x, x + Vec2::new(0.0, 122.0 * zoom)],
                                            Stroke::new(3.0_f32, ACCENT()),
                                        );
                                        if ui.input(|i| i.pointer.any_released())
                                            && canvas.contains(pointer)
                                            && ui.clip_rect().contains(pointer)
                                        {
                                            moved = Some((id, lane_id.clone(), slot));
                                        }
                                    }
                                }
                                if ui.input(|i| i.pointer.any_released()) {
                                    self.dragging = None;
                                }
                            }
                        }
                        if background.double_clicked() && !over_node {
                            let lane = background
                                .interact_pointer_pos()
                                .and_then(|p| {
                                    graph
                                        .storyline_order
                                        .get((((p.y - canvas.min.y) / zoom - 20.0) / LANE).max(0.0)
                                            as usize)
                                })
                                .map(|l| l.0.clone());
                            self.new_event(lane.as_deref());
                        }
                        background.context_menu(|ui| {
                            if ui.button("＋ 新建事件").clicked() {
                                self.new_event(None);
                                ui.close();
                            }
                        });
                        if graph.nodes.is_empty() {
                            painter.text(
                                canvas.center(),
                                egui::Align2::CENTER_CENTER,
                                "从第一个事件开始",
                                egui::FontId::proportional(20.0),
                                MUTED(),
                            );
                        }
                    });
                canvas_controls::footer(self, ui, viewport, width, height);
                if let Some(id) = chosen {
                    self.select_event(&id);
                }
                if let Some((file, line)) = source {
                    self.jump_to_file(&file, line, 1);
                }
                if let Some(lane) = add_lane {
                    self.new_event(Some(&lane));
                }
                if let Some((id, lane, slot)) = moved {
                    if self.commit("时间线顺序已写入源文件", |p| {
                        p.move_event(&id, &lane, slot)
                    }) {
                        self.select_event(&id);
                    }
                }
                if let Some((from, to)) = connect {
                    let drift = graph
                        .nodes
                        .iter()
                        .find(|n| n.name == from)
                        .zip(graph.nodes.iter().find(|n| n.name == to))
                        .is_some_and(|(a, b)| a.storyline != b.storyline);
                    let label = self.link_label.clone();
                    if self.commit("连接已写入事件正文", |p| {
                        p.connect_events(&from, &to, &label, drift)
                    }) {
                        self.link_from = None;
                        self.select_event(&from);
                    }
                }
            });
        self.graph_edge_details(ctx);
    }
}

fn draw_visit_marker(painter: &egui::Painter, rect: Rect, zoom: f32, count: u32) {
    painter.text(
        rect.right_top() + Vec2::new(-12.0, 10.0) * zoom,
        egui::Align2::RIGHT_TOP,
        format!("访问 ×{count}"),
        egui::FontId::proportional(9.0 * zoom),
        SUCCESS(),
    );
}

fn node_hover_ui(ui: &mut egui::Ui, node: &GraphNode, visit_count: Option<u32>) {
    ui.label(format!(
        "{}\n{}:{}\n拖动卡片调整位置,右侧圆点用于连线",
        node.summary.as_deref().unwrap_or(&node.name),
        node.file,
        node.line
    ));
    if let Some(count) = visit_count {
        // 覆盖数保留文字标识；缩小画布后仍可按正常字号查看。
        ui.label(
            RichText::new(format!("访问 ×{count}"))
                .color(SUCCESS())
                .size(META_SIZE),
        );
    }
}

#[cfg(test)]
mod visit_tests;
