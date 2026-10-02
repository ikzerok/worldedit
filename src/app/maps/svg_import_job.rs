use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use worldline_core::vector_scene::{SceneProgress, SvgScenePreview};

pub(super) struct SvgResult {
    pub(super) source: String,
    pub(super) preview: Result<SvgScenePreview, String>,
}

pub(super) struct SvgJob {
    cancelled: Arc<AtomicBool>,
    progress: Arc<Mutex<SceneProgress>>,
    #[cfg(not(target_arch = "wasm32"))]
    receiver: std::sync::mpsc::Receiver<Result<SvgResult, String>>,
    #[cfg(target_arch = "wasm32")]
    worker: Option<crate::worker_host::WorkerJob>,
    #[cfg(target_arch = "wasm32")]
    picker: Option<super::svg_picker::SvgPicker>,
    #[cfg(target_arch = "wasm32")]
    source: String,
}

impl SvgJob {
    pub(super) fn check(source: String, ctx: &egui::Context) -> Result<Self, String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::native(move || Ok(source), ctx)
        }
        #[cfg(target_arch = "wasm32")]
        {
            let worker = Some(start_worker(source.clone(), ctx)?);
            Ok(Self {
                cancelled: Arc::new(AtomicBool::new(false)),
                progress: status(),
                worker,
                picker: None,
                source,
            })
        }
    }

    pub(super) fn pick(ctx: &egui::Context) -> Result<Self, String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::native(
                || {
                    use std::io::Read;
                    let path = rfd::FileDialog::new()
                        .add_filter("SVG 矢量", &["svg"])
                        .pick_file()
                        .ok_or("已取消文件选择")?;
                    let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
                    if file.metadata().map_err(|error| error.to_string())?.len() > 2 * 1024 * 1024 {
                        return Err("SVG 超过 2 MiB 上限".into());
                    }
                    let mut bytes = Vec::new();
                    file.take(2 * 1024 * 1024 + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|error| error.to_string())?;
                    if bytes.len() > 2 * 1024 * 1024 {
                        return Err("SVG 超过 2 MiB 上限".into());
                    }
                    String::from_utf8(bytes).map_err(|_| "SVG 不是有效 UTF-8".into())
                },
                ctx,
            )
        }
        #[cfg(target_arch = "wasm32")]
        {
            let picker = Some(super::svg_picker::SvgPicker::start(ctx)?);
            Ok(Self {
                cancelled: Arc::new(AtomicBool::new(false)),
                progress: status(),
                worker: None,
                picker,
                source: String::new(),
            })
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn native(
        read: impl FnOnce() -> Result<String, String> + Send + 'static,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancelled.clone();
        let progress = status();
        let worker_progress = progress.clone();
        let ctx = ctx.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("svg-preview".into())
            .spawn(move || {
                let result = read().map(|source| {
                    let mut callback = |value: SceneProgress| {
                        if let Ok(mut current) = worker_progress.lock() {
                            *current = value;
                        }
                        ctx.request_repaint();
                        !worker_cancel.load(Ordering::Acquire)
                    };
                    let preview = worldline_core::svg_import::preview_scene_with_control(
                        &source,
                        &Default::default(),
                        &mut callback,
                    )
                    .map_err(format_error);
                    SvgResult { source, preview }
                });
                let _ = sender.send(result);
                ctx.request_repaint();
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            cancelled,
            progress,
            receiver,
        })
    }

    pub(super) fn label(&self) -> String {
        self.progress
            .lock()
            .map(|value| format!("{} · {} / {}", value.stage, value.completed, value.total))
            .unwrap_or_else(|_| "正在检查 SVG".into())
    }

    pub(super) fn poll(&mut self, _ctx: &egui::Context) -> Option<Result<SvgResult, String>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match self.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("SVG 后台任务意外结束，输入保留".into()))
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(result) = self
                .picker
                .as_mut()
                .and_then(super::svg_picker::SvgPicker::poll)
            {
                self.picker = None;
                match result.and_then(|source| {
                    self.source = source.clone();
                    start_worker(source, _ctx)
                }) {
                    Ok(worker) => self.worker = Some(worker),
                    Err(error) => return Some(Err(error)),
                }
            }
            use crate::worker_protocol::{WorkEvent, WorkOutput};
            match self.worker.as_mut()?.take_event()? {
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
                WorkEvent::Error(error) => Some(Ok(SvgResult {
                    source: self.source.clone(),
                    preview: Err(error),
                })),
                WorkEvent::Done { output, binaries } if binaries.is_empty() => match *output {
                    WorkOutput::SvgPreview { preview } => Some(Ok(SvgResult {
                        source: self.source.clone(),
                        preview: Ok(preview),
                    })),
                    _ => Some(Err("SVG 后台预检返回类型不匹配".into())),
                },
                _ => Some(Err("SVG 后台预检返回类型不匹配".into())),
            }
        }
    }
}

fn status() -> Arc<Mutex<SceneProgress>> {
    Arc::new(Mutex::new(SceneProgress {
        stage: "等待读取 / 安全预检".into(),
        completed: 0,
        total: 1,
    }))
}

#[cfg(not(target_arch = "wasm32"))]
fn format_error(error: worldline_core::vector_scene::SceneError) -> String {
    format!(
        "{error}{}{}",
        error
            .line
            .map(|line| format!(" · 行 {line} 列 {}", error.column.unwrap_or(1)))
            .unwrap_or_default(),
        error
            .field
            .as_deref()
            .map(|field| format!(" · {field}"))
            .unwrap_or_default()
    )
}

#[cfg(target_arch = "wasm32")]
fn start_worker(
    source: String,
    ctx: &egui::Context,
) -> Result<crate::worker_host::WorkerJob, String> {
    use crate::worker_protocol::{WorkRequest, WorkTask};
    crate::worker_host::WorkerJob::start(
        WorkRequest {
            schema_version: 1,
            job_id: "svg-preview".into(),
            generation: 0,
            baseline: String::new(),
            entry: "world.wl".into(),
            snapshot_state: None,
            task: WorkTask::SvgPreview { source },
        },
        &crate::archive::Files::new(),
        ctx,
    )
}

impl Drop for SvgJob {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        #[cfg(target_arch = "wasm32")]
        if let Some(worker) = &mut self.worker {
            worker.cancel();
        }
    }
}
