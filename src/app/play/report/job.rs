//! 桌面后台与浏览器逐帧推进；绝不调用当前 live Story 的推进接口。
use super::{ReportJob, ReportRoute, ReviewedReport};
use crate::app::WorldeditApp;
use worldline_core::CompileResult;
use worldline_runtime::{PlaythroughReportOptions, ReplayBudget, ReplayCancellation};

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
    fn report_trace(&self) -> Option<(String, worldline_runtime::ReplayTrace)> {
        match self.playthrough_report.route {
            ReportRoute::Live => self
                .play
                .as_ref()?
                .story
                .as_ref()
                .map(|story| ("生成时捕获的当前路径".into(), story.replay_trace())),
            ReportRoute::Saved(index) => self
                .replay_debugger
                .saved_paths
                .get(index)
                .map(|path| (path.name.clone(), path.trace.clone())),
        }
    }

    pub(super) fn begin_playthrough_report(&mut self, ctx: &egui::Context) {
        if self.playthrough_report.job.is_some() {
            return;
        }
        self.playthrough_report.privacy_confirmed = false;
        let inputs = self.unapplied_play_inputs();
        if !inputs.is_empty()
            && (!self.playthrough_report.scope_confirmed
                || inputs != self.playthrough_report.confirmed_inputs)
        {
            self.playthrough_report.scope_confirmed = false;
            self.playthrough_report.notice =
                Some("请先核对并确认报告不包含列出的未应用草稿".into());
            return;
        }
        // 先借用检查，scope与owned_snapshot随后才复制完整源码。
        let Some(snapshot) = self.snapshot.as_ref() else {
            self.playthrough_report.notice = Some("已应用工程稿没有可用的编译快照".into());
            return;
        };
        let source_bytes = snapshot
            .result
            .sources
            .iter()
            .try_fold(0usize, |sum, (path, text)| {
                sum.checked_add(path.as_os_str().len())
                    .and_then(|bytes| bytes.checked_add(text.len()))
            });
        if snapshot.result.sources.len() > 4096
            || source_bytes.is_none_or(|bytes| bytes > 64 * 1024 * 1024)
        {
            self.playthrough_report.notice =
                Some("已应用源码超过4096文件或64MiB报告输入额度".into());
            return;
        }
        let Some(scope) = self.applied_play_scope() else {
            self.playthrough_report.notice = Some("已应用工程稿没有可用的编译快照".into());
            return;
        };
        if let Err(error) = self.project.verify_review_navigation() {
            self.playthrough_report.notice = Some(format!("工作区已变化，请刷新后重试：{error}"));
            return;
        }
        if !scope.matches_project(&self.project, self.version) {
            self.playthrough_report.notice = Some("已应用稿与编译快照不一致，请重新编译".into());
            return;
        }
        let Some((name, trace)) = self.report_trace() else {
            self.playthrough_report.notice = Some("请开始试玩，或选择一条已录制路径".into());
            return;
        };
        let Some(snapshot) = self.snapshot.as_ref() else {
            return;
        };
        let snapshot = owned_snapshot(&snapshot.result);
        let presentation =
            match crate::app::play::localization::prepare_trace(&self.project, &trace) {
                Ok(presentation) => presentation,
                Err(error) => {
                    self.playthrough_report.notice = Some(error);
                    return;
                }
            };
        let settings = (
            self.playthrough_report.max_steps,
            self.playthrough_report.time_budget_ms,
        );
        let options = PlaythroughReportOptions {
            budget: ReplayBudget::new(
                self.playthrough_report.max_steps,
                self.playthrough_report.time_budget_ms,
            ),
            ..Default::default()
        };
        #[cfg(target_arch = "wasm32")]
        let options = PlaythroughReportOptions {
            budget: ReplayBudget::new(
                options.budget.max_steps,
                options.budget.time_budget_ms.min(2_000),
            ),
            ..options
        };
        let cancellation = ReplayCancellation::new();
        let route = self.playthrough_report.route;
        self.playthrough_report.reviewed = None;
        self.playthrough_report.notice = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (sender, receiver) = std::sync::mpsc::channel();
            let token = cancellation.clone();
            std::thread::spawn(move || {
                let result = match &presentation {
                    Some(presentation) => {
                        worldline_runtime::generate_playthrough_report_with_presentation(
                            &snapshot,
                            &trace,
                            options,
                            &token,
                            presentation,
                        )
                    }
                    None => worldline_runtime::generate_playthrough_report(
                        &snapshot, &trace, options, &token,
                    ),
                }
                .map_err(|error| format!("{}：{}", error.code, error.message));
                let _ = sender.send(result);
            });
            self.playthrough_report.job = Some(ReportJob {
                scope,
                route,
                name,
                settings,
                cancellation,
                receiver,
            });
        }
        #[cfg(target_arch = "wasm32")]
        let prepared = match &presentation {
            Some(presentation) => {
                worldline_runtime::PlaythroughReportSession::new_with_presentation(
                    &snapshot,
                    trace,
                    options,
                    cancellation.clone(),
                    presentation,
                )
            }
            None => worldline_runtime::PlaythroughReportSession::new(
                &snapshot,
                trace,
                options,
                cancellation.clone(),
            ),
        };
        #[cfg(target_arch = "wasm32")]
        match prepared {
            Ok(session) => {
                self.playthrough_report.job = Some(ReportJob {
                    scope,
                    route,
                    name,
                    settings,
                    cancellation,
                    snapshot,
                    session,
                })
            }
            Err(error) => {
                self.playthrough_report.notice = Some(format!("{}：{}", error.code, error.message))
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }

    pub(in crate::app) fn poll_playthrough_report(&mut self, ctx: &egui::Context) {
        if self
            .playthrough_report
            .job
            .as_ref()
            .is_some_and(|job| !job.scope.matches_project(&self.project, self.version))
        {
            self.playthrough_report.job = None;
            self.playthrough_report.privacy_confirmed = false;
            self.playthrough_report.notice =
                Some("验证已取消：工作区、已应用版本或完整内容基线变化，请重新生成".into());
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let finished = match self
            .playthrough_report
            .job
            .as_ref()
            .map(|job| job.receiver.try_recv())
        {
            Some(Ok(result)) => Some(result),
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                Some(Err("试玩报告任务异常退出".into()))
            }
            _ => None,
        };
        #[cfg(target_arch = "wasm32")]
        let finished = self.playthrough_report.job.as_mut().and_then(|job| {
            match job
                .session
                .advance(&job.snapshot, ReplayBudget::new(512, 4))
            {
                Ok(Some(report)) => Some(Ok(report)),
                Ok(None) => None,
                Err(error) => Some(Err(format!("{}：{}", error.code, error.message))),
            }
        });
        if let Some(result) = finished {
            let Some(job) = self.playthrough_report.job.take() else {
                return;
            };
            match result {
                Ok(report) => {
                    self.playthrough_report.reviewed = Some(ReviewedReport {
                        scope: job.scope.clone(),
                        route: job.route,
                        name: job.name.clone(),
                        settings: job.settings,
                        report,
                    })
                }
                Err(error) => self.playthrough_report.notice = Some(error),
            }
        }
        if self.playthrough_report.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
}
