//! typed上下文的图形投影；布局只影响个人视图。
use super::{
    graph_layout::{NODE_SIZE, SELECTED_SIZE},
    state, WorldeditApp,
};
use crate::{
    theme::{self, *},
    visual::truncated,
};
use egui::{Rect, Sense, Stroke, Vec2};
use std::collections::{BTreeMap, BTreeSet};
use worldline_core::{
    world_context::{WorldContextKind as Kind, WorldContextPrecision, WorldContextSource},
    RelationDirection, TargetRef,
};

struct Node {
    target: TargetRef,
    name: String,
    exists: bool,
    source: Option<(String, u32)>,
}
pub(super) struct Edge {
    pub from: TargetRef,
    pub to: TargetRef,
    pub label: String,
    pub kind: Kind,
    pub direction: RelationDirection,
    pub source: WorldContextSource,
}

impl WorldeditApp {
    pub(super) fn character_context_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_value(&mut self.character_focus.full, false, "焦点一跳")
                .changed()
                || ui
                    .selectable_value(&mut self.character_focus.full, true, "全人物 · 旧式关系")
                    .changed()
            {
                self.character_focus.positions.clear();
                self.character_focus.layout_manual = false;
                self.character_focus.auto_fit = true;
                self.character_focus.fit = true;
            }
            if ui.button("定位所选").clicked() {
                self.character_focus.auto_fit = false;
                let point = self
                    .character_editor
                    .as_ref()
                    .and_then(|e| e.original.as_ref())
                    .and_then(|id| {
                        self.character_focus
                            .positions
                            .get(&format!("character:{id}"))
                    })
                    .copied()
                    .unwrap_or([0.0, 0.0]);
                let zoom = self.character_focus.camera.zoom;
                self.character_focus.camera.pan = [-point[0] * zoom, -point[1] * zoom];
            }
            if ui.button("适配当前结果").clicked() {
                self.character_focus.auto_fit = true;
                self.character_focus.fit = true;
            }
            if ui.small_button("−").clicked() {
                self.character_focus.auto_fit = false;
                self.character_focus.camera.zoom =
                    (self.character_focus.camera.zoom / 1.2).max(0.05);
            }
            ui.label(format!("{:.0}%", self.character_focus.camera.zoom * 100.0));
            if ui.small_button("＋").clicked() {
                self.character_focus.auto_fit = false;
                self.character_focus.camera.zoom =
                    (self.character_focus.camera.zoom * 1.2).min(6.0);
            }
            if ui.button("恢复局部布局").clicked() {
                self.character_focus.positions.clear();
                self.character_focus.layout_manual = false;
                self.character_focus.auto_fit = true;
                self.character_focus.fit = true;
            }
            ui.checkbox(&mut self.character_focus.show_results, "来源列表");
        });
        if !self.character_focus.full {
            if ui.available_width() < 620.0 {
                ui.horizontal_wrapped(|ui| {
                    ui.menu_button("关系类型、图例与筛选", |ui| {
                        self.character_context_legend(ui)
                    });
                    ui.label(theme::muted("F6 切区 · 拖动空白平移"))
                        .on_hover_text("滚轮缩放 · 双击节点看真实来源 · 圆点只创建旧式人物关系");
                });
            } else {
                self.character_context_legend(ui);
            }
        } else {
            ui.label(theme::muted("全人物图只显示旧式关系；属性引用等请切回焦点一跳。每页最多250人物，页间边不在本页绘制。"));
        }
        ui.horizontal_wrapped(|ui| {
            if let Some(from) = self.character_link.clone() {
                ui.label(format!("创建旧式关系：{from} → 点击目标人物"));
                if ui.button("取消建边").clicked() {
                    self.character_link = None;
                }
            } else if crate::theme::add_enabled(
                ui,
                self.character_editor
                    .as_ref()
                    .is_some_and(|e| e.original.is_some()),
                egui::Button::new("创建旧式人物关系"),
            )
            .clicked()
                && !self.prevent_replacing_draft("人物资料")
            {
                self.character_link = self
                    .character_editor
                    .as_ref()
                    .and_then(|e| e.original.clone());
            }
        });
    }
    fn character_context_legend(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for (i, (_, label, style)) in state::KINDS.iter().enumerate() {
                if i == 5 && !self.character_focus.mentions {
                    continue;
                }
                if ui
                    .checkbox(
                        &mut self.character_focus.filters[i],
                        format!("{label} · {style}"),
                    )
                    .changed()
                {
                    self.character_focus.fit = true;
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.character_focus.mentions, "包含文字提及（非事实）");
            ui.label(theme::muted(
                "拖动空白平移 · 滚轮缩放 · 双击节点看来源 · F6 切区",
            ));
        });
    }
    fn character_graph_projection(&mut self, ui: &mut egui::Ui) -> (Vec<Node>, Vec<Edge>) {
        if !self.character_focus.full {
            let Some(result) = &self.character_focus.result else {
                return (Vec::new(), Vec::new());
            };
            let edges: Vec<_> = result
                .records
                .iter()
                .filter(|r| self.character_focus.visible(&r.kind))
                .map(|r| Edge {
                    from: r.from_ref.clone(),
                    to: r.to_ref.clone(),
                    label: r.role.clone(),
                    kind: r.kind,
                    direction: r.direction,
                    source: r.source.clone(),
                })
                .collect();
            let included: BTreeSet<_> = edges
                .iter()
                .flat_map(|r| [state::key(&r.from), state::key(&r.to)])
                .chain(std::iter::once(state::key(&result.target)))
                .collect();
            let nodes = result
                .nodes
                .iter()
                .filter(|n| included.contains(&state::key(&n.target)))
                .map(|n| Node {
                    target: n.target.clone(),
                    name: n.display.clone(),
                    exists: n.exists,
                    source: n.file.clone().zip(n.line),
                })
                .collect();
            return (nodes, edges);
        }
        let Some(snapshot) = &self.snapshot else {
            return (Vec::new(), Vec::new());
        };
        let symbols = &snapshot.result.analysis.symbols;
        let pages = symbols.character_order.len().div_ceil(250).max(1);
        self.character_focus.full_page = self.character_focus.full_page.min(pages - 1);
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "全索引 {} 人物 · 第 {}/{} 页",
                symbols.character_order.len(),
                self.character_focus.full_page + 1,
                pages
            ));
            if crate::theme::add_enabled(
                ui,
                self.character_focus.full_page > 0,
                egui::Button::new("上一页"),
            )
            .clicked()
            {
                self.character_focus.full_page -= 1;
                self.character_focus.fit = true;
            }
            if crate::theme::add_enabled(
                ui,
                self.character_focus.full_page + 1 < pages,
                egui::Button::new("下一页"),
            )
            .clicked()
            {
                self.character_focus.full_page += 1;
                self.character_focus.fit = true;
            }
        });
        let mut ids: Vec<_> = symbols
            .character_order
            .iter()
            .skip(self.character_focus.full_page * 250)
            .take(250)
            .cloned()
            .collect();
        if let Some(id) = self
            .character_editor
            .as_ref()
            .and_then(|e| e.original.as_ref())
        {
            if !ids.contains(id) {
                if ids.len() == 250 {
                    ids.pop();
                }
                ids.push(id.clone());
            }
        }
        let nodes = ids
            .iter()
            .filter_map(|id| {
                symbols.characters.get(id).map(|p| Node {
                    target: TargetRef::new("character", id),
                    name: p.display.clone(),
                    exists: true,
                    source: Some((p.decl_file.clone(), p.decl_span.line)),
                })
            })
            .collect();
        let edges = ids
            .iter()
            .filter_map(|id| symbols.characters.get(id).map(|p| (id, p)))
            .flat_map(|(id, p)| {
                p.relations
                    .iter()
                    .filter(|r| ids.contains(&r.target))
                    .map(move |r| Edge {
                        from: TargetRef::new("character", id),
                        to: TargetRef::new("character", &r.target),
                        label: r.label.clone(),
                        kind: Kind::LegacyCharacterRelation,
                        direction: RelationDirection::Directed,
                        source: WorldContextSource {
                            file: r.file.clone(),
                            line: r.line,
                            column: None,
                            precision: WorldContextPrecision::Line,
                        },
                    })
            })
            .collect();
        (nodes, edges)
    }
    pub(in crate::app) fn character_context_graph(&mut self, ui: &mut egui::Ui) {
        let (nodes, edges) = self.character_graph_projection(ui);
        let selected = self
            .character_editor
            .as_ref()
            .and_then(|e| e.original.as_ref())
            .map(|id| TargetRef::new("character", id));
        let height = if self.character_focus.show_results && !self.character_focus.full {
            (ui.available_height() * 0.47).max(150.0)
        } else {
            ui.available_height().max(150.0)
        };
        let (canvas, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), height),
            Sense::click_and_drag(),
        );
        self.character_focus.canvas = Some(canvas);
        let keyboard = ui.interact(
            canvas,
            egui::Id::new("character-context-canvas"),
            Sense::click(),
        );
        let painter = ui.painter_at(canvas);
        painter.rect_filled(canvas, theme::shapes().document, theme::BG());
        if keyboard.has_focus() {
            painter.rect_stroke(
                canvas.shrink(2.0),
                8,
                Stroke::new(2.0_f32, ACCENT()),
                egui::StrokeKind::Inside,
            );
        }
        if nodes.is_empty() {
            painter.text(
                canvas.center(),
                egui::Align2::CENTER_CENTER,
                "选择人物，查看同一快照的关系与来源",
                egui::FontId::proportional(15.0),
                MUTED(),
            );
            return;
        }
        let keys = nodes
            .iter()
            .map(|node| state::key(&node.target))
            .collect::<Vec<_>>();
        let selected_key = selected.as_ref().map(state::key);
        self.character_focus
            .update_graph_layout(&keys, selected_key.as_deref(), canvas.size());
        let size = [canvas.width() as f64, canvas.height() as f64];
        if self.character_focus.fit {
            let points = nodes
                .iter()
                .filter_map(|n| {
                    self.character_focus
                        .positions
                        .get(&state::key(&n.target))
                        .copied()
                        .map(|point| (point, selected.as_ref() == Some(&n.target)))
                })
                .collect::<Vec<_>>();
            self.character_focus.fit_graph(&points, size);
            self.character_focus.fit = false;
        }
        if response.dragged() {
            self.character_focus.auto_fit = false;
            let d = ui.input(|i| i.pointer.delta());
            self.character_focus.camera.pan_by([d.x as f64, d.y as f64]);
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.character_focus.auto_fit = false;
                let p = ui
                    .input(|i| i.pointer.hover_pos())
                    .unwrap_or(canvas.center())
                    - canvas.min;
                self.character_focus.camera.zoom_at(
                    (scroll as f64 * 0.003).exp(),
                    [p.x as f64, p.y as f64],
                    size,
                );
            }
        }
        let zoom = self.character_focus.camera.zoom as f32;
        let rects: BTreeMap<_, _> = nodes
            .iter()
            .map(|n| {
                let p = self
                    .character_focus
                    .camera
                    .world_to_canvas(self.character_focus.positions[&state::key(&n.target)], size);
                (
                    state::key(&n.target),
                    Rect::from_center_size(
                        canvas.min + Vec2::new(p[0] as f32, p[1] as f32),
                        if selected.as_ref() == Some(&n.target) {
                            SELECTED_SIZE * zoom.max(1.0)
                        } else {
                            NODE_SIZE * zoom
                        },
                    ),
                )
            })
            .collect();
        let mut source = super::graph_edges::draw_edges(ui, &painter, canvas, &rects, &edges);
        let mut target = None;
        for node in &nodes {
            let rect = rects[&state::key(&node.target)];
            if !canvas.intersects(rect) {
                continue;
            }
            let response = ui.interact(
                rect.intersect(canvas),
                ui.id().with(("context-node", state::key(&node.target))),
                Sense::click_and_drag(),
            );
            let chosen = selected.as_ref() == Some(&node.target);
            painter.rect_filled(
                rect,
                theme::shapes().control,
                if chosen {
                    theme::SELECTION()
                } else if response.hovered() {
                    theme::HOVER()
                } else {
                    theme::DOCUMENT()
                },
            );
            painter.rect_stroke(
                rect,
                theme::shapes().control,
                Stroke::new(
                    if chosen { 2.5_f32 } else { 1.0_f32 },
                    if chosen {
                        ACCENT()
                    } else {
                        theme::CONTROL_BORDER()
                    },
                ),
                egui::StrokeKind::Inside,
            );
            if response.has_focus() {
                // 键盘焦点用独立外环与文字，不复用当前人物的内描边。
                painter.rect_stroke(
                    rect.expand(4.0),
                    theme::shapes().control,
                    Stroke::new(theme::focus_width(), theme::FOCUS()),
                    egui::StrokeKind::Outside,
                );
                let marker = egui::pos2(
                    rect.left().max(canvas.left() + 4.0),
                    (rect.bottom() + 7.0).min(canvas.bottom() - 14.0),
                );
                painter.text(
                    marker,
                    egui::Align2::LEFT_TOP,
                    "键盘焦点",
                    egui::FontId::proportional(11.0),
                    theme::FOCUS(),
                );
            }
            let node_zoom = if chosen { zoom.max(1.0) } else { zoom };
            if node_zoom > 0.22 {
                painter.text(
                    rect.center() - Vec2::new(0.0, 10.0 * node_zoom),
                    egui::Align2::CENTER_CENTER,
                    truncated(&node.name, 12),
                    egui::FontId::proportional((14.0 * node_zoom).max(9.0)),
                    TEXT(),
                );
                painter.text(
                    rect.center() + Vec2::new(0.0, 12.0 * node_zoom),
                    egui::Align2::CENTER_CENTER,
                    if chosen {
                        "当前焦点"
                    } else if !node.exists {
                        "目标失效"
                    } else {
                        node.target.kind.as_str()
                    },
                    egui::FontId::proportional((11.0 * node_zoom).max(8.0)),
                    MUTED(),
                );
            }
            if response.clicked() {
                target = Some(node.target.clone());
            }
            if response.double_clicked() {
                if let Some((file, line)) = &node.source {
                    source = Some((file.clone(), *line, 1));
                }
            }
            if response.dragged() && self.character_link.is_none() {
                self.character_focus.layout_manual = true;
                self.character_focus.auto_fit = false;
                let d = ui.input(|i| i.pointer.delta()) / zoom;
                if let Some(p) = self
                    .character_focus
                    .positions
                    .get_mut(&state::key(&node.target))
                {
                    p[0] += d.x as f64;
                    p[1] += d.y as f64;
                }
            }
            response.on_hover_text(format!(
                "{}\n{}\n{}",
                node.name,
                state::key(&node.target),
                if node.exists {
                    "双击定位声明"
                } else {
                    "目标已失效，不按同名替换"
                }
            ));
        }
        let ports = nodes
            .iter()
            .filter(|node| node.exists && node.target.kind == "character")
            .map(|node| (node.target.clone(), rects[&state::key(&node.target)]))
            .collect::<Vec<_>>();
        if let Some(port_target) = self.character_legacy_ports(ui, canvas, &ports) {
            target = Some(port_target);
        }
        if let Some((file, line, column)) = source {
            self.jump_to_file(&file, line, column);
        } else if let Some(target) = target {
            self.activate_character_context_target(target);
        }
    }
    pub(super) fn activate_character_context_target(&mut self, target: TargetRef) {
        if let Some(from) = self.character_link.clone() {
            if target.kind != "character" || target.id == from {
                return;
            }
            if self.prevent_replacing_draft("人物资料") {
                return;
            }
            self.select_character(&from);
            if let Some(mut editor) = self.character_editor.clone() {
                editor.draft.relations.push((target.id, "关联".into()));
                if self.commit(
                    "旧式人物关系已创建（已应用，未保存）",
                    |p| p.write_character(&editor.path, Some(&from), &editor.draft),
                ) {
                    self.character_link = None;
                    self.character_editor = Some(editor);
                }
            }
        } else {
            let object = self
                .snapshot
                .as_ref()
                .and_then(|s| s.result.analysis.catalog.object(&target))
                .cloned();
            if let Some(object) = object {
                self.navigate_object(&object);
            } else {
                self.message = Some("目标已不存在，未按同名替换".into());
            }
        }
    }
}
