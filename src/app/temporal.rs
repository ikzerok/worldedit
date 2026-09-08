//! 按世界时段展示部分顺序;只有显式约束会产生先后关系。
use super::WorldeditApp;
use crate::theme::{self, *};
use crate::visual::{bezier_points, draw_arrow};
use egui::{Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::collections::{BTreeMap, HashMap};

impl WorldeditApp {
    pub(super) fn new_period_dialog(&mut self) {
        let mut index = 1;
        while self.snapshot.as_ref().is_some_and(|s| {
            s.result
                .analysis
                .timeline
                .periods
                .iter()
                .any(|p| p.id == format!("period_{index}"))
        }) {
            index += 1;
        }
        self.new_period = Some((format!("period_{index}"), "新时段".into(), None));
    }

    pub(super) fn temporal_tab(&mut self, ctx: &egui::Context) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let graph = snapshot.result.analysis.graph.clone();
        let timeline = snapshot.result.analysis.timeline.clone();
        egui::CentralPanel::default()
            .frame(theme::panel().fill(BG))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.heading("世界时间线");
                        ui.label(theme::muted(format!(
                            "{} 个时段 · 无序事件与局部有序链并存",
                            timeline.periods.len()
                        )));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(theme::primary("＋ 事件")).clicked() {
                            self.new_event(None);
                        }
                        if ui.button("＋ 时段").clicked() {
                            self.new_period_dialog();
                        }
                    });
                });
                ui.add_space(12.0);
                ui.horizontal_wrapped(|ui| {
                    if let Some(from) = self.link_from.clone() {
                        ui.colored_label(
                            ACCENT,
                            format!("{from} 先发生 → 点击同一时段中较晚的事件"),
                        );
                        if ui.small_button("取消").clicked() {
                            self.link_from = None;
                        }
                    } else {
                        ui.label(theme::muted(
                            "连线明确先后 · 未连接的事件不要求固定顺序 · 拖动只整理版面",
                        ));
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("搜索事件 / 人物")
                            .desired_width(150.0),
                    );
                });
                ui.add_space(12.0);
                let mut groups = Vec::new();
                let mut positions = HashMap::<usize, Pos2>::new();
                let mut top = 0.0;
                let mut width = ui.available_width() / self.zoom;
                let period_order = timeline.period_order();
                let depths: HashMap<_, _> = period_order
                    .iter()
                    .map(|(p, depth)| (p.id.clone(), *depth))
                    .collect();
                for (period, depth) in &period_order {
                    let mut ranks: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
                    for event in timeline.events.iter().filter(|e| e.period == period.id) {
                        if let Some(&id) = graph.ids.get(&event.event) {
                            ranks.entry(event.rank).or_default().push(id as usize);
                        }
                    }
                    let count = ranks.values().map(Vec::len).sum::<usize>();
                    let rows = ranks.values().map(Vec::len).max().unwrap_or(0);
                    let columns = ranks.keys().max().copied().unwrap_or(0) + 1;
                    let height = 74.0 + rows as f32 * 132.0;
                    width = width.max(48.0 + columns as f32 * 280.0 + *depth as f32 * 28.0);
                    for (rank, items) in &mut ranks {
                        items.sort_by_key(|&i| &graph.nodes[i].name);
                        for (row, &id) in items.iter().enumerate() {
                            positions.insert(
                                id,
                                Pos2::new(
                                    24.0 + *rank as f32 * 280.0 + *depth as f32 * 28.0,
                                    top + 68.0 + row as f32 * 132.0,
                                ),
                            );
                        }
                    }
                    groups.push((
                        Some(period.id.clone()),
                        period.display.clone(),
                        top,
                        height,
                        count,
                    ));
                    top += height + 18.0;
                }
                let mut ungrouped: Vec<_> = graph
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| {
                        n.is_event && !timeline.events.iter().any(|e| e.event == n.name)
                    })
                    .map(|(i, _)| i)
                    .collect();
                ungrouped.sort_by_key(|&i| {
                    (
                        &graph.nodes[i].storyline,
                        graph.nodes[i].seq,
                        &graph.nodes[i].name,
                    )
                });
                if !ungrouped.is_empty() {
                    let rows = ungrouped.len().div_ceil(2);
                    let height = 94.0 + rows as f32 * 132.0;
                    for (i, &node) in ungrouped.iter().enumerate() {
                        positions.insert(
                            node,
                            Pos2::new(
                                24.0 + (i % 2) as f32 * 280.0,
                                top + 68.0 + (i / 2) as f32 * 132.0,
                            ),
                        );
                    }
                    groups.push((
                        None,
                        "未分配时段 · 按编排序号整理".into(),
                        top,
                        height,
                        ungrouped.len(),
                    ));
                    top += height;
                }
                let zoom = self.zoom;
                let viewport = ui.available_size() - Vec2::new(0.0, 36.0);
                let mut select = None;
                let mut connect = None;
                let mut add = None;
                egui::ScrollArea::both()
                    .id_salt("temporal-canvas")
                    .auto_shrink([false, false])
                    .max_height((ui.available_height() - 36.0).max(180.0))
                    .show(ui, |ui| {
                        let (canvas, background) = ui.allocate_exact_size(
                            Vec2::new(width * zoom, (top * zoom).max(ui.available_height())),
                            Sense::click(),
                        );
                        let painter = ui.painter_at(canvas);
                        let screen = |p: Pos2| canvas.min + p.to_vec2() * zoom;
                        for (group_index, (id, display, y, height, count)) in
                            groups.iter().enumerate()
                        {
                            let depth = id
                                .as_ref()
                                .and_then(|id| depths.get(id))
                                .copied()
                                .unwrap_or(0);
                            let descendants = groups
                                .iter()
                                .skip(group_index + 1)
                                .take_while(|(id, _, _, _, _)| {
                                    id.as_ref()
                                        .and_then(|id| depths.get(id))
                                        .is_some_and(|d| *d > depth)
                                })
                                .collect::<Vec<_>>();
                            let bottom = descendants
                                .last()
                                .map(|(_, _, y, h, _)| y + h)
                                .unwrap_or(y + height);
                            let indent = depth as f32 * 28.0;
                            let band = Rect::from_min_size(
                                screen(Pos2::new(indent, *y)),
                                Vec2::new(width - indent, bottom - y) * zoom,
                            );
                            painter.rect_filled(band, 12, PANEL);
                            painter.rect_stroke(
                                band,
                                12,
                                Stroke::new(1.0_f32, BORDER),
                                egui::StrokeKind::Inside,
                            );
                            painter.text(
                                band.min + Vec2::new(24.0, 18.0) * zoom,
                                egui::Align2::LEFT_TOP,
                                display,
                                egui::FontId::proportional(16.0 * zoom),
                                TEXT,
                            );
                            painter.text(
                                band.min + Vec2::new(24.0, 42.0) * zoom,
                                egui::Align2::LEFT_TOP,
                                format!(
                                    "{} · {count} 个直属事件 · {} 个子时段",
                                    id.as_deref().unwrap_or("未归组"),
                                    descendants.len()
                                ),
                                egui::FontId::proportional(11.0 * zoom),
                                MUTED,
                            );
                            let button = Rect::from_min_size(
                                band.min + Vec2::new(width - indent - 116.0, 16.0) * zoom,
                                Vec2::new(94.0, 30.0) * zoom,
                            );
                            if ui
                                .put(
                                    button,
                                    egui::Button::new(
                                        egui::RichText::new("＋ 事件").size(14.0 * zoom),
                                    )
                                    .wrap_mode(egui::TextWrapMode::Extend),
                                )
                                .clicked()
                            {
                                add = Some(id.clone());
                            }
                            if let Some(id) = id {
                                let child_button = Rect::from_min_size(
                                    band.min + Vec2::new(width - indent - 220.0, 16.0) * zoom,
                                    Vec2::new(98.0, 30.0) * zoom,
                                );
                                if ui
                                    .put(
                                        child_button,
                                        egui::Button::new(
                                            egui::RichText::new("＋ 子时段").size(14.0 * zoom),
                                        )
                                        .wrap_mode(egui::TextWrapMode::Extend),
                                    )
                                    .clicked()
                                {
                                    self.new_period_dialog();
                                    if let Some((_, _, parent)) = &mut self.new_period {
                                        *parent = Some(id.clone());
                                    }
                                }
                                let label = Rect::from_min_size(
                                    band.min,
                                    Vec2::new(
                                        (width - indent - 230.0).max(50.0) * zoom,
                                        52.0 * zoom,
                                    ),
                                );
                                if ui
                                    .interact(
                                        label,
                                        egui::Id::new(("period-title", id)),
                                        Sense::click(),
                                    )
                                    .double_clicked()
                                {
                                    self.new_period = Some((
                                        id.clone(),
                                        display.clone(),
                                        timeline
                                            .periods
                                            .iter()
                                            .find(|p| &p.id == id)
                                            .and_then(|p| p.parent.clone()),
                                    ));
                                }
                            }
                        }
                        let rects: HashMap<_, _> = positions
                            .iter()
                            .map(|(&id, pos)| {
                                let pos = self
                                    .temporal_positions
                                    .get(&graph.nodes[id].name)
                                    .unwrap_or(pos);
                                (
                                    id,
                                    Rect::from_min_size(
                                        screen(*pos),
                                        Vec2::new(228.0, 106.0) * zoom,
                                    ),
                                )
                            })
                            .collect();
                        if let Some(target) = &self.focus_event {
                            if let Some(rect) = graph
                                .ids
                                .get(target)
                                .and_then(|id| rects.get(&(*id as usize)))
                            {
                                ui.scroll_to_rect(rect.expand(22.0), Some(egui::Align::Center));
                                self.focus_event = None;
                            }
                        }
                        for edge in &timeline.edges {
                            let (Some(before), Some(after)) =
                                (graph.ids.get(&edge.before), graph.ids.get(&edge.after))
                            else {
                                continue;
                            };
                            let (Some(a), Some(b)) = (
                                rects.get(&(*before as usize)),
                                rects.get(&(*after as usize)),
                            ) else {
                                continue;
                            };
                            let start = a.right_center();
                            let end = b.left_center();
                            let c0 = start + Vec2::new(42.0 * zoom, 0.0);
                            let c1 = end - Vec2::new(42.0 * zoom, 0.0);
                            painter.add(egui::Shape::line(
                                bezier_points(start, c0, c1, end, 24),
                                Stroke::new(1.8_f32, ACCENT),
                            ));
                            draw_arrow(&painter, c1, end, ACCENT);
                            painter.text(
                                start.lerp(end, 0.5) - Vec2::new(0.0, 12.0),
                                egui::Align2::CENTER_BOTTOM,
                                "先于",
                                egui::FontId::proportional(11.0 * zoom),
                                ACCENT,
                            );
                        }
                        for (&id, rect) in &rects {
                            if !ui.is_rect_visible(*rect) {
                                continue;
                            }
                            let node = &graph.nodes[id];
                            let response = ui.interact(
                                *rect,
                                egui::Id::new(("temporal-event", &node.name)),
                                Sense::click_and_drag(),
                            );
                            let selected = self
                                .event_editor
                                .as_ref()
                                .is_some_and(|e| e.draft.id == node.name);
                            let caption = format!(
                                "{} · {}",
                                node.storyline,
                                if timeline
                                    .edges
                                    .iter()
                                    .any(|e| e.before == node.name || e.after == node.name)
                                {
                                    "有先后约束"
                                } else {
                                    "自由事件"
                                }
                            );
                            super::views::draw_node(
                                &painter,
                                *rect,
                                node,
                                selected,
                                response.hovered(),
                                super::views::NodeHeading::Caption(&caption),
                                &self.search,
                            );
                            if response.clicked() {
                                if let Some(from) = &self.link_from {
                                    if from != &node.name {
                                        connect = Some((from.clone(), node.name.clone()));
                                    }
                                } else {
                                    select = Some(node.name.clone());
                                }
                            }
                            if response.double_clicked() {
                                self.jump_to_file(&node.file, node.line, 1);
                            }
                            if response.dragged() && self.link_from.is_none() {
                                let pos = self
                                    .temporal_positions
                                    .entry(node.name.clone())
                                    .or_insert(positions[&id]);
                                *pos += ui.input(|i| i.pointer.delta()) / zoom;
                                pos.x = pos.x.clamp(12.0, width - 240.0);
                                if let Some((_, _, y, height, _)) =
                                    groups.iter().find(|(_, _, y, height, _)| {
                                        positions[&id].y >= *y && positions[&id].y < *y + *height
                                    })
                                {
                                    pos.y = pos.y.clamp(*y + 66.0, *y + *height - 118.0);
                                }
                            }
                            let port =
                                Rect::from_center_size(rect.right_center(), Vec2::splat(18.0));
                            painter.circle_filled(port.center(), 4.0, ACCENT);
                            let response = ui.interact(
                                port,
                                egui::Id::new(("temporal-port", &node.name)),
                                Sense::click_and_drag(),
                            );
                            if response.drag_started() || response.clicked() {
                                self.link_from = Some(node.name.clone());
                            }
                        }
                        if let Some(from) = &self.link_from {
                            if let (Some(id), Some(pointer)) =
                                (graph.ids.get(from), ui.input(|i| i.pointer.hover_pos()))
                            {
                                if let Some(rect) = rects.get(&(*id as usize)) {
                                    painter.line_segment(
                                        [rect.right_center(), pointer],
                                        Stroke::new(1.5_f32, ACCENT),
                                    );
                                }
                                if ui.input(|i| i.pointer.any_released()) {
                                    if let Some((&to, _)) = rects.iter().find(|(i, r)| {
                                        r.contains(pointer) && graph.nodes[**i].name != *from
                                    }) {
                                        connect =
                                            Some((from.clone(), graph.nodes[to].name.clone()));
                                    }
                                }
                            }
                        }
                        background.context_menu(|ui| {
                            if ui.button("＋ 新建时段").clicked() {
                                self.new_period_dialog();
                                ui.close();
                            }
                        });
                    });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("— 先后约束").color(ACCENT).size(12.0));
                    if ui.small_button("−").clicked() {
                        self.zoom = (self.zoom - 0.1).max(0.5);
                    }
                    ui.label(theme::muted(format!("{:.0}%", self.zoom * 100.0)));
                    if ui.small_button("＋").clicked() {
                        self.zoom = (self.zoom + 0.1).min(1.6);
                    }
                    if ui.small_button("按约束整理").clicked() {
                        self.temporal_positions.clear();
                    }
                    if ui.small_button("定位所选").clicked() {
                        self.focus_event = self.event_editor.as_ref().map(|e| e.draft.id.clone());
                    }
                    if ui.small_button("适配全图").clicked() {
                        self.zoom = (viewport.x / width.max(1.0))
                            .min(viewport.y / top.max(1.0))
                            .clamp(0.25, 1.6);
                    }
                });
                if let Some(id) = select {
                    self.select_event(&id);
                }
                if let Some(period) = add {
                    self.new_event(None);
                    if let Some(editor) = &mut self.event_editor {
                        editor.draft.period = period;
                    }
                }
                if let Some((before, after)) = connect {
                    if self.commit("先后约束已写入源文件", |p| {
                        p.order_events(&before, &after)
                    }) {
                        self.link_from = None;
                        self.temporal_positions.clear();
                        self.select_event(&after);
                    }
                }
            });
    }
}
