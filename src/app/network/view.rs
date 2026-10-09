use super::*;
impl WorldeditApp {
    pub(in crate::app) fn network_tab(&mut self, ctx: &egui::Context) {
        self.refresh_query_scope_network();
        if self.query_scope_active()
            && (ctx.available_rect().width() < 680.0 || ctx.available_rect().height() < 280.0)
        {
            self.compact_scope_network(ctx);
            return;
        }
        let topic_active = !self.query_scope_active() && self.topic_view_active(ctx);
        egui::SidePanel::right("network-inspector")
            .default_width(theme::INSPECTOR_WIDTH)
            .width_range(250.0..=420.0)
            .frame(theme::panel())
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("network-inspector-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.network_inspector_contents(ui, topic_active);
                    });
            });
        egui::CentralPanel::default()
            .frame(theme::panel().fill(theme::canvas_background()))
            .show(ctx, |ui| {
                if self.query_scope_active() || !self.topic_views(ui) {
                    self.network_toolbar(ui);
                    self.network_page_controls(ui);
                    ui.separator();
                    self.network_canvas(ui);
                }
            });
    }
    pub(super) fn network_inspector_contents(&mut self, ui: &mut egui::Ui, topic_active: bool) {
        self.query_scope_panel(ui);
        if topic_active {
            ui.heading("专题视图");
            ui.label(theme::muted("专题映射与筛选仅用于只读 core 查询。"));
        } else {
            ui.heading("局部关系网络");
            if let Some(catalog) = self
                .snapshot
                .as_ref()
                .map(|snapshot| &snapshot.result.analysis.catalog)
            {
                let mut center = self.network_state.focus.clone();
                if crate::app::object_picker::object_picker(
                    ui,
                    "network-focus-picker",
                    "搜索中心对象",
                    &mut center,
                    catalog,
                    &[],
                ) {
                    if let Some(target) = center {
                        self.open_network(target);
                    } else {
                        self.network_state = Default::default();
                        self.network_selected = None;
                        self.network_loaded_view = None;
                    }
                }
            }
            self.network_filters(ui);
            if let Some(target) = self.network_selected.clone() {
                ui.separator();
                ui.label(RichText::new("选中对象").strong());
                if let Some(snapshot) = &self.snapshot {
                    if let Some(object) = snapshot.result.analysis.catalog.object(&target) {
                        ui.label(&object.display);
                        theme::technical_value(ui, kind_label(&target.kind), &target.id);
                    }
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button("阅读资料").clicked() {
                        self.open_reading(target.clone());
                    }
                    if ui.button("作为中心").clicked() {
                        self.network_state.enter(target.clone());
                    }
                    if ui.button("创建关系").clicked() {
                        self.edit_relation(None, Some(target.clone()));
                    }
                });
            }
            self.network_edge_list(ui);
            if self.query_scope_active() {
                ui.label(theme::muted(
                    "临时范围不会写入共享布局；清除范围后可保存普通视图。",
                ));
            } else {
                self.network_saved_views(ui);
            }
        }
    }
    pub(super) fn network_page_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            if crate::theme::add_enabled(
                ui,
                self.network_state.can_previous(),
                egui::Button::new("上一页"),
            )
            .clicked()
            {
                self.network_state.previous_page();
            }
            if crate::theme::add_enabled(
                ui,
                self.network_state
                    .result
                    .as_ref()
                    .is_some_and(|result| result.continuation.is_some()),
                egui::Button::new("下一页"),
            )
            .clicked()
            {
                self.network_state.next_page();
            }
            ui.label(theme::muted(format!(
                "第 {} 页 · 最多 250 节点 / 500 关系",
                self.network_state.offset / 500 + 1
            )));
        });
    }
}
