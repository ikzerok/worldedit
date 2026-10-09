use super::*;
impl WorkbenchState {
    pub(super) fn apply_sort(
        &mut self,
        app: &WorldeditApp,
        ctx: &egui::Context,
        sort: Option<CatalogQuerySort>,
    ) {
        if self.query.sort == sort {
            return;
        }
        if let Some(mut running) = self.running.take() {
            running.cancel();
        }
        self.query.set_sort(sort);
        self.page = None;
        self.error = None;
        self.run_query(app, ctx);
    }
    pub(super) fn run_query(&mut self, app: &WorldeditApp, ctx: &egui::Context) {
        if let Some(mut running) = self.running.take() {
            running.cancel();
        }
        self.page = None;
        self.error = None;
        self.status = None;
        self.queued = false;
        match job::RunningQuery::start(app, self.query.clone(), current_options(self), ctx) {
            Ok(job) => {
                self.running = Some(job);
                ctx.request_repaint();
            }
            Err(error) if error == job::BUSY => {
                self.queued = true;
                self.status = Some(error);
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
            Err(error) => self.error = Some(error),
        }
    }
    pub(super) fn poll_query(&mut self, app: &WorldeditApp, ctx: &egui::Context) {
        let Some(running) = self.running.as_mut() else {
            if self.queued {
                self.run_query(app, ctx);
            }
            return;
        };
        let Some(result) = running.poll() else {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            return;
        };
        let running = self.running.take().expect("polled query exists");
        if running.cancel_requested {
            self.page = None;
            self.error = Some("查询已取消，未返回部分结果。".into());
            return;
        }
        if running.query != self.query
            || running.options.max_candidates != current_options(self).max_candidates
            || running.key != (app.version, app.map_revision)
            || running.observation != app.project.catalog_scope_observation_key()
        {
            self.page = None;
            self.error = Some("筛选条件或工程已变化；丢弃旧结果，请重新运行。".into());
            return;
        }
        match result {
            Ok((baseline, snapshot)) if baseline == app.project.content_baseline() => {
                if let Err(error) =
                    snapshot.validate_for(&self.query, &baseline, self.max_candidates)
                {
                    self.error = Some(error.to_string());
                    return;
                }
                let previous = self
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.query().total());
                self.page = snapshot.query().page(0, self.page_size).ok();
                self.snapshot = Some(std::sync::Arc::new(snapshot));
                self.snapshot_key = Some(running.key);
                self.snapshot_observation = Some(running.observation.clone());
                self.snapshot_query = Some(self.query.clone());
                if let Some(scope) = self.scope.as_mut() {
                    scope.relation_key = None;
                    scope.relation_filter_key = None;
                    scope.map_cache = None;
                    scope.inspection_offset = 0;
                    scope.placement_offsets.clear();
                    scope.unresolved_offset = 0;
                }
                self.status = previous.map(|old| {
                    format!(
                        "范围已刷新：原 {old} 个命中，当前 {} 个命中",
                        self.page.as_ref().map_or(0, |page| page.total)
                    )
                });
                self.error = None;
            }
            Ok(_) => {
                self.page = None;
                self.error = Some("Project 缓冲已变化；旧快照已丢弃，请重新查询。".into());
            }
            Err(error) => {
                self.page = None;
                self.error = Some(error);
            }
        }
    }
    pub(super) fn current_snapshot(
        &self,
        app: &WorldeditApp,
    ) -> Option<&worldline_core::catalog_scope::CatalogScopeSnapshot> {
        if self.snapshot_key != Some((app.version, app.map_revision))
            || self.snapshot_query.as_ref() != Some(&self.query)
            || self.snapshot_observation.as_deref()
                != Some(app.project.catalog_scope_observation_key().as_str())
        {
            return None;
        }
        self.snapshot
            .as_deref()
            .filter(|snapshot| snapshot.query().max_candidates == self.max_candidates)
    }
    pub(super) fn next_page(&mut self, app: &WorldeditApp) {
        let Some(cursor) = self.page.as_ref().and_then(|page| page.next.clone()) else {
            return;
        };
        if let Some(snapshot) = self.current_snapshot(app) {
            match snapshot.query().continue_page(&cursor) {
                Ok(page) => {
                    self.page = Some(page);
                    self.error = None;
                }
                Err(error) => {
                    self.page = None;
                    self.error = Some(error.to_string());
                }
            }
        } else {
            self.page = None;
            self.error = Some("分页快照已过期；请重新运行查询。".into());
        }
    }
    pub(super) fn previous_page(&mut self, app: &WorldeditApp, offset: usize) {
        if let Some(snapshot) = self.current_snapshot(app) {
            match snapshot.query().page(offset, self.page_size) {
                Ok(page) => {
                    self.page = Some(page);
                    self.error = None;
                }
                Err(error) => {
                    self.page = None;
                    self.error = Some(error.to_string());
                }
            }
        } else {
            self.page = None;
            self.error = Some("分页快照已过期；请重新运行查询。".into());
        }
    }
    pub(super) fn cancel_query(&mut self) {
        self.queued = false;
        if self.running.is_none() {
            self.error = Some("查询已取消，未返回部分结果。".into());
        }
        if let Some(running) = &mut self.running {
            running.cancel();
        }
    }
}
