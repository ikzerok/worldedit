use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
#[cfg(not(target_arch = "wasm32"))]
use worldline_core::vector_scene::ScenePlan;
use worldline_core::vector_scene::{SceneBatch, SceneProgress};
use worldline_core::{presentation_commands::Revision, project::Project};

pub(super) enum PreparedScene {
    #[cfg(not(target_arch = "wasm32"))]
    Plan(ScenePlan),
    #[cfg(target_arch = "wasm32")]
    Batch(SceneBatch),
}

pub(super) struct SceneBatchJob {
    pub(super) baseline: String,
    pub(super) batch: SceneBatch,
    pub(super) generation: u64,
    pub(super) review: bool,
    cancelled: Arc<AtomicBool>,
    progress: Arc<Mutex<SceneProgress>>,
    #[cfg(not(target_arch = "wasm32"))]
    receiver: std::sync::mpsc::Receiver<Result<PreparedScene, String>>,
    #[cfg(target_arch = "wasm32")]
    worker: crate::worker_host::WorkerJob,
}

impl SceneBatchJob {
    pub(super) fn start(
        project: &Project,
        revision: Revision,
        batch: SceneBatch,
        generation: u64,
        review: bool,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        let baseline = project.content_baseline();
        let cancelled = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new(SceneProgress {
            stage: "准备核心预检".into(),
            completed: 0,
            total: 1,
        }));
        #[cfg(not(target_arch = "wasm32"))]
        {
            let snapshot = project.clone();
            let worker_batch = batch.clone();
            let worker_cancel = cancelled.clone();
            let worker_progress = progress.clone();
            let ctx = ctx.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::Builder::new()
                .name("scene-preview".into())
                .spawn(move || {
                    let mut callback = |value: SceneProgress| {
                        if let Ok(mut progress) = worker_progress.lock() {
                            *progress = value;
                        }
                        ctx.request_repaint();
                        !worker_cancel.load(Ordering::Acquire)
                    };
                    let result = worldline_core::vector_scene::preview_batch_with_control(
                        &snapshot,
                        revision,
                        worker_batch,
                        &Default::default(),
                        &mut callback,
                    )
                    .map(PreparedScene::Plan)
                    .map_err(|error| error.to_string());
                    let _ = sender.send(result);
                    ctx.request_repaint();
                })
                .map_err(|error| format!("无法启动矢量预检：{error}"))?;
            Ok(Self {
                baseline,
                batch,
                generation,
                review,
                cancelled,
                progress,
                receiver,
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::{WorkRequest, WorkTask};
            let (files, state) = super::scene_snapshot::capture(project)?;
            let entry = project
                .entry
                .strip_prefix(&project.root)
                .map_err(|_| "后台工程入口越界")?
                .to_owned();
            let request = WorkRequest {
                schema_version: 1,
                job_id: format!("scene-{generation}"),
                generation,
                baseline: baseline.clone(),
                entry,
                snapshot_state: Some(state),
                task: WorkTask::ScenePreview {
                    revision,
                    batch: batch.clone(),
                },
            };
            let worker = crate::worker_host::WorkerJob::start(request, &files, ctx)?;
            Ok(Self {
                baseline,
                batch,
                generation,
                review,
                cancelled,
                progress,
                worker,
            })
        }
    }

    pub(super) fn status(&self) -> SceneProgress {
        self.progress
            .lock()
            .map(|value| value.clone())
            .unwrap_or(SceneProgress {
                stage: "正在处理".into(),
                completed: 0,
                total: 0,
            })
    }

    pub(super) fn poll(&mut self) -> Option<Result<PreparedScene, String>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match self.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("矢量预检线程意外结束，输入已保留".into()))
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::{WorkEvent, WorkOutput};
            match self.worker.take_event()? {
                WorkEvent::Progress {
                    stage,
                    completed,
                    total,
                } => {
                    if let Ok(mut value) = self.progress.lock() {
                        *value = SceneProgress {
                            stage,
                            completed,
                            total,
                        };
                    }
                    None
                }
                WorkEvent::Error(error) => Some(Err(error)),
                WorkEvent::Done { output, binaries } if binaries.is_empty() => match *output {
                    WorkOutput::ScenePreview { batch } => Some(Ok(PreparedScene::Batch(batch))),
                    _ => Some(Err("矢量预检返回了不匹配的结果".into())),
                },
                _ => Some(Err("矢量预检返回了不匹配的结果".into())),
            }
        }
    }
}

impl Drop for SceneBatchJob {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        #[cfg(target_arch = "wasm32")]
        self.worker.cancel();
    }
}
