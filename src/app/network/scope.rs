use super::*;
use worldline_core::catalog_scope::ScopeRole;

impl WorldeditApp {
    /// Returns true even when stale, so current catalog data cannot fill an old scope.
    pub(super) fn refresh_query_scope_network(&mut self) -> bool {
        if !self.query_scope_active() {
            return false;
        }
        if self
            .catalog_workbench
            .scope
            .as_ref()
            .is_some_and(|scope| scope.before_network.is_none())
        {
            let mut workbench = std::mem::take(&mut self.catalog_workbench);
            let focus = workbench
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.query().matches().first())
                .map(|item| item.target.clone());
            workbench
                .scope
                .as_mut()
                .expect("active scope")
                .preserve_network(self);
            if let Some(focus) = focus {
                self.network_state.set_focus(focus);
            }
            self.catalog_workbench = workbench;
        }
        if !self.query_scope_current() {
            self.network_state.result = None;
            return true;
        }
        let Some(target) = self.network_state.focus.clone() else {
            self.network_state.result = None;
            return true;
        };
        let mut options = self
            .network_state
            .filters
            .query_options(self.network_state.offset);
        options.scope_refs = self.network_state.scope_refs.clone();
        options.include_unscoped = self.network_state.include_unscoped;
        let offset = options.offset;
        options.offset = 0;
        let filter_key = serde_json::to_string(&(&target, &options)).unwrap_or_default();
        options.offset = offset;
        let state = &mut self.catalog_workbench;
        let Some(scope) = state.scope.as_mut() else {
            return false;
        };
        if scope
            .relation_filter_key
            .as_ref()
            .is_some_and(|old| old != &filter_key)
        {
            self.network_state.reset_query_page();
            options.offset = 0;
        }
        scope.relation_filter_key = Some(filter_key);
        let key = serde_json::to_string(&(&target, &options)).unwrap_or_default();
        if scope.relation_key.as_ref() == Some(&key) {
            return true;
        }
        let Some(snapshot) = &state.snapshot else {
            return true;
        };
        let result = snapshot.query_relations(&target, options);
        self.network_state
            .positions
            .entry(target_key(&target))
            .or_insert([0.0, 0.0]);
        for depth in 1..=2 {
            let nodes = result
                .nodes
                .iter()
                .filter(|node| node.depth == depth)
                .collect::<Vec<_>>();
            for (index, node) in nodes.iter().enumerate() {
                let angle = std::f64::consts::TAU * index as f64 / nodes.len().max(1) as f64;
                let radius = if depth == 1 { 240.0 } else { 450.0 };
                self.network_state
                    .positions
                    .entry(target_key(&node.target))
                    .or_insert([radius * angle.cos(), radius * angle.sin()]);
            }
        }
        self.network_state.result = Some(result);
        scope.relation_key = Some(key);
        true
    }
    pub(super) fn query_scope_node_caption(&self, target: &TargetRef) -> &'static str {
        match self.query_scope_role(target) {
            Some(ScopeRole::Match) => "命中",
            Some(ScopeRole::ContextOnly) => "仅关系上下文",
            Some(ScopeRole::Unresolved) => "未解析端点",
            None => "",
        }
    }
}
