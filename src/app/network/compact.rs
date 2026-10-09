//! 临时范围的窄矮布局：图与材料同宽切换，继续调用原网络动作。
use super::*;
impl WorldeditApp {
    pub(super) fn compact_scope_network(&mut self, ctx: &egui::Context) {
        let id = egui::Id::new("catalog-scope-network-pane");
        let mut pane = ctx.data(|data| data.get_temp::<u8>(id)).unwrap_or(0);
        egui::CentralPanel::default()
            .frame(
                theme::panel()
                    .fill(theme::canvas_background())
                    .inner_margin(egui::Margin::symmetric(6, 2)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if pane >= 3 {
                        if ui.button("返回关系图").clicked() {
                            pane = 0;
                        }
                        ui.strong(if pane == 4 {
                            "完整网络检查器"
                        } else {
                            "关系操作"
                        });
                        return;
                    }
                    ui.selectable_value(&mut pane, 0, "关系图");
                    ui.selectable_value(&mut pane, 1, "范围列表");
                    ui.selectable_value(&mut pane, 2, "关系上下文");
                    if ui.button("关系操作").clicked() {
                        pane = 3;
                    }
                });
                ui.separator();
                if pane == 0 {
                    self.network_canvas(ui);
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt(("compact-network-materials", pane))
                        .max_height(ui.available_height().max(0.0))
                        .min_scrolled_height(0.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            #[cfg(test)]
                            ui.ctx().data_mut(|data| {
                                data.insert_temp(
                                    egui::Id::new("catalog-scope-material-clip"),
                                    ui.clip_rect(),
                                )
                            });
                            if pane == 1 {
                                self.query_scope_panel(ui);
                            } else if pane == 2 {
                                self.compact_network_context(ui);
                            } else if pane == 3 {
                                self.network_toolbar(ui);
                                self.network_page_controls(ui);
                                ui.separator();
                                self.network_filters(ui);
                                if ui.button("完整网络检查器").clicked() {
                                    pane = 4;
                                }
                            } else {
                                self.network_inspector_contents(ui, false);
                            }
                        });
                }
            });
        ctx.data_mut(|data| data.insert_temp(id, pane));
    }
    fn compact_network_context(&mut self, ui: &mut egui::Ui) {
        if !self.query_scope_current() {
            ui.label(theme::muted("范围已过期，请从范围操作刷新"));
            return;
        }
        let Some(scope) = self.catalog_workbench.snapshot.clone() else {
            return;
        };
        let nodes = self
            .network_state
            .result
            .as_ref()
            .map(|result| result.nodes.clone())
            .unwrap_or_default();
        let mut read = None;
        let mut source = None;
        let mut focus = None;
        for node in nodes {
            ui.push_id((&node.target.kind, &node.target.id), |ui| {
                let object = scope.object(&node.target);
                ui.label(format!(
                    "{} · {}",
                    object.map_or(node.target.id.as_str(), |object| object.display.as_str()),
                    self.query_scope_node_caption(&node.target)
                ));
                ui.label(theme::muted(format!(
                    "{}:{}",
                    node.target.kind, node.target.id
                )));
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("阅读资料").clicked() {
                        read = Some(node.target.clone());
                    }
                    if ui.small_button("作为中心").clicked() {
                        focus = Some(node.target.clone());
                    }
                    if let Some(origin) = object.and_then(|object| object.source.as_ref()) {
                        if ui.small_button("打开来源").clicked() {
                            source = Some(origin.clone());
                        }
                    }
                });
                ui.separator();
            });
        }
        if read.is_some() || source.is_some() || focus.is_some() {
            let workbench = std::mem::take(&mut self.catalog_workbench);
            let ready = workbench.scope_navigation_ready(self);
            self.catalog_workbench = workbench;
            if !ready {
                return;
            }
            let location = self.author_location(Some(ui.ctx()));
            if let Some(active) = self.catalog_workbench.scope.as_mut() {
                active.last_view = Some(location);
            }
        }
        if let Some(target) = read {
            self.open_reading(target);
        }
        if let Some(origin) = source {
            self.jump_to_file(&origin.file, origin.line, 1);
        }
        if let Some(target) = focus {
            self.network_state.enter(target);
        }
    }
}
