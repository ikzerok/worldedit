use super::super::WorldeditApp;
#[cfg(not(target_arch = "wasm32"))]
use worldline_runtime::ReplayTrace;
use worldline_runtime::{ReplayBudget, ReplayCancellation, ReplaySession};
impl WorldeditApp {
    pub(super) fn begin_replay(&mut self, ctx: &egui::Context) {
        self.request_replay(ctx);
    }

    pub(super) fn begin_replay_applied(
        &mut self,
        ctx: &egui::Context,
        scope: super::scope::AppliedPlayScope,
    ) {
        if !scope.matches_project(&self.project, self.version) {
            self.replay_debugger.notice =
                Some("已应用源码与编译快照不一致，请重新编译后重放".into());
            return;
        }
        if self.replay_debugger.job.is_some() {
            return;
        }
        let Some(index) = self.replay_debugger.selected_path else {
            self.replay_debugger.notice = Some("请先选择或导入一条路径".into());
            return;
        };
        let Some(trace) = self
            .replay_debugger
            .saved_paths
            .get(index)
            .map(|path| path.trace.clone())
        else {
            self.replay_debugger.select_replay_path(None);
            self.replay_debugger.notice = Some("所选路径已失效，请重新选择".into());
            return;
        };
        let Some(snapshot) = self.snapshot.as_ref() else {
            self.replay_debugger.notice = Some("没有可重放的编译快照".into());
            return;
        };
        if snapshot.result.has_errors() {
            self.replay_debugger.notice = Some("已应用工程稿有编译错误，无法重放".into());
            return;
        }
        let program = snapshot.result.program.clone();
        let analysis = snapshot.result.analysis.clone();
        let max_steps = self.replay_debugger.max_steps;
        #[cfg(target_arch = "wasm32")]
        let max_steps = max_steps.min(100_000);
        let time_budget_ms = self.replay_debugger.time_budget_ms;
        #[cfg(target_arch = "wasm32")]
        let time_budget_ms = time_budget_ms.min(2_000);
        let budget = ReplayBudget::new(max_steps, time_budget_ms);
        let cancellation = ReplayCancellation::new();
        // 只读预检也适用于 native：旧版本可以交换/查看，但不能启动执行或清掉旧结果。
        let session = match ReplaySession::new(trace.clone(), budget, cancellation.clone()) {
            Ok(session) => session,
            Err(error) => {
                self.replay_debugger.notice = Some(format!("路径可只读查看，但无法重放：{error}"));
                return;
            }
        };
        let path_name = self.replay_debugger.saved_paths[index].name.clone();
        self.replay_debugger.result = None;
        self.replay_debugger.result_path_name = Some(path_name);
        self.replay_debugger.result_version = Some(scope.version);
        self.replay_debugger.result_scope = Some(scope);
        self.replay_debugger.notice = None;
        self.replay_debugger.explanations = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            drop(session);
            let worker_cancellation = cancellation.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let result =
                    ReplayTrace::replay(&program, &analysis, &trace, budget, &worker_cancellation)
                        .map_err(|error| error.to_string());
                let _ = sender.send(result);
            });
            self.replay_debugger.job = Some(super::super::ReplayJob {
                cancellation,
                receiver,
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.replay_debugger.job = Some(super::super::ReplayJob {
                cancellation,
                program,
                analysis,
                session,
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn poll_replay(&mut self, ctx: &egui::Context) {
        let state = self
            .replay_debugger
            .job
            .as_ref()
            .map(|job| job.receiver.try_recv());
        match state {
            Some(Ok(Ok(result))) => {
                self.replay_debugger.job = None;
                self.replay_debugger.result = Some(result);
            }
            Some(Ok(Err(error))) => {
                self.replay_debugger.job = None;
                self.replay_debugger.notice = Some(error);
            }
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                self.replay_debugger.job = None;
                self.replay_debugger.notice = Some("重放任务异常退出".into());
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
            }
            None => {}
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(in crate::app) fn poll_replay(&mut self, ctx: &egui::Context) {
        let state = self.replay_debugger.job.as_mut().map(|job| {
            job.session
                .advance(&job.program, &job.analysis, ReplayBudget::new(512, 4))
        });
        match state {
            Some(Ok(Some(result))) => {
                self.replay_debugger.job = None;
                self.replay_debugger.result = Some(result);
            }
            Some(Ok(None)) => ctx.request_repaint_after(std::time::Duration::from_millis(16)),
            Some(Err(error)) => {
                self.replay_debugger.job = None;
                self.replay_debugger.notice = Some(error.to_string());
            }
            None => {}
        }
    }
}
