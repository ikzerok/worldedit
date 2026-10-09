//! 世界关联局部网络：布局是个人状态，只有显式“保存共享布局”才写 Project。
use super::catalog::kind_label;
use super::{Tab, WorldeditApp};
use crate::theme::{self, *};
use egui::{Color32, Rect, RichText, Sense, Stroke, Vec2};
mod compact;
mod edges;
mod labels;
mod scope;
mod view;
use labels::DisplayLabels;
use worldline_core::catalog::TargetRef;
use worldline_core::graph_views::{self, GraphViewCommand};
use worldline_core::{RelationDirection, RelationQueryDirection};

const NODE_SIZE: Vec2 = Vec2::new(132.0, 44.0);

fn target_key(target: &TargetRef) -> String {
    worldline_core::graph_views::position_key(target)
}

fn node_color(kind: &str) -> Color32 {
    match kind {
        "entity" => BLUE(),
        "character" => GOLD(),
        "event" | "scene" => ACCENT(),
        _ => MUTED(),
    }
}
impl WorldeditApp {
    pub(super) fn open_network(&mut self, target: TargetRef) {
        if self.network_state.focus.is_some() {
            self.network_state.enter(target);
        } else {
            self.network_state.set_focus(target);
        }
        self.network_loaded_view = None;
        self.network_view_id.clear();
        self.network_view_title.clear();
        self.tab = Tab::Network;
    }

    pub(super) fn load_graph_view(&mut self, id: &str) {
        let Some(view) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.graph_index.views.get(id))
            .cloned()
        else {
            self.io_error = Some("共享布局已不存在，请刷新后重试".into());
            return;
        };
        self.network_state.load(&view.draft);
        self.network_view_id = view.draft.id.clone();
        self.network_view_title = view.draft.title.clone();
        self.network_loaded_view = Some(view.draft.id.clone());
        self.tab = Tab::Network;
    }

    fn save_graph_view(&mut self) {
        let id = self.network_view_id.trim().to_owned();
        let title = self.network_view_title.trim().to_owned();
        if id.is_empty() || title.is_empty() {
            self.io_error = Some("保存共享布局需要填写稳定 ID 和标题".into());
            return;
        }
        let Some(draft) = self.network_state.draft(id.clone(), title) else {
            self.io_error = Some("请先选择网络中心对象".into());
            return;
        };
        let command = GraphViewCommand {
            expected_revision: self.map_revision,
            expected_baseline: self.project.content_baseline(),
            original: self.network_loaded_view.clone(),
            draft,
        };
        let before = self.project.clone();
        let Some(content) = self.snapshot.as_ref().map(|snapshot| &snapshot.result) else {
            return;
        };
        match graph_views::apply_with_content(
            &mut self.project,
            &mut self.map_revision,
            command,
            content,
        ) {
            Ok(_) => {
                self.remember(before);
                self.refresh_presentation_after_map_command();
                self.network_loaded_view = Some(id);
                self.message = Some("共享网络布局已更新；保存全部可写入作品目录".into());
                self.io_error = None;
            }
            Err(error) => self.io_error = Some(error),
        }
    }

    fn network_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if crate::theme::add_enabled(
                ui,
                self.network_state.can_back(),
                egui::Button::new("← 返回"),
            )
            .clicked()
            {
                self.network_state.back();
            }
            ui.label(RichText::new("世界关联").strong().size(19.0));
            egui::ComboBox::from_id_salt("network-depth")
                .selected_text(format!("{} 层", self.network_state.filters.depth))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.network_state.filters.depth, 1, "1 层");
                    ui.selectable_value(&mut self.network_state.filters.depth, 2, "2 层");
                });
            egui::ComboBox::from_id_salt("network-direction")
                .selected_text(match self.network_state.filters.direction {
                    RelationQueryDirection::Both => "双向读取",
                    RelationQueryDirection::Outgoing => "只看出向",
                    RelationQueryDirection::Incoming => "只看入向",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.network_state.filters.direction,
                        RelationQueryDirection::Both,
                        "双向读取",
                    );
                    ui.selectable_value(
                        &mut self.network_state.filters.direction,
                        RelationQueryDirection::Outgoing,
                        "只看出向",
                    );
                    ui.selectable_value(
                        &mut self.network_state.filters.direction,
                        RelationQueryDirection::Incoming,
                        "只看入向",
                    );
                });
        });
    }

    fn network_filters(&mut self, ui: &mut egui::Ui) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let relation_types = snapshot.result.analysis.catalog.relation_types.clone();
        ui.menu_button("关系类型筛选", |ui| {
            if ui.button("清除筛选").clicked() {
                self.network_state.filters.relation_types.clear();
            }
            for kind in relation_types.values() {
                let mut selected = self.network_state.filters.relation_types.contains(&kind.id);
                if ui
                    .checkbox(&mut selected, format!("{} · {}", kind.display, kind.id))
                    .changed()
                {
                    if selected {
                        self.network_state
                            .filters
                            .relation_types
                            .push(kind.id.clone());
                        self.network_state.filters.relation_types.sort();
                        self.network_state.filters.relation_types.dedup();
                    } else {
                        self.network_state
                            .filters
                            .relation_types
                            .retain(|id| id != &kind.id);
                    }
                }
            }
        });
        if ui.button("自动布局").clicked() {
            self.network_state.reset_layout();
        }
        if ui.button("显示全部隐藏关系").clicked() {
            self.network_state.show_all();
        }
    }

    fn network_saved_views(&mut self, ui: &mut egui::Ui) {
        let views = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .graph_index
                    .views
                    .values()
                    .map(|view| (view.draft.id.clone(), view.draft.title.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        ui.separator();
        ui.label(RichText::new("共享布局").strong());
        egui::ComboBox::from_id_salt("saved-network-view")
            .selected_text(
                self.network_loaded_view
                    .as_deref()
                    .unwrap_or("选择已保存布局"),
            )
            .show_ui(ui, |ui| {
                for (id, title) in &views {
                    if ui.button(format!("{title} · {id}")).clicked() {
                        self.load_graph_view(id);
                        ui.close();
                    }
                }
            });
        ui.add(
            egui::TextEdit::singleline(&mut self.network_view_id)
                .hint_text("布局 ID，例如 lighthouse_relations"),
        );
        ui.add(egui::TextEdit::singleline(&mut self.network_view_title).hint_text("共享布局标题"));
        if ui.add(theme::primary("保存共享布局")).clicked() {
            self.save_graph_view();
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("新建展示预设…").clicked() {
                self.open_preset_editor(None);
            }
            if let Some(snapshot) = &self.snapshot {
                let presets = snapshot
                    .preset_index
                    .presets
                    .values()
                    .map(|preset| (preset.draft.id.clone(), preset.draft.title.clone()))
                    .collect::<Vec<_>>();
                ui.menu_button("应用预设", |ui| {
                    for (id, title) in presets {
                        if ui.button(format!("{title} · {id}")).clicked() {
                            self.apply_presentation_preset(&id);
                            ui.close();
                        }
                    }
                });
            }
        });
        ui.label(theme::muted(
            "拖动、缩放、隐藏关系默认仅属于当前个人浏览；上方按钮才写共享文档。",
        ));
    }

    fn network_canvas(&mut self, ui: &mut egui::Ui) {
        let scoped = self.refresh_query_scope_network();
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let catalog = &snapshot.result.analysis.catalog;
        if !scoped {
            self.network_state.refresh(catalog, self.version);
        }
        let result = self.network_state.result.clone();
        let available = ui.available_size();
        let desired = if scoped && (available.x < 600.0 || available.y < 280.0) {
            available.max(Vec2::splat(1.0))
        } else {
            available.max(Vec2::new(320.0, 280.0))
        };
        let (response, painter) = ui.allocate_painter(desired, Sense::drag());
        let rect = response.rect;
        #[cfg(test)]
        ui.ctx().data_mut(|data| {
            data.insert_temp(
                egui::Id::new("catalog-scope-network-clip"),
                rect.intersect(painter.clip_rect()),
            )
        });
        let size = [f64::from(rect.width()), f64::from(rect.height())];
        if response.double_clicked() {
            let points = self
                .network_state
                .positions
                .values()
                .copied()
                .collect::<Vec<_>>();
            self.network_state.camera.fit(&points, size);
        }

        if response.dragged() && self.network_state.dragging().is_none() {
            let delta = response.drag_delta();
            self.network_state
                .camera
                .pan_by([f64::from(delta.x), f64::from(delta.y)]);
        }
        if response.hovered() {
            let scroll = ui.input(|input| input.raw_scroll_delta.y);
            if scroll.abs() > f32::EPSILON {
                if let Some(pointer) = ui.input(|input| input.pointer.hover_pos()) {
                    let local = pointer - rect.min;
                    self.network_state.camera.zoom_at(
                        (f64::from(scroll) / 480.0).exp(),
                        [f64::from(local.x), f64::from(local.y)],
                        size,
                    );
                }
            }
        }
        let Some(result) = result else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "从资料页选择“查看关联”，或使用右侧“搜索中心对象”。",
                egui::FontId::proportional(16.0),
                MUTED(),
            );
            return;
        };
        let frozen = self.catalog_workbench.snapshot.clone().filter(|_| scoped);
        let labels = frozen
            .as_deref()
            .map_or_else(|| DisplayLabels::new(catalog), DisplayLabels::from_scope);
        for edge in &result.edges {
            if self.network_state.hidden.contains(&edge.id) {
                continue;
            }
            let Some(from) = self
                .network_state
                .positions
                .get(&target_key(&edge.from_ref))
            else {
                continue;
            };
            let Some(to) = self.network_state.positions.get(&target_key(&edge.to_ref)) else {
                continue;
            };
            let a = self.network_state.camera.world_to_canvas(*from, size);
            let b = self.network_state.camera.world_to_canvas(*to, size);
            let a = rect.min + Vec2::new(a[0] as f32, a[1] as f32);
            let b = rect.min + Vec2::new(b[0] as f32, b[1] as f32);
            painter.line_segment([a, b], Stroke::new(1.4_f32, MUTED()));
            painter.text(
                a.lerp(b, 0.5),
                egui::Align2::CENTER_CENTER,
                &edge.label,
                egui::FontId::proportional(12.0),
                TEXT(),
            );
        }

        for node in result.nodes {
            let key = target_key(&node.target);
            let Some(position) = self.network_state.positions.get(&key).copied() else {
                continue;
            };
            let point = self.network_state.camera.world_to_canvas(position, size);
            let center = rect.min + Vec2::new(point[0] as f32, point[1] as f32);
            let node_rect = Rect::from_center_size(center, NODE_SIZE);
            let node_response = ui.interact(
                node_rect,
                ui.id().with(("network-node", &key)),
                Sense::click_and_drag(),
            );
            if node_response.drag_started() {
                self.network_state.begin_drag(&key);
            }
            if node_response.dragged() {
                if let Some(pointer) = node_response.interact_pointer_pos() {
                    let local = pointer - rect.min;
                    let world = self
                        .network_state
                        .camera
                        .canvas_to_world([f64::from(local.x), f64::from(local.y)], size);
                    self.network_state.drag_to(world);
                }
            }
            if node_response.drag_stopped() {
                self.network_state.finish_drag();
            }
            if node_response.clicked() {
                self.network_selected = Some(node.target.clone());
            }
            if node_response.double_clicked() {
                self.network_state.enter(node.target.clone());
            }
            painter.rect_filled(
                node_rect,
                theme::shapes().control,
                if self.network_selected.as_ref() == Some(&node.target) {
                    theme::SELECTION()
                } else if node_response.hovered() {
                    theme::HOVER()
                } else {
                    theme::DOCUMENT()
                },
            );
            painter.rect_stroke(
                node_rect,
                theme::shapes().control,
                Stroke::new(
                    if self.network_selected.as_ref() == Some(&node.target) {
                        2.8_f32
                    } else {
                        1.4_f32
                    },
                    node_color(&node.target.kind),
                ),
                egui::StrokeKind::Inside,
            );
            node_response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    true,
                    self.network_selected.as_ref() == Some(&node.target),
                    labels.get(&node.target),
                )
            });
            if node_response.has_focus() {
                painter.rect_stroke(
                    node_rect.expand(3.0),
                    theme::shapes().control,
                    Stroke::new(theme::focus_width(), theme::FOCUS()),
                    egui::StrokeKind::Outside,
                );
            }
            node_response.on_hover_text(format!(
                "{} · {}:{}",
                labels.get(&node.target),
                node.target.kind,
                node.target.id
            ));
            let node_painter = painter.with_clip_rect(node_rect.shrink(4.0).intersect(rect));
            node_painter.text(
                node_rect.center() - Vec2::new(0.0, 7.0),
                egui::Align2::CENTER_CENTER,
                labels.get(&node.target),
                egui::FontId::proportional(14.0),
                TEXT(),
            );
            node_painter.text(
                node_rect.center() + Vec2::new(0.0, 10.0),
                egui::Align2::CENTER_CENTER,
                format!(
                    "{} · {} {}",
                    kind_label(&node.target.kind),
                    node.target.id,
                    self.query_scope_node_caption(&node.target)
                ),
                egui::FontId::proportional(10.5),
                MUTED(),
            );
        }

        if result.truncated {
            painter.text(
                rect.left_bottom() + Vec2::new(12.0, -12.0),
                egui::Align2::LEFT_BOTTOM,
                "当前页已达上限，可用“下一页”继续读取；原关系未被删除。",
                egui::FontId::proportional(12.0),
                GOLD(),
            );
        }
    }
}
