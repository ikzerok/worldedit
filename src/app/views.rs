//! 可交互的时间线与事件关系图,结构和顺序均来自 core analysis。
use super::{Tab, WorldeditApp};
use crate::theme::{self, *};
use crate::visual::{bezier_points, draw_arrow, lighten, truncated};
use egui::{Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::collections::HashMap;
use worldline_core::{EdgeKind, GraphNode};

const WIDTH: f32 = 228.0;
const HEIGHT: f32 = 106.0;
const CELL: f32 = 286.0;
const LANE: f32 = 178.0;
const LEFT: f32 = 142.0;

impl WorldeditApp {
    pub(super) fn canvas_tab(&mut self, ctx: &egui::Context) {
        if self.tab == Tab::Timeline
            && self
                .snapshot
                .as_ref()
                .is_some_and(|s| !s.result.analysis.timeline.periods.is_empty())
        {
            self.temporal_tab(ctx);
            return;
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let graph = snapshot.result.analysis.graph.clone();
        let anchors = snapshot.result.analysis.anchors.clone();
        let timeline = self.tab == Tab::Timeline;
        egui::CentralPanel::default()
            .frame(theme::panel().fill(BG))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.heading(if timeline {
                            "时间线"
                        } else {
                            "事件关系图"
                        });
                        ui.label(theme::muted(format!(
                            "{} 条故事线  /  {} 个事件  /  {} 个文件",
                            graph.storyline_order.len(),
                            graph.nodes.iter().filter(|n| n.is_event).count(),
                            self.project.documents.len()
                        )));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(theme::primary("＋ 新建事件")).clicked() {
                            self.new_event(None);
                        }
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search)
                                .hint_text("搜索 ID / 名称 / 人物")
                                .desired_width(160.0),
                        );
                    });
                });
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.small_button("＋ 新建时段").clicked() {
                        self.new_period_dialog();
                    }
                    if let Some(from) = self.link_from.clone() {
                        ui.colored_label(ACCENT, format!("从 {from} 连线 · 点击目标事件"));
                        ui.label(theme::muted("选择文案"));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.link_label)
                                .desired_width(110.0)
                                .hint_text("留空设置直接出口"),
                        );
                        if ui.small_button("取消").clicked() {
                            self.link_from = None;
                        }
                    } else {
                        ui.label(theme::muted(if timeline {
                            "拖动卡片调整顺序与故事线 · 点击编辑 · 双击空白处添加"
                        } else {
                            "拖动卡片整理布局 · 拖动右侧圆点建立连接"
                        }));
                    }
                });
                ui.add_space(12.0);
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
                        painter.rect_filled(canvas, 10, BG);
                        let dot = 26.0 * zoom;
                        for x in 0..(total.x / dot) as i32 {
                            for y in 0..(total.y / dot) as i32 {
                                let pos = canvas.min
                                    + Vec2::new(x as f32 * dot + 8.0, y as f32 * dot + 8.0);
                                if painter.clip_rect().contains(pos) {
                                    painter.circle_filled(pos, 0.8, BORDER.gamma_multiply(0.6));
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
                                painter.rect_filled(band, 10, PANEL.gamma_multiply(0.7));
                                let (id, display) = &graph.storyline_order[li];
                                painter.rect_filled(
                                    Rect::from_min_size(
                                        band.min + Vec2::new(10.0, 20.0) * zoom,
                                        Vec2::new(3.0, 40.0) * zoom,
                                    ),
                                    2,
                                    if li % 2 == 0 { ACCENT } else { BLUE },
                                );
                                painter.text(
                                    to_screen(Pos2::new(24.0, 45.0 + li as f32 * LANE)),
                                    egui::Align2::LEFT_TOP,
                                    truncated(display, 9),
                                    egui::FontId::proportional(14.0 * zoom),
                                    TEXT,
                                );
                                painter.text(
                                    to_screen(Pos2::new(24.0, 73.0 + li as f32 * LANE)),
                                    egui::Align2::LEFT_TOP,
                                    format!("{}  /  {} 事件", truncated(id, 10), lane.len()),
                                    egui::FontId::proportional(10.0 * zoom),
                                    MUTED,
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
                        let root = |id: usize| -> usize {
                            if !timeline || graph.nodes[id].is_event {
                                return id;
                            }
                            let name = graph.nodes[id].name.split('.').next().unwrap_or("");
                            graph
                                .ids
                                .get(name)
                                .copied()
                                .map(|n| n as usize)
                                .unwrap_or(id)
                        };
                        for (edge_index, edge) in graph.edges.iter().enumerate() {
                            let (f, t) = (root(edge.from as usize), root(edge.to as usize));
                            if timeline && (edge.kind == EdgeKind::Enter || f == t) {
                                continue;
                            }
                            let (Some(from), Some(to)) = (rects.get(&f), rects.get(&t)) else {
                                continue;
                            };
                            let color = match edge.kind {
                                EdgeKind::Drift => BLUE,
                                EdgeKind::Choice => ACCENT,
                                EdgeKind::Divert => GOLD,
                                EdgeKind::Enter => MUTED,
                            };
                            let start = from.right_center();
                            let end = to.left_center();
                            let bend = (end.x - start.x)
                                .abs()
                                .mul_add(0.45, 40.0 * zoom)
                                .min(180.0 * zoom);
                            let c0 = start + Vec2::new(bend, 0.0);
                            let c1 = end - Vec2::new(bend, 0.0);
                            let points = bezier_points(start, c0, c1, end, 30);
                            painter.add(egui::Shape::line(
                                points,
                                Stroke::new(
                                    if edge.kind == EdgeKind::Drift {
                                        2.3_f32
                                    } else {
                                        1.5_f32
                                    },
                                    color.gamma_multiply(0.65),
                                ),
                            ));
                            draw_arrow(&painter, c1, end, color);
                            if edge.target_requirement.is_some()
                                || edge
                                    .contexts
                                    .iter()
                                    .any(|c| !c.conditions.is_empty() || !c.choices.is_empty())
                            {
                                let middle = bezier_points(start, c0, c1, end, 2)[1];
                                let response = ui
                                    .push_id(("edge-condition", edge_index), |ui| {
                                        ui.put(
                                            Rect::from_center_size(
                                                middle + Vec2::new(0.0, 8.0),
                                                Vec2::new(42.0, 20.0),
                                            ),
                                            egui::Button::new(RichText::new("条件").size(10.0))
                                                .small(),
                                        )
                                    })
                                    .inner;
                                if response.clicked() {
                                    source = Some((edge.file.clone(), edge.line));
                                }
                                response.on_hover_ui(|ui| {
                                    ui.label(RichText::new("显式条件上下文").strong());
                                    for (index, context) in edge.contexts.iter().enumerate() {
                                        if edge.contexts.len() > 1 {
                                            ui.label(format!("可能分支 {}", index + 1));
                                        }
                                        for choice in &context.choices {
                                            ui.label(format!("选择：{choice}"));
                                        }
                                        for condition in &context.conditions {
                                            ui.label(condition);
                                        }
                                    }
                                    if let Some(requirement) = &edge.target_requirement {
                                        ui.separator();
                                        ui.label(format!("目标准入：{requirement}"));
                                    }
                                    ui.label(theme::muted("不推演运行必然性。点击定位源文件。"));
                                });
                            }
                            if let Some(label) = &edge.label {
                                let middle = bezier_points(start, c0, c1, end, 2)[1];
                                painter.text(
                                    middle + Vec2::new(0.0, -9.0),
                                    egui::Align2::CENTER_BOTTOM,
                                    truncated(label, 14),
                                    egui::FontId::proportional(10.0 * zoom),
                                    color,
                                );
                            }
                        }
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
                            if anchors.iter().any(|a| a.node == node.name) {
                                painter.text(
                                    rect.right_bottom() - Vec2::new(12.0, 12.0) * zoom,
                                    egui::Align2::RIGHT_BOTTOM,
                                    "◆",
                                    egui::FontId::proportional(11.0 * zoom),
                                    ACCENT,
                                );
                            }
                            response.clone().on_hover_text(format!(
                                "{}\n{}:{}\n拖动卡片调整位置,右侧圆点用于连线",
                                node.summary.as_deref().unwrap_or(&node.name),
                                node.file,
                                node.line
                            ));
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
                                painter.circle_filled(port.center(), 4.0 * zoom, ACCENT);
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
                                    Stroke::new(1.5_f32, ACCENT),
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
                                            Stroke::new(3.0_f32, ACCENT),
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
                                MUTED,
                            );
                        }
                    });
                ui.horizontal(|ui| {
                    ui.colored_label(ACCENT, "— 选择");
                    ui.colored_label(GOLD, "— 跃迁");
                    ui.colored_label(BLUE, "— 跨线漂流");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("＋").clicked() {
                            self.zoom = (self.zoom + 0.1).min(1.6);
                        }
                        ui.label(theme::muted(format!("{:.0}%", self.zoom * 100.0)));
                        if ui.small_button("−").clicked() {
                            self.zoom = (self.zoom - 0.1).max(0.5);
                        }
                        if ui.small_button("定位所选").clicked() {
                            self.focus_event =
                                self.event_editor.as_ref().map(|e| e.draft.id.clone());
                        }
                        if ui.small_button("适配全图").clicked() {
                            self.zoom = (viewport.x / width.max(1.0))
                                .min(viewport.y / height.max(1.0))
                                .clamp(0.25, 1.6);
                        }
                        if ui.small_button("重置布局").clicked() {
                            self.zoom = 1.0;
                            self.graph_positions.clear();
                        }
                    });
                });
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
    }
}

pub(super) enum NodeHeading<'a> {
    Entry,
    Sequence,
    Caption(&'a str),
}

pub(super) fn draw_node(
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
        ACCENT
    } else if entry {
        BLUE
    } else {
        BORDER
    };
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 4.0)),
        10,
        egui::Color32::BLACK.gamma_multiply(0.18),
    );
    painter.rect_filled(
        rect,
        10,
        (if hovered { lighten(CARD) } else { CARD }).gamma_multiply(opacity),
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
        if entry { BLUE } else { MUTED },
    );
    painter.text(
        origin + Vec2::new(0.0, 23.0) * zoom,
        egui::Align2::LEFT_TOP,
        truncated(node.summary.as_deref().unwrap_or(&node.name), 17),
        egui::FontId::proportional(15.0 * zoom),
        TEXT.gamma_multiply(opacity),
    );
    painter.text(
        origin + Vec2::new(0.0, 47.0) * zoom,
        egui::Align2::LEFT_TOP,
        truncated(&node.name, 28),
        egui::FontId::monospace(11.0 * zoom),
        MUTED.gamma_multiply(opacity),
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
        MUTED.gamma_multiply(opacity),
    );
}
