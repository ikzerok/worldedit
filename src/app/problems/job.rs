use super::schedule::ReportTicket;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use worldline_core::{
    problems::{ProblemsOptions, ProblemsReport},
    project::Project,
};

pub(super) struct ProblemsJob {
    pub ticket: ReportTicket,
    cancelled: Arc<AtomicBool>,
    progress: Arc<Mutex<String>>,
    #[cfg(not(target_arch = "wasm32"))]
    receiver: std::sync::mpsc::Receiver<Result<ProblemsReport, String>>,
    #[cfg(target_arch = "wasm32")]
    worker: crate::worker_host::WorkerJob,
}
impl ProblemsJob {
    pub(super) fn start(
        project: &Project,
        ticket: ReportTicket,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new("准备工程问题检查".into()));
        #[cfg(not(target_arch = "wasm32"))]
        {
            let snapshot = project.clone();
            let worker_cancel = cancelled.clone();
            let worker_progress = progress.clone();
            let ctx = ctx.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::Builder::new()
                .name("project-problems".into())
                .spawn(move || {
                    let result = snapshot
                        .problems_report_with_progress(&report_options(), &mut |domain| {
                            if let Ok(mut value) = worker_progress.lock() {
                                *value = format!("正在检查 {}", super::view::domain_label(domain));
                            }
                            ctx.request_repaint();
                            !worker_cancel.load(Ordering::Acquire)
                        })
                        .map_err(|error| error.to_string());
                    if !worker_cancel.load(Ordering::Acquire) {
                        let _ = sender.send(result);
                    }
                    ctx.request_repaint();
                })
                .map_err(|error| format!("无法启动工程问题检查：{error}"))?;
            Ok(Self {
                ticket,
                cancelled,
                progress,
                receiver,
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::{
                WorkRequest, WorkTask, MAX_BYTES, MAX_FILES, SCHEMA_VERSION,
            };
            let state = project.snapshot_state()?;
            let retained = state
                .documents
                .iter()
                .filter_map(|d| d.retained_bytes.as_ref())
                .collect::<Vec<_>>();
            let retained_bytes = retained
                .iter()
                .try_fold(0usize, |total, bytes| total.checked_add(bytes.len()))
                .ok_or("墓碑预算溢出")?;
            let files = project.snapshot_files_limited(
                MAX_FILES
                    .checked_sub(retained.len())
                    .ok_or("墓碑文件数超额")?,
                MAX_BYTES
                    .checked_sub(retained_bytes)
                    .ok_or("墓碑字节超额")?,
            )?;
            let request = WorkRequest {
                schema_version: SCHEMA_VERSION,
                job_id: format!("problems-{}-{}", ticket.version, ticket.generation),
                generation: ticket.generation,
                baseline: ticket.baseline.clone(),
                entry: project
                    .entry
                    .strip_prefix(&project.root)
                    .map_err(|_| "工程入口越界")?
                    .to_owned(),
                snapshot_state: Some(state),
                task: WorkTask::ProblemsReport {
                    options: report_options(),
                    source_observation: ticket.observation.clone(),
                },
            };
            let worker = crate::worker_host::WorkerJob::start(request, &files, ctx)?;
            Ok(Self {
                ticket,
                cancelled,
                progress,
                worker,
            })
        }
    }

    pub(super) fn status(&self) -> String {
        self.progress
            .lock()
            .map(|value| value.clone())
            .unwrap_or_else(|_| "正在检查工程问题".into())
    }

    pub(super) fn poll(&mut self) -> Option<Result<ProblemsReport, String>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match self.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("工程问题检查线程已结束，未取得完整报告".into()))
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::{WorkEvent, WorkOutput};
            match self.worker.take_event()? {
                WorkEvent::Progress { stage, .. } => {
                    if let Ok(mut value) = self.progress.lock() {
                        *value = stage;
                    }
                    None
                }
                WorkEvent::Error(error) => Some(Err(error)),
                WorkEvent::Done { output, binaries } if binaries.is_empty() => match *output {
                    WorkOutput::ProblemsReport { report } => Some(Ok(report)),
                    _ => Some(Err("工程问题后台结果类型不匹配".into())),
                },
                _ => Some(Err("工程问题后台负载不匹配".into())),
            }
        }
    }
}
impl Drop for ProblemsJob {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        #[cfg(target_arch = "wasm32")]
        self.worker.cancel();
    }
}

fn report_options() -> ProblemsOptions {
    ProblemsOptions {
        max_report_bytes: 16 * 1024 * 1024,
        ..Default::default()
    }
}
