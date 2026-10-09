use super::*;
use worldline_core::catalog_scope::CatalogScopeSnapshot;

#[cfg(not(target_arch = "wasm32"))]
static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(not(target_arch = "wasm32"))]
struct QueryPermit;
#[cfg(not(target_arch = "wasm32"))]
impl Drop for QueryPermit {
    fn drop(&mut self) {
        ACTIVE.store(false, std::sync::atomic::Ordering::Release);
    }
}
pub(super) const BUSY: &str = "前一个查询仍在结束；已保留最新请求，等待真实计算释放后再启动";

pub(super) struct RunningQuery {
    #[cfg(not(target_arch = "wasm32"))]
    pub cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    #[cfg(not(target_arch = "wasm32"))]
    pub receiver: std::sync::mpsc::Receiver<(String, Result<CatalogScopeSnapshot, String>)>,
    #[cfg(target_arch = "wasm32")]
    worker: crate::worker_host::WorkerJob,
    pub query: CatalogQuery,
    pub options: CatalogQueryOptions,
    pub cancel_requested: bool,
    pub key: (u64, worldline_core::presentation_commands::Revision),
    pub observation: String,
}
impl RunningQuery {
    pub(super) fn start(
        app: &WorldeditApp,
        query: CatalogQuery,
        options: CatalogQueryOptions,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        #[cfg(not(target_arch = "wasm32"))]
        let permit = {
            ACTIVE
                .compare_exchange(
                    false,
                    true,
                    std::sync::atomic::Ordering::AcqRel,
                    std::sync::atomic::Ordering::Acquire,
                )
                .map_err(|_| BUSY.to_owned())?;
            QueryPermit
        };
        let baseline = app.project.content_baseline();
        let key = (app.version, app.map_revision);
        let observation = app.project.catalog_scope_observation_key();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let project = app.project.clone();
            let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let control = cancel.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            let submitted = query.clone();
            let ctx = ctx.clone();
            std::thread::Builder::new()
                .name("catalog-scope".into())
                .spawn(move || {
                    let _permit = permit;
                    let result = project
                        .catalog_scope_snapshot_cancellable(
                            &submitted,
                            options.max_candidates,
                            || control.load(std::sync::atomic::Ordering::Relaxed),
                        )
                        .map_err(|error| error.to_string());
                    // Release the large captured source snapshot before the compute permit.
                    drop(project);
                    let _ = sender.send((baseline, result));
                    ctx.request_repaint();
                })
                .map_err(|error| format!("无法启动查询：{error}"))?;
            Ok(Self {
                cancel,
                receiver,
                query,
                options,
                cancel_requested: false,
                key,
                observation,
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::{
                WorkRequest, WorkTask, MAX_BYTES, MAX_FILES, SCHEMA_VERSION,
            };
            let state = app.project.snapshot_state()?;
            let mut retained_bytes = 0usize;
            let mut retained_files = 0usize;
            for document in &state.documents {
                if let Some(bytes) = &document.retained_bytes {
                    retained_bytes = retained_bytes
                        .checked_add(bytes.len())
                        .ok_or("墓碑字节预算溢出")?;
                    retained_files += 1;
                }
            }
            let files = app.project.snapshot_files_limited(
                MAX_FILES
                    .checked_sub(retained_files)
                    .ok_or("墓碑文件超限")?,
                MAX_BYTES
                    .checked_sub(retained_bytes)
                    .ok_or("墓碑字节超限")?,
            )?;
            let request = WorkRequest {
                schema_version: SCHEMA_VERSION,
                job_id: format!("catalog-scope-{}", app.version),
                generation: app.version,
                baseline,
                entry: app
                    .project
                    .entry
                    .strip_prefix(&app.project.root)
                    .map_err(|_| "工程入口越界")?
                    .to_owned(),
                snapshot_state: Some(state),
                task: WorkTask::CatalogScope {
                    query: query.clone(),
                    max_candidates: options.max_candidates,
                },
            };
            let worker = crate::worker_host::WorkerJob::start(request, &files, ctx)?;
            Ok(Self {
                worker,
                query,
                options,
                cancel_requested: false,
                key,
                observation,
            })
        }
    }
    pub(super) fn cancel(&mut self) {
        self.cancel_requested = true;
        #[cfg(not(target_arch = "wasm32"))]
        self.cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        #[cfg(target_arch = "wasm32")]
        self.worker.cancel();
    }
    pub(super) fn poll(&mut self) -> Option<Result<(String, CatalogScopeSnapshot), String>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match self.receiver.try_recv() {
                Ok((baseline, result)) => Some(result.map(|scope| (baseline, scope))),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("查询任务意外结束；请重新运行".into()))
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::{WorkEvent, WorkOutput};
            if self.cancel_requested {
                return Some(Err("查询已取消".into()));
            }
            match self.worker.take_event()? {
                WorkEvent::Progress { .. } => None,
                WorkEvent::Error(error) => Some(Err(error)),
                WorkEvent::Done { output, binaries } if binaries.is_empty() => match *output {
                    WorkOutput::CatalogScope { scope } => {
                        Some(Ok((scope.query().snapshot.clone(), scope)))
                    }
                    _ => Some(Err("查询后台结果类型不匹配".into())),
                },
                _ => Some(Err("查询后台包含意外负载".into())),
            }
        }
    }
}
impl Drop for RunningQuery {
    fn drop(&mut self) {
        self.cancel();
    }
}
