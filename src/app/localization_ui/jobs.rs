//! Queue only the latest preparation; results are tied to workspace, applied version and request.
use super::*;
use crate::localization_job::{Job, Output, Task, BUSY};
use worldline_core::localization::LocalizationCatalogQuery;

pub(super) enum Intent {
    Catalog { query: LocalizationCatalogQuery },
    Edits { keys: Vec<String> },
    Id { key: String },
    Import,
    Export,
}
struct Request {
    task: Task,
    intent: Intent,
}
struct Running {
    job: Job,
    intent: Intent,
}
#[derive(Clone, Copy)]
pub(super) enum AcceptedKind {
    Catalog,
    Export,
    Import,
    Preview,
}
#[derive(Default)]
pub(super) struct Jobs {
    generation: u64,
    context: Option<(std::path::PathBuf, u64)>,
    accepted_catalog: Option<(std::path::PathBuf, u64)>,
    accepted_export: Option<(std::path::PathBuf, u64)>,
    accepted_import: Option<(std::path::PathBuf, u64)>,
    accepted_preview: Option<(std::path::PathBuf, u64)>,
    queued: Option<Request>,
    running: Option<Running>,
    pub notice: Option<String>,
}
impl Jobs {
    // A retained page may outlive a context switch; the current pump context alone is not proof.
    pub fn accepted_matches(&self, kind: AcceptedKind, root: &Path, version: u64) -> bool {
        let accepted = match kind {
            AcceptedKind::Catalog => &self.accepted_catalog,
            AcceptedKind::Export => &self.accepted_export,
            AcceptedKind::Import => &self.accepted_import,
            AcceptedKind::Preview => &self.accepted_preview,
        };
        accepted
            .as_ref()
            .is_some_and(|(path, v)| path == root && *v == version)
    }
    fn accepted(&mut self, kind: AcceptedKind, root: &Path, version: u64) {
        let slot = match kind {
            AcceptedKind::Catalog => &mut self.accepted_catalog,
            AcceptedKind::Export => &mut self.accepted_export,
            AcceptedKind::Import => &mut self.accepted_import,
            AcceptedKind::Preview => &mut self.accepted_preview,
        };
        *slot = Some((root.to_owned(), version));
    }
    fn clear_accepted(&mut self) {
        self.accepted_catalog = None;
        self.accepted_export = None;
        self.accepted_import = None;
        self.accepted_preview = None;
    }
    pub fn submit(&mut self, task: Task, intent: Intent) {
        self.cancel();
        self.notice = None;
        self.queued = Some(Request { task, intent });
    }
    pub fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.running = None;
        self.queued = None;
    }
    pub fn cancel_preview(&mut self) {
        if self
            .running
            .as_ref()
            .is_some_and(|r| !matches!(r.intent, Intent::Catalog { .. }))
            || self
                .queued
                .as_ref()
                .is_some_and(|r| !matches!(r.intent, Intent::Catalog { .. }))
        {
            self.cancel();
        }
    }
    pub fn pending(&self) -> bool {
        self.running.is_some() || self.queued.is_some()
    }
    pub fn catalog_pending(&self) -> bool {
        self.running
            .as_ref()
            .is_some_and(|r| matches!(r.intent, Intent::Catalog { .. }))
            || self
                .queued
                .as_ref()
                .is_some_and(|r| matches!(r.intent, Intent::Catalog { .. }))
    }
}

fn report_failure(state: &mut LocalizationUiState, intent: &Intent, error: String) {
    match intent {
        Intent::Catalog { .. } => state.workbench.query_error = Some(error),
        _ => state.status = Some(Err(error)),
    }
}

pub(super) fn pump(
    ctx: &egui::Context,
    project: &Project,
    state: &mut LocalizationUiState,
    version: u64,
) {
    let context = (project.root.clone(), version);
    if state
        .jobs
        .context
        .as_ref()
        .is_some_and(|old| old != &context)
    {
        state.jobs.cancel();
        state.jobs.clear_accepted();
        state.workbench.invalidate();
        state.workbench.preview = None;
        state.import_plan = None;
        state.export_plan = None;
        state.confirm_apply = false;
    }
    state.jobs.context = Some(context);
    let received = state
        .jobs
        .running
        .as_mut()
        .and_then(|running| running.job.poll());
    if let Some(result) = received {
        let running = state.jobs.running.take().unwrap();
        let job = &running.job;
        if job.root == project.root
            && job.version == version
            && job.generation == state.jobs.generation
        {
            match result {
                Ok(output) if job.task.matches_output(&output) => {
                    install(state, &job.root, version, &job.task, running.intent, output)
                }
                Ok(_) => report_failure(
                    state,
                    &running.intent,
                    "本地化结果类型不匹配；保留先前稳定内容".into(),
                ),
                Err(error) => report_failure(state, &running.intent, error),
            }
        }
    }
    if state.jobs.running.is_none() && state.jobs.queued.is_some() {
        if Job::available() {
            let request = state.jobs.queued.take().unwrap();
            match Job::start(
                project,
                request.task.clone(),
                state.jobs.generation,
                version,
                ctx,
            ) {
                Ok(job) => {
                    state.jobs.running = Some(Running {
                        job,
                        intent: request.intent,
                    });
                    state.jobs.notice = None;
                }
                Err(error) if error == BUSY => {
                    state.jobs.queued = Some(request);
                    state.jobs.notice = Some(error);
                }
                Err(error) => report_failure(state, &request.intent, error),
            }
        } else {
            state.jobs.notice = Some(BUSY.into());
        }
    }
    if state.jobs.pending() {
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

fn install(
    state: &mut LocalizationUiState,
    root: &Path,
    version: u64,
    task: &Task,
    intent: Intent,
    output: Output,
) {
    match (intent, task, output) {
        (Intent::Catalog { query }, _, Output::Catalog { page }) => {
            catalog::install(state, page, version, query);
            state.jobs.accepted(AcceptedKind::Catalog, root, version);
        }
        (Intent::Edits { keys }, Task::EditPreview { draft }, Output::EditPreview { plan }) => {
            state.workbench.preview = Some(plans::Preview::Edits {
                draft: draft.clone(),
                plan,
                keys,
            });
            state.jobs.accepted(AcceptedKind::Preview, root, version);
        }
        (Intent::Id { key }, Task::IdPreview { draft }, Output::IdPreview { plan }) => {
            state.workbench.preview = Some(plans::Preview::Id {
                draft: draft.clone(),
                plan,
                key,
            });
            state.jobs.accepted(AcceptedKind::Preview, root, version);
        }
        (Intent::Import, _, Output::ImportPreview { exchange, plan }) => {
            state.status = Some(Ok(if plan.can_apply {
                "导入预览通过；工程尚未修改，请复核后确认。"
            } else {
                "核心诊断阻止应用；原输入完整保留。"
            }
            .into()));
            state.import_exchange = Some(exchange);
            state.import_plan = Some(plan);
            state.jobs.accepted(AcceptedKind::Import, root, version);
        }
        (Intent::Export, _, Output::ExportPreview { plan }) => {
            state.status = Some(Ok(if plan.can_export {
                "导出预览通过；尚未创建文件。"
            } else {
                "导出预览包含诊断，不能导出。"
            }
            .into()));
            state.export_plan = Some(plan);
            state.jobs.accepted(AcceptedKind::Export, root, version);
        }
        (intent, _, _) => report_failure(
            state,
            &intent,
            "本地化请求与回传结果不一致；没有修改工程".into(),
        ),
    }
}

pub(super) fn status(ui: &mut Ui, state: &mut LocalizationUiState) {
    if state.jobs.pending() {
        ui.horizontal_wrapped(|ui| {
            ui.spinner();
            ui.label("正在后台核对本地化；先前稳定内容保留");
            if ui.button("取消本次核对").reveal_focus(ui).clicked() {
                state.jobs.cancel();
                state.jobs.notice =
                    Some("已取消；原生内部核对退出后释放，迟到结果不会显示。输入保留。".into());
            }
        });
    }
    if let Some(notice) = &state.jobs.notice {
        ui.small(notice);
    }
}

#[cfg(test)]
pub(super) fn settle(project: &Project, state: &mut LocalizationUiState, version: u64) {
    let ctx = egui::Context::default();
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while state.jobs.pending() {
        pump(&ctx, project, state, version);
        assert!(
            std::time::Instant::now() < end,
            "localization job did not settle"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
