use worldline_core::{presentation::MapDocument, project::Project};

pub(super) enum SvgExportEvent {
    Source(String),
    #[cfg(not(target_arch = "wasm32"))]
    Picked(Option<std::path::PathBuf>),
    #[cfg(not(target_arch = "wasm32"))]
    Staged(SvgStage),
}

pub(super) struct SvgExportJob {
    #[cfg(not(target_arch = "wasm32"))]
    receiver: std::sync::mpsc::Receiver<Result<SvgExportEvent, String>>,
    #[cfg(not(target_arch = "wasm32"))]
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    #[cfg(target_arch = "wasm32")]
    worker: crate::worker_host::WorkerJob,
}

impl SvgExportJob {
    pub(super) fn render(
        _project: &Project,
        map: MapDocument,
        selected: Option<std::collections::BTreeSet<String>>,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::native(
                move |_| {
                    worldline_core::vector_scene::map_to_safe_svg(&map, selected.as_ref())
                        .map(SvgExportEvent::Source)
                        .map_err(|error| error.to_string())
                },
                ctx,
            )
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::{WorkRequest, WorkTask};
            let (files, state) = super::scene_snapshot::capture(_project)?;
            let request = WorkRequest {
                schema_version: 1,
                job_id: "map-svg-export".into(),
                generation: 0,
                baseline: _project.content_baseline(),
                entry: _project
                    .entry
                    .strip_prefix(&_project.root)
                    .map_err(|_| "工程入口越界")?
                    .to_owned(),
                snapshot_state: Some(state),
                task: WorkTask::MapSvgExport {
                    map_id: map.id,
                    selected,
                },
            };
            Ok(Self {
                worker: crate::worker_host::WorkerJob::start(request, &files, ctx)?,
            })
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn native(
        work: impl FnOnce(&std::sync::atomic::AtomicBool) -> Result<SvgExportEvent, String>
            + Send
            + 'static,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            mpsc, Arc,
        };
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let ctx = ctx.clone();
        std::thread::Builder::new()
            .name("map-svg-export".into())
            .spawn(move || {
                let result = work(&worker_cancel);
                if !worker_cancel.load(Ordering::Acquire) {
                    let _ = sender.send(result);
                }
                ctx.request_repaint();
            })
            .map_err(|error| error.to_string())?;
        Ok(Self { receiver, cancel })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn pick(filename: String, ctx: &egui::Context) -> Result<Self, String> {
        Self::native(
            move |_| {
                let path = rfd::FileDialog::new()
                    .add_filter("SVG", &["svg"])
                    .set_file_name(filename)
                    .save_file()
                    .ok_or(super::svg_import_job::NO_FILE_SELECTION)?;
                Ok(SvgExportEvent::Picked(Some(path)))
            },
            ctx,
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn stage(
        target: std::path::PathBuf,
        source: String,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        Self::native(
            move |cancel| {
                SvgStage::write(target, source.as_bytes(), cancel).map(SvgExportEvent::Staged)
            },
            ctx,
        )
    }

    pub(super) fn poll(&mut self) -> Option<Result<SvgExportEvent, String>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match self.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("SVG 导出任务已结束，未交付文件".into()))
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::{WorkEvent, WorkOutput};
            match self.worker.take_event()? {
                WorkEvent::Progress { .. } => None,
                WorkEvent::Error(error) => Some(Err(error)),
                WorkEvent::Done { output, binaries } if binaries.is_empty() => match *output {
                    WorkOutput::MapSvgExport { source } => Some(Ok(SvgExportEvent::Source(source))),
                    _ => Some(Err("SVG 导出返回了不匹配结果".into())),
                },
                _ => Some(Err("SVG 导出返回了不匹配结果".into())),
            }
        }
    }
}

impl Drop for SvgExportJob {
    fn drop(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        self.cancel
            .store(true, std::sync::atomic::Ordering::Release);
        #[cfg(target_arch = "wasm32")]
        self.worker.cancel();
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) struct SvgStage {
    temporary: std::path::PathBuf,
    target: std::path::PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl SvgStage {
    fn write(
        target: std::path::PathBuf,
        bytes: &[u8],
        cancel: &std::sync::atomic::AtomicBool,
    ) -> Result<Self, String> {
        use std::{
            io::Write,
            sync::atomic::{AtomicU64, Ordering},
        };
        static NEXT: AtomicU64 = AtomicU64::new(0);
        if target.exists() {
            return Err("SVG 目标已存在，拒绝覆盖；请选择新文件名".into());
        }
        let parent = target.parent().ok_or("SVG 目标目录无效")?;
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(".worldedit-svg-{}-{id}.tmp", std::process::id()));
        if cancel.load(Ordering::Acquire) {
            return Err("SVG 导出已取消".into());
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        let stage = Self { temporary, target };
        for chunk in bytes.chunks(64 * 1024) {
            if cancel.load(Ordering::Acquire) {
                drop(file);
                return Err("SVG 导出已取消".into());
            }
            if let Err(error) = file.write_all(chunk) {
                drop(file);
                return Err(error.to_string());
            }
        }
        let result = file.sync_all();
        drop(file);
        result.map_err(|error| error.to_string())?;
        Ok(stage)
    }

    /// 在主线程重新核 baseline 后才执行；hard_link 原子拒绝已有目标。
    pub(super) fn commit(self) -> Result<std::path::PathBuf, String> {
        std::fs::hard_link(&self.temporary, &self.target)
            .map_err(|error| format!("SVG 文件未交付（目标存在或不支持安全完成）：{error}"))?;
        Ok(self.target.clone())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for SvgStage {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.temporary);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    fn directory(name: &str) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("worldedit-svg-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }
    #[test]
    fn svg_staging_is_invisible_until_commit_and_cancel_cleans_it() {
        let root = directory("cancel");
        let target = root.join("map.svg");
        let stage = SvgStage::write(target.clone(), b"<svg/>", &AtomicBool::new(false)).unwrap();
        assert!(!target.exists());
        assert!(stage.temporary.exists());
        drop(stage);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        let stage = SvgStage::write(target.clone(), b"<svg/>", &AtomicBool::new(false)).unwrap();
        assert_eq!(stage.commit().unwrap(), target);
        assert_eq!(std::fs::read(&target).unwrap(), b"<svg/>");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn svg_target_race_and_pre_cancel_leave_existing_bytes_untouched() {
        let root = directory("race");
        let target = root.join("map.svg");
        assert!(SvgStage::write(target.clone(), b"candidate", &AtomicBool::new(true)).is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        let stage = SvgStage::write(target.clone(), b"candidate", &AtomicBool::new(false)).unwrap();
        std::fs::write(&target, b"other writer").unwrap();
        assert!(stage.commit().is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"other writer");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(root);
    }
}
