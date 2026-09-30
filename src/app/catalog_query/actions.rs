use super::*;
use worldline_core::queries::QueryError;
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
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(running) = self.running.take() {
            running
                .cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.query.set_sort(sort);
        self.page = None;
        self.error = None;
        self.run_query(app, ctx);
    }

    pub(super) fn run_query(&mut self, app: &WorldeditApp, _ctx: &egui::Context) {
        self.page = None;
        self.error = None;
        let options = CatalogQueryOptions {
            offset: 0,
            page_size: self.page_size.clamp(1, MAX_CATALOG_QUERY_PAGE_SIZE),
            max_candidates: self.max_candidates.clamp(1, MAX_CATALOG_QUERY_CANDIDATES),
        };
        let query = self.query.clone();
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.running.is_some() {
                return;
            }
            let project = app.project.clone();
            let snapshot = project.content_baseline();
            let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let cancel_thread = cancel.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            let query_thread = query.clone();
            std::thread::spawn(move || {
                let result = project
                    .query_catalog_cancellable(&query_thread, options, || {
                        cancel_thread.load(std::sync::atomic::Ordering::Relaxed)
                    })
                    .map_err(|error| error.to_string());
                let _ = sender.send((snapshot, result));
            });
            self.running = Some(RunningQuery {
                cancel,
                receiver,
                query,
                options,
                cancel_requested: false,
            });
            _ctx.request_repaint();
        }
        #[cfg(target_arch = "wasm32")]
        {
            let snapshot = app.project.content_baseline();
            match app
                .project
                .query_catalog_cancellable(&query, options, || false)
            {
                Ok(page) => {
                    self.page = Some(page);
                    self.error = None;
                }
                Err(error) => {
                    self.error = Some(error.to_string());
                }
            }
            if snapshot != app.project.content_baseline() {
                self.page = None;
                self.error = Some("Project 在查询期间发生变化；请重新运行查询。".into());
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn poll_query(&mut self, app: &WorldeditApp, ctx: &egui::Context) {
        use std::sync::mpsc::TryRecvError;
        let result = match self.running.as_ref() {
            Some(running) => match running.receiver.try_recv() {
                Ok(result) => Some(Some(result)),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(None),
            },
            None => return,
        };
        match result {
            None => ctx.request_repaint_after(std::time::Duration::from_millis(16)),
            Some(result) => {
                let submitted_query = self
                    .running
                    .as_ref()
                    .map(|running| running.query.clone())
                    .unwrap_or_default();
                let submitted_options = self.running.as_ref().map(|running| running.options);
                let cancel_requested = self
                    .running
                    .as_ref()
                    .is_some_and(|running| running.cancel_requested);
                self.running = None;
                match result {
                    Some((snapshot, Ok(page))) => {
                        if cancel_requested {
                            self.page = None;
                            self.error = Some("查询已取消，未返回部分结果。".into());
                        } else if submitted_query == self.query
                            && submitted_options == Some(current_options(self))
                            && snapshot == app.project.content_baseline()
                        {
                            self.page = Some(page);
                            self.error = None;
                        } else {
                            self.page = None;
                            self.error = Some(
                                "筛选条件或 Project 缓冲已变化；丢弃旧结果，请从第一页重查。"
                                    .into(),
                            );
                        }
                    }
                    Some((_, Err(error))) => {
                        self.page = None;
                        self.error = Some(if cancel_requested || error.contains("查询已取消") {
                            "查询已取消，未返回部分结果。".into()
                        } else if submitted_query != self.query
                            || submitted_options != Some(current_options(self))
                        {
                            "筛选条件或分页参数已变化；旧查询已取消，没有返回部分结果。".into()
                        } else {
                            error
                        });
                    }
                    None => {
                        self.page = None;
                        self.error = Some("查询任务意外结束；请重新运行。".into());
                    }
                }
            }
        }
    }

    pub(super) fn next_page(&mut self, app: &WorldeditApp) {
        let Some(cursor) = self.page.as_ref().and_then(|page| page.next.as_ref()) else {
            return;
        };
        match app.project.continue_catalog_query(&self.query, cursor) {
            Ok(page) => {
                self.page = Some(page);
                self.error = None;
            }
            Err(QueryError::StaleCursor) => {
                self.page = None;
                self.error = Some("分页游标已过期；请从第一页重新查询。".into());
            }
            Err(error) => {
                self.page = None;
                self.error = Some(error.to_string());
            }
        }
    }

    pub(super) fn previous_page(&mut self, app: &WorldeditApp, offset: usize) {
        let options = CatalogQueryOptions {
            offset,
            page_size: self.page_size.clamp(1, MAX_CATALOG_QUERY_PAGE_SIZE),
            max_candidates: self.max_candidates.clamp(1, MAX_CATALOG_QUERY_CANDIDATES),
        };
        match app.project.query_catalog(&self.query, options) {
            Ok(page) => {
                self.page = Some(page);
                self.error = None;
            }
            Err(error) => {
                self.page = None;
                self.error = Some(error.to_string());
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn cancel_query(&mut self) {
        if let Some(running) = &mut self.running {
            running
                .cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
            running.cancel_requested = true;
        }
    }
}
