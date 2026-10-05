//! 当前已应用快照的只读试玩报告；运行与 Markdown 只由 runtime 生成。
mod export;
mod job;
mod view;

use super::scope::AppliedPlayScope;
use worldline_runtime::{PlaythroughReport, ReplayCancellation};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ReportRoute {
    #[default]
    Live,
    Saved(usize),
}

struct ReviewedReport {
    scope: AppliedPlayScope,
    route: ReportRoute,
    name: String,
    settings: (u64, u64),
    report: PlaythroughReport,
}

struct ReportJob {
    scope: AppliedPlayScope,
    route: ReportRoute,
    name: String,
    settings: (u64, u64),
    cancellation: ReplayCancellation,
    #[cfg(not(target_arch = "wasm32"))]
    receiver: std::sync::mpsc::Receiver<Result<PlaythroughReport, String>>,
    #[cfg(target_arch = "wasm32")]
    snapshot: worldline_core::CompileResult,
    #[cfg(target_arch = "wasm32")]
    session: worldline_runtime::PlaythroughReportSession,
}
impl Drop for ReportJob {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

pub(in crate::app) struct PlaythroughReportState {
    pub(in crate::app) open: bool,
    pub(in crate::app::play) focus_on_open: bool,
    route: ReportRoute,
    job: Option<ReportJob>,
    reviewed: Option<ReviewedReport>,
    scope_confirmed: bool,
    confirmed_inputs: Vec<crate::app::export_scope::UnappliedInput>,
    privacy_confirmed: bool,
    max_steps: u64,
    time_budget_ms: u64,
    #[cfg(not(target_arch = "wasm32"))]
    destination: String,
    notice: Option<String>,
}
impl Default for PlaythroughReportState {
    fn default() -> Self {
        Self {
            open: false,
            focus_on_open: false,
            route: ReportRoute::Live,
            job: None,
            reviewed: None,
            scope_confirmed: false,
            confirmed_inputs: Vec::new(),
            privacy_confirmed: false,
            max_steps: 100_000,
            time_budget_ms: 30_000,
            #[cfg(not(target_arch = "wasm32"))]
            destination: String::new(),
            notice: None,
        }
    }
}

impl crate::app::WorldeditApp {
    pub(in crate::app) fn close_playthrough_report(&mut self) {
        self.playthrough_report.open = false;
        self.playthrough_report.focus_on_open = false;
        self.playthrough_report.job = None;
        self.playthrough_report.privacy_confirmed = false;
        self.playthrough_report.scope_confirmed = false;
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
