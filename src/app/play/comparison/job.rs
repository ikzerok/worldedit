//! 显式启动、后台/合作式推进及取消；绘制不重新比较。
use super::{ComparedRoutes, ComparisonJob};
use crate::app::{play::scope::AppliedPlayScope, WorldeditApp};
use worldline_core::CompileResult;
use worldline_runtime::{ReplayBudget, ReplayCancellation, RouteComparisonOptions};

fn owned_snapshot(source: &CompileResult) -> CompileResult {
    CompileResult {
        program: source.program.clone(),
        analysis: source.analysis.clone(),
        diagnostics: source.diagnostics.clone(),
        sources: source.sources.clone(),
        options: source.options,
    }
}

impl WorldeditApp {
    pub(in crate::app::play) fn begin_comparison_applied(
        &mut self,
        ctx: &egui::Context,
        scope: AppliedPlayScope,
    ) {
        if self.comparison.job.is_some() {
            return;
        }
        if !scope.matches_project(&self.project, self.version) {
            self.comparison.notice = Some("已应用源码与编译快照不一致，请重新编译后比较".into());
            return;
        }
        let Some((a, b)) = self.comparison.a.zip(self.comparison.b) else {
            return;
        };
        let Some(left) = self.replay_debugger.saved_paths.get(a) else {
            return;
        };
        let Some(right) = self.replay_debugger.saved_paths.get(b) else {
            return;
        };
        let Some(snapshot) = self.snapshot.as_ref().filter(|s| !s.result.has_errors()) else {
            self.comparison.notice = Some("已应用工程稿没有可用的编译快照".into());
            return;
        };
        let names = [left.name.clone(), right.name.clone()];
        let left = left.trace.clone();
        let right = right.trace.clone();
        let snapshot = owned_snapshot(&snapshot.result);
        let time_budget_ms = self.comparison.time_budget_ms;
        #[cfg(target_arch = "wasm32")]
        let time_budget_ms = time_budget_ms.min(2_000);
        let options = RouteComparisonOptions {
            budget: ReplayBudget::new(
                self.comparison.max_steps.min(100_000),
                time_budget_ms.min(30_000),
            ),
            ..Default::default()
        };
        let cancellation = ReplayCancellation::new();
        self.comparison.generation = self.comparison.generation.wrapping_add(1);
        let id = self.comparison.generation;
        self.comparison.result = None;
        self.comparison.selected_state = None;
        self.comparison.selected_action = None;
        self.comparison.scroll = 0.0;
        self.comparison.restore_scroll = true;
        self.comparison.notice = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (sender, receiver) = std::sync::mpsc::channel();
            let token = cancellation.clone();
            std::thread::spawn(move || {
                let result =
                    worldline_runtime::compare_routes(&snapshot, &left, &right, options, &token)
                        .map_err(|error| format!("{}：{}", error.code, error.message));
                let _ = sender.send(result);
            });
            self.comparison.job = Some(ComparisonJob {
                id,
                scope,
                names,
                paths: [a, b],
                cancellation,
                receiver,
            });
        }
        #[cfg(target_arch = "wasm32")]
        match worldline_runtime::RouteComparisonSession::new(
            &snapshot,
            left,
            right,
            options,
            cancellation.clone(),
        ) {
            Ok(session) => {
                self.comparison.job = Some(ComparisonJob {
                    id,
                    scope,
                    names,
                    paths: [a, b],
                    cancellation,
                    snapshot,
                    session,
                })
            }
            Err(error) => {
                self.comparison.notice = Some(format!("{}：{}", error.code, error.message))
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }

    pub(in crate::app) fn poll_comparison(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        let finished = match self
            .comparison
            .job
            .as_ref()
            .map(|job| job.receiver.try_recv())
        {
            Some(Ok(result)) => Some(result),
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                Some(Err("路线对照任务异常退出".into()))
            }
            _ => None,
        };
        #[cfg(target_arch = "wasm32")]
        let finished = self.comparison.job.as_mut().and_then(|job| {
            match job
                .session
                .advance(&job.snapshot, ReplayBudget::new(512, 4))
            {
                Ok(Some(result)) => Some(Ok(result)),
                Ok(None) => None,
                Err(error) => Some(Err(format!("{}：{}", error.code, error.message))),
            }
        });
        if let Some(result) = finished {
            let Some(job) = self.comparison.job.take() else {
                return;
            };
            if job.id != self.comparison.generation {
                return;
            }
            match result {
                Ok(result) => {
                    self.comparison.selected_state = result
                        .state_differences
                        .first()
                        .map(|d| d.id.clone())
                        .or_else(|| {
                            result
                                .left
                                .state_actions
                                .records
                                .first()
                                .or_else(|| result.right.state_actions.records.first())
                                .map(|record| record.state.clone())
                        });
                    self.comparison.result = Some(ComparedRoutes {
                        id: job.id,
                        scope: job.scope.clone(),
                        names: job.names.clone(),
                        paths: job.paths,
                        result,
                        reversed: false,
                    });
                }
                Err(error) => self.comparison.notice = Some(error),
            }
        }
        if self.comparison.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
}
