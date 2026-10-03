//! 作者问题工具只消费 core 报告；不在 UI 推断诊断、位置或修复状态。
mod details;
mod excerpt;
mod job;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod job_slot_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod keyboard_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod lifecycle_tests;
mod navigation;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod performance_tests;
mod schedule;
mod source;
pub(super) use source::SourceProblem;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod source_history_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod source_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod state_visual_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
mod ui;
mod view;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod workbench_tests;

use crate::app::WorldeditApp;
use schedule::ReportSchedule;
use std::sync::Arc;
use worldline_core::problems::{
    ProblemCursor, ProblemPage, ProblemQuery, ProblemRelatedPage, ProblemsReport,
};

#[derive(Default)]
pub(super) struct ProblemsState {
    report: Option<Arc<ProblemsReport>>,
    report_version: Option<u64>,
    observation: Option<String>,
    observation_at: f64,
    observation_failed: bool,
    invalidated: bool,
    schedule: ReportSchedule,
    job: Option<job::ProblemsJob>,
    query: ProblemQuery,
    page: Option<Arc<ProblemPage>>,
    cursor: Option<ProblemCursor>,
    previous_pages: Vec<Option<ProblemCursor>>,
    selected: Option<String>,
    related: Option<Arc<ProblemRelatedPage>>,
    related_offset: usize,
    related_history: Vec<Option<ProblemCursor>>,
    related_cursor: Option<ProblemCursor>,
    current_file: bool,
    notice: Option<String>,
    error: Option<String>,
    pub return_focus: Option<egui::Id>,
    focus_list: bool,
    focus_detail: bool,
    scroll_selected: bool,
    detail_reset: bool,
    pub narrow_detail: bool,
    rendered_rows: usize,
    source: Option<Arc<SourceProblem>>,
    source_collapsed: bool,
    source_generation: u64,
    severity_counts: (usize, usize),
}

impl ProblemsState {
    fn cancel_job(&mut self) {
        if self.job.as_mut().is_some_and(job::ProblemsJob::cancel) {
            self.job = None;
        }
    }

    fn reject_source_navigation(&mut self, error: String) {
        self.invalidated = true;
        self.source_generation = self.source_generation.wrapping_add(1);
        self.cancel_job();
        self.schedule.cancel();
        self.error = Some(error);
        self.notice = Some("来源尚未重新检查，已撤除位置强调；当前稿与返回历史已保留".into());
    }

    fn cancelling(&self) -> bool {
        self.job
            .as_ref()
            .is_some_and(job::ProblemsJob::is_cancelled)
    }

    pub(in crate::app) fn reset_for_workspace(&mut self) {
        self.cancel_job();
        let retiring = self.job.take();
        *self = Self {
            job: retiring,
            ..Default::default()
        };
    }

    fn refresh_query(&mut self) {
        let Some(report) = &self.report else { return };
        match report.query(&self.query, self.cursor.as_ref(), 200) {
            Ok(page) => {
                if self
                    .selected
                    .as_ref()
                    .is_some_and(|id| !page.entries.iter().any(|e| &e.id == id))
                {
                    self.selected = None;
                    self.source = None;
                    self.related = None;
                    self.narrow_detail = false;
                    self.notice = Some("原选中问题不在当前结果，请重新选择；这不表示已解决".into());
                }
                self.page = Some(Arc::new(page));
                self.error = None;
            }
            Err(error) => {
                self.page = None;
                self.error = Some(error.to_string());
            }
        }
    }

    fn change_filter(&mut self) {
        self.cursor = None;
        self.previous_pages.clear();
        self.refresh_query();
    }

    fn select(&mut self, id: String) {
        if self.selected.as_ref() != Some(&id) {
            self.selected = Some(id.clone());
            self.source = None;
            self.detail_reset = true;
            self.related_cursor = None;
            self.related_history.clear();
            self.related_offset = 0;
            self.related = self
                .report
                .as_ref()
                .and_then(|report| report.related_page(&id, None, 50).ok().map(Arc::new));
            self.notice = None;
        }
        self.scroll_selected = true;
    }

    fn install(&mut self, report: ProblemsReport, version: u64) {
        // 即使外部观测回到同一hash，旧导航也不得被重新接管。
        self.source_generation = self.source_generation.wrapping_add(1);
        self.source = None;
        let changed = self
            .report
            .as_ref()
            .is_none_or(|old| old.report_version != report.report_version);
        if changed {
            self.source = None;
            self.focus_detail = false;
            if self.selected.take().is_some() {
                self.notice = Some(
                    "检查基线已更新，原选中问题未跨版本匹配；请重新选择，不能据此认定已解决".into(),
                );
            }
            self.cursor = None;
            self.previous_pages.clear();
            self.related = None;
            self.narrow_detail = false;
        }
        self.severity_counts = report
            .entries
            .iter()
            .fold((0, 0), |(errors, warnings), entry| {
                (
                    errors + usize::from(entry.severity == worldline_core::Severity::Error),
                    warnings + usize::from(entry.severity == worldline_core::Severity::Warning),
                )
            });
        self.invalidated = false;
        self.report = Some(Arc::new(report));
        self.report_version = Some(version);
        self.refresh_query();
    }

    fn stale(&self, version: u64) -> bool {
        self.invalidated
            || self.report_version != Some(version)
            || self.observation_failed
            || self
                .report
                .as_ref()
                .is_some_and(|report| Some(&report.source_observation) != self.observation.as_ref())
    }
}

impl WorldeditApp {
    pub(in crate::app) fn open_problems(&mut self, ctx: &egui::Context) {
        self.sync_edit_layers(ctx);
        if !self.personal.settings.diagnostics {
            self.problems.return_focus = ctx.memory(|m| m.focused());
        }
        self.personal.settings.diagnostics = true;
        self.personal.settings.focus = false;
        self.problems.focus_list = true;
    }

    pub(in crate::app) fn poll_problems(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|input| input.time);
        if self.problems.schedule.observe(self.version, now) {
            self.problems.invalidated = true;
            self.problems.cancel_job();
            self.problems.error = None;
        }
        if (self.problems.observation.is_none() && !self.problems.observation_failed)
            || now >= self.problems.observation_at + 1.
        {
            self.problems.observation_at = now;
            match self.project.problems_observation_key() {
                Ok(key) => {
                    if self.problems.observation.as_ref() != Some(&key)
                        || self.problems.observation_failed
                    {
                        self.problems.invalidated = true;
                        self.problems.schedule.invalidate(self.version, now);
                        self.problems.cancel_job();
                        self.problems.error = None;
                    }
                    self.problems.observation = Some(key);
                    self.problems.observation_failed = false;
                }
                Err(error) => {
                    self.problems.observation_failed = true;
                    self.problems.cancel_job();
                    self.problems.schedule.cancel();
                    self.problems.error =
                        Some(format!("来源范围无法重新观测，旧结果未重新检查：{error}"));
                }
            }
        }
        if self.problems.current_file {
            let path = self
                .active_file
                .strip_prefix(&self.project.root)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"));
            if self.problems.query.path != path {
                self.problems.query.path = path;
                self.problems.change_filter();
            }
        }
        if let Some((ticket, cancelled, result)) = self.problems.job.as_mut().and_then(|job| {
            job.poll()
                .map(|result| (job.ticket.clone(), job.is_cancelled(), result))
        }) {
            self.finish_problem_job(ticket, cancelled, result, now);
        }
        if self.problems.job.is_none()
            && self.problems.schedule.ready(now)
            && !self.problems.observation_failed
        {
            if let Some(ticket) = self.problems.schedule.begin(
                self.project.content_baseline(),
                self.problems.observation.clone().unwrap_or_default(),
            ) {
                match job::ProblemsJob::start(&self.project, ticket, ctx) {
                    Ok(job) => self.problems.job = Some(job),
                    Err(error) => {
                        self.problems.schedule.finish();
                        self.problems.error = Some(error);
                    }
                }
            }
        }
        if self.problems.job.is_some()
            || self.problems.stale(self.version) && self.problems.error.is_none()
        {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn finish_problem_job(
        &mut self,
        ticket: schedule::ReportTicket,
        cancelled: bool,
        result: Result<ProblemsReport, String>,
        now: f64,
    ) {
        if cancelled {
            self.problems.job = None;
            self.problems.schedule.finish();
            // 保留最新排队意图：显式取消的attempted仍为true，retry/invalidate才允许重启。
            return;
        }
        let baseline = self.project.content_baseline();
        let observation = self.project.problems_observation_key().ok();
        let current = observation.as_ref().is_some_and(|observation| {
            self.problems
                .schedule
                .accepts(&ticket, self.version, &baseline, observation)
        });
        self.problems.job = None;
        self.problems.schedule.finish();
        if current {
            match result {
                Ok(report)
                    if report.content_baseline == baseline
                        && observation.as_ref() == Some(&report.source_observation) =>
                {
                    self.problems.install(report, self.version)
                }
                Ok(_) => self.problems.error = Some("后台报告基线不匹配，未替换当前结果".into()),
                Err(error) => self.problems.error = Some(error),
            }
        } else if let Some(observation) = observation {
            self.problems.observation = Some(observation);
            self.problems.invalidated = true;
            self.problems.schedule.invalidate(self.version, now);
            self.problems.notice = Some("来源范围在检查期间变化，已丢弃旧结果并等待重检".into());
        } else {
            self.problems.observation_failed = true;
            self.problems.error = Some("来源范围无法读取，后台结果未接管当前稿".into());
        }
    }

    fn retry_problems(&mut self, ctx: &egui::Context) {
        self.problems.invalidated = true;
        self.problems.cancel_job();
        self.problems.error = None;
        self.problems.schedule.retry(ctx.input(|input| input.time));
        ctx.request_repaint();
    }

    fn cancel_problems(&mut self) {
        self.problems.invalidated = true;
        self.problems.cancel_job();
        self.problems.schedule.cancel();
        self.problems.error = Some("检查已取消；现有结果未重新检查，请显式刷新".into());
    }
}
