use super::render_budget::{ExecutionGuard, RenderPermit};
use super::scene_renderer::{RasterReady, RenderKey};
use std::sync::Arc;
use worldline_core::vector_scene::MapScene;

#[cfg(not(target_arch = "wasm32"))]
pub(super) struct RenderJob {
    receiver: std::sync::mpsc::Receiver<RasterReady>,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    key: RenderKey,
    permit: Arc<RenderPermit>,
    thread: Option<std::thread::JoinHandle<()>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl RenderJob {
    pub(super) fn start(
        ctx: &egui::Context,
        scene: MapScene,
        key: RenderKey,
        permit: Arc<RenderPermit>,
        execution: ExecutionGuard,
    ) -> Result<Self, String> {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        let (sender, receiver) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let ctx = ctx.clone();
        let fallback_key = key.clone();
        let fallback_permit = permit.clone();
        let thread = std::thread::Builder::new()
            .name("scene-raster".into())
            .spawn(move || {
                let _execution = execution;
                let pixels = if worker_cancelled.load(Ordering::Acquire) {
                    Err("渲染任务已取消".into())
                } else {
                    crate::scene_raster::render_scene(&scene, key.extent, &key.spec)
                };
                // 结果与 reservation 一起进通道；UI 已关时 send 失败会同时释放。
                let ready = RasterReady {
                    key,
                    pixels,
                    permit,
                };
                drop(scene);
                if !worker_cancelled.load(Ordering::Acquire) {
                    let _ = sender.send(ready);
                }
                ctx.request_repaint();
            })
            .map_err(|error| format!("无法启动 scene 渲染线程：{error}"))?;
        Ok(Self {
            receiver,
            cancelled,
            key: fallback_key,
            permit: fallback_permit,
            thread: Some(thread),
        })
    }

    pub(super) fn poll(&mut self) -> Option<RasterReady> {
        if self
            .thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
        {
            return None;
        }
        if self
            .thread
            .take()
            .is_some_and(|thread| thread.join().is_err())
        {
            return Some(RasterReady {
                key: self.key.clone(),
                permit: self.permit.clone(),
                pixels: Err("scene 渲染线程异常退出".into()),
            });
        }
        match self.receiver.try_recv() {
            Ok(ready) => Some(ready),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(RasterReady {
                key: self.key.clone(),
                permit: self.permit.clone(),
                pixels: Err("scene 渲染线程意外结束，请重试".into()),
            }),
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) struct RenderJob {
    worker: crate::worker_host::WorkerJob,
    key: RenderKey,
    permit: Arc<RenderPermit>,
    execution: Option<ExecutionGuard>,
}

#[cfg(target_arch = "wasm32")]
impl RenderJob {
    pub(super) fn start(
        ctx: &egui::Context,
        scene: MapScene,
        key: RenderKey,
        permit: Arc<RenderPermit>,
        execution: ExecutionGuard,
    ) -> Result<Self, String> {
        use crate::worker_protocol::{WorkRequest, WorkTask};
        let request = WorkRequest {
            schema_version: 1,
            job_id: format!("render-{}", key.generation),
            generation: key.generation,
            baseline: format!("{key:?}"),
            entry: "world.wl".into(),
            snapshot_state: None,
            task: WorkTask::RenderScene {
                scene,
                extent: key.extent,
                spec: key.spec.clone(),
            },
        };
        let worker =
            crate::worker_host::WorkerJob::start(request, &crate::archive::Files::new(), ctx)?;
        Ok(Self {
            worker,
            key,
            permit,
            execution: Some(execution),
        })
    }

    pub(super) fn poll(&mut self) -> Option<RasterReady> {
        use crate::worker_protocol::{WorkEvent, WorkOutput};
        let pixels = match self.worker.take_event()? {
            WorkEvent::Progress { .. } => return None,
            WorkEvent::Error(error) => Err(error),
            WorkEvent::Done {
                output,
                mut binaries,
            } => match *output {
                WorkOutput::RenderedScene { spec }
                    if spec == self.key.spec && binaries.len() == 1 =>
                {
                    Ok(binaries.remove(0))
                }
                _ => Err("后台渲染结果类型、视口或像素数不匹配".into()),
            },
        };
        self.worker.cancel();
        self.execution = None;
        Some(RasterReady {
            key: self.key.clone(),
            pixels,
            permit: self.permit.clone(),
        })
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for RenderJob {
    fn drop(&mut self) {
        self.worker.cancel();
        self.execution = None;
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for RenderJob {
    fn drop(&mut self) {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::Release);
    }
}
