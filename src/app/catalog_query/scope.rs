use super::*;
use crate::app::{network_state::NetworkState, personal::Location, Tab};
use worldline_core::catalog_scope::{ScopePlacement, ScopeRole};

pub(in crate::app) struct ActiveScope {
    pub before: Location,
    pub before_map: Option<crate::app::maps::AuthorMapPosition>,
    pub before_network: Option<NetworkState>,
    pub before_network_labels: Option<(Option<TargetRef>, Option<String>, String, String)>,
    pub last_view: Option<Location>,
    pub relation_key: Option<String>,
    pub relation_filter_key: Option<String>,
    pub map_cache: Option<(String, std::sync::Arc<Vec<ScopePlacement>>, usize)>,
    pub inspection_offset: usize,
    pub placement_offsets: BTreeMap<TargetRef, usize>,
    pub unresolved_offset: usize,
}
impl ActiveScope {
    pub(in crate::app) fn preserve_network(&mut self, app: &mut WorldeditApp) {
        if self.before_network.is_none() {
            self.before_network = Some(std::mem::take(&mut app.network_state));
            self.before_network_labels = Some((
                app.network_selected.take(),
                app.network_loaded_view.take(),
                std::mem::take(&mut app.network_view_id),
                std::mem::take(&mut app.network_view_title),
            ));
        }
    }
}
impl WorkbenchState {
    pub(super) fn enter_scope(
        &mut self,
        app: &mut WorldeditApp,
        ctx: &egui::Context,
        network: bool,
    ) {
        if !self.scope_navigation_ready(app) {
            return;
        }
        if self.scope.is_none() {
            self.scope = Some(ActiveScope {
                before: app.author_location(Some(ctx)),
                before_map: app.capture_query_map_position(),
                before_network: None,
                before_network_labels: None,
                last_view: None,
                relation_key: None,
                relation_filter_key: None,
                map_cache: None,
                placement_offsets: BTreeMap::new(),
                unresolved_offset: 0,
                inspection_offset: self.page.as_ref().map_or(0, |page| page.offset),
            });
        }
        if network {
            let focus = self
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.query().matches().first())
                .map(|item| item.target.clone());
            if let Some(focus) = focus {
                self.scope_network(app, focus);
            } else {
                if let Some(scope) = self.scope.as_mut() {
                    scope.preserve_network(app);
                }
                app.tab = Tab::Network;
                app.network_state.result = None;
            }
        } else {
            app.switch_tab(Tab::Map);
        }
        self.open = true;
    }
    pub(in crate::app) fn scope_navigation_ready(&self, app: &mut WorldeditApp) -> bool {
        if app.ime_composing || app.ime_source_draft.is_some() {
            app.message = Some("输入法组合或未提交源码输入仍待处理；当前输入已保留".into());
            return false;
        }
        if app.map_navigation_blocked() {
            return false;
        }
        if self
            .current_snapshot(app)
            .is_none_or(|snapshot| snapshot.query().snapshot != app.project.content_baseline())
        {
            app.message = Some("查询范围已过期；请保留当前输入，返回查询后显式刷新".into());
            return false;
        }
        true
    }
    pub(super) fn scope_network(&mut self, app: &mut WorldeditApp, target: TargetRef) {
        if !self.scope_navigation_ready(app) {
            return;
        }
        let Some(scope) = self.scope.as_mut() else {
            return;
        };
        scope.preserve_network(app);
        scope.relation_key = None;
        app.open_network(target);
    }
    pub(super) fn return_query(&mut self, app: &mut WorldeditApp, ctx: &egui::Context) {
        if app.ime_composing || app.ime_source_draft.is_some() || app.map_navigation_blocked() {
            return;
        }
        if let Some(scope) = self.scope.as_mut() {
            if matches!(app.tab, Tab::Map | Tab::Network) {
                scope.last_view = Some(app.author_location(Some(ctx)));
            }
        }
        self.open = true;
        app.switch_tab(Tab::Catalog);
    }
    pub(super) fn return_inspection(&mut self, app: &mut WorldeditApp, ctx: &egui::Context) {
        if !self.scope_navigation_ready(app) {
            return;
        }
        if let Some(location) = self
            .scope
            .as_ref()
            .and_then(|scope| scope.last_view.clone())
        {
            app.restore_author_location(location, ctx);
        } else {
            app.switch_tab(Tab::Map);
        }
    }
    pub(super) fn clear_scope(&mut self, app: &mut WorldeditApp, ctx: &egui::Context) {
        if app.ime_composing || app.ime_source_draft.is_some() || app.map_navigation_blocked() {
            app.message = Some("当前输入仍待处理；临时范围与返回位置已保留".into());
            return;
        }
        let Some(mut scope) = self.scope.take() else {
            return;
        };
        let map_restored = scope
            .before_map
            .as_ref()
            .is_none_or(|map| app.restore_map_position(Some(map)));
        if let Some(network) = scope.before_network.take() {
            app.network_state = network;
        }
        if let Some((selected, loaded, id, title)) = scope.before_network_labels.take() {
            app.network_selected = selected;
            app.network_loaded_view = loaded;
            app.network_view_id = id;
            app.network_view_title = title;
        }
        app.restore_author_location(scope.before, ctx);
        if map_restored {
            app.message = Some("已清除临时查询范围并恢复进入前的个人视图；作品内容未修改".into());
        }
    }
}
impl WorldeditApp {
    pub(in crate::app) fn query_scope_active(&self) -> bool {
        self.catalog_workbench.scope.is_some()
    }
    pub(in crate::app) fn query_scope_current(&self) -> bool {
        self.catalog_workbench.current_snapshot(self).is_some()
    }
    pub(in crate::app) fn query_scope_role(&self, target: &TargetRef) -> Option<ScopeRole> {
        self.query_scope_active()
            .then(|| {
                self.catalog_workbench
                    .current_snapshot(self)
                    .map(|snapshot| snapshot.role(target))
            })
            .flatten()
    }
    pub(in crate::app) fn query_scope_panel(&mut self, ui: &mut egui::Ui) {
        if !self.query_scope_active() {
            return;
        }
        let mut workbench = std::mem::take(&mut self.catalog_workbench);
        workbench.render_scope(self, ui);
        self.catalog_workbench = workbench;
    }
    pub(in crate::app) fn catalog_scope_return_bar(&mut self, ctx: &egui::Context) {
        let mut workbench = std::mem::take(&mut self.catalog_workbench);
        workbench.poll_query(self, ctx);
        if workbench.scope.is_some() {
            let current = workbench.current_snapshot(self).is_some();
            let mut action = 0;
            let compact =
                ctx.available_rect().width() < 640.0 || ctx.available_rect().height() < 320.0;
            let panel = egui::TopBottomPanel::top("catalog-scope-return");
            let panel = if compact {
                panel.frame(crate::theme::panel().inner_margin(egui::Margin::symmetric(6, 2)))
            } else {
                panel
            };
            panel.show(ctx, |ui| {
                if compact {
                    ui.horizontal(|ui| {
                        let total = workbench
                            .snapshot
                            .as_ref()
                            .map_or(0, |snapshot| snapshot.query().total());
                        ui.strong(format!(
                            "范围 · {total}{}",
                            if current { "" } else { " · 已过期" }
                        ));
                        if workbench.running.is_some() || workbench.queued {
                            ui.spinner();
                        }
                        ui.menu_button("范围操作", |ui| {
                            scope_actions(ui, &mut action, true);
                            if (workbench.running.is_some() || workbench.queued)
                                && ui.button("取消刷新").clicked()
                            {
                                action = 5;
                                ui.close();
                            }
                        });
                    });
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong("临时查询范围");
                        if workbench.running.is_some() || workbench.queued {
                            ui.spinner();
                            ui.label("后台刷新中，保留上一份范围");
                            if ui.button("取消刷新").clicked() {
                                action = 5;
                            }
                        }
                        if let Some(snapshot) = &workbench.snapshot {
                            ui.label(format!(
                                "{} 个命中{}",
                                snapshot.query().total(),
                                if current { "" } else { " · 已过期" }
                            ));
                        }
                        scope_actions(ui, &mut action, false);
                    });
                }
            });
            match action {
                1 => workbench.return_query(self, ctx),
                2 => workbench.return_inspection(self, ctx),
                3 => workbench.run_query(self, ctx),
                4 => workbench.clear_scope(self, ctx),
                5 => workbench.cancel_query(),
                _ => {}
            }
        }
        self.catalog_workbench = workbench;
    }
    pub(in crate::app) fn query_scope_map_overlays(
        &mut self,
    ) -> (std::sync::Arc<Vec<ScopePlacement>>, usize) {
        if !self.query_scope_active() || !self.query_scope_current() {
            return (std::sync::Arc::default(), 0);
        }
        let map_id = self.map_selection.clone().unwrap_or_default();
        let workbench = &mut self.catalog_workbench;
        let scope = workbench.scope.as_mut().expect("active scope");
        if scope
            .map_cache
            .as_ref()
            .is_none_or(|(old, _, _)| old != &map_id)
        {
            let snapshot = workbench.snapshot.as_ref().expect("current snapshot");
            let all = snapshot
                .placements()
                .iter()
                .filter(|p| p.map_id == map_id && p.role == ScopeRole::Match);
            let total = all.clone().count();
            let overlays = all.take(500).cloned().collect();
            scope.map_cache = Some((map_id, std::sync::Arc::new(overlays), total));
        }
        let (_, placements, total) = scope.map_cache.as_ref().expect("map cached");
        (placements.clone(), *total)
    }
}

fn scope_actions(ui: &mut egui::Ui, action: &mut u8, close_menu: bool) {
    for (value, label) in [
        (1, "返回同一查询"),
        (2, "返回巡检位置"),
        (3, "刷新范围"),
        (4, "清除范围并恢复"),
    ] {
        if ui.button(label).clicked() {
            *action = value;
            if close_menu {
                ui.close();
            }
        }
    }
}
