use super::super::WorldeditApp;
use worldline_runtime::{ReplayBudget, ReplayCancellation, ReplayTrace};
impl WorldeditApp {
    pub(super) fn begin_replay(&mut self, ctx: &egui::Context) {
        let _ = ctx;
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
            self.replay_debugger.selected_path = None;
            self.replay_debugger.notice = Some("所选路径已失效，请重新选择".into());
            return;
        };
        let Some(snapshot) = self.snapshot.as_ref() else {
            self.replay_debugger.notice = Some("没有可重放的编译快照".into());
            return;
        };
        if snapshot.result.has_errors() {
            self.replay_debugger.notice = Some("当前稿件有编译错误，无法重放".into());
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
        let path_name = self.replay_debugger.saved_paths[index].name.clone();
        self.replay_debugger.result = None;
        self.replay_debugger.result_path_name = Some(path_name);
        self.replay_debugger.result_version = Some(self.version);
        self.replay_debugger.notice = None;
        self.replay_debugger.explanations = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
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
            match ReplayTrace::replay(&program, &analysis, &trace, budget, &cancellation) {
                Ok(result) => self.replay_debugger.result = Some(result),
                Err(error) => self.replay_debugger.notice = Some(error.to_string()),
            }
        }
    }

    pub(super) fn poll_replay(&mut self, ctx: &egui::Context) {
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
}
