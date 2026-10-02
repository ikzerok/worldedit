//! 只物化已审核文件；后台写入/清理，主线程在关闭意图之后才请求系统打开。
use super::super::WorldeditApp;
use super::*;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

thread_local! {
    static PREVIEWS: std::cell::RefCell<BTreeMap<String, PreviewDirectory>> = const { std::cell::RefCell::new(BTreeMap::new()) };
    #[cfg(test)]
    static OPEN_REQUESTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(super) fn clear_cached_for_app_close() {
    let previews = PREVIEWS.with(|previews| std::mem::take(&mut *previews.borrow_mut()));
    if !previews.is_empty() {
        close::spawn_cleanup(move || drop(previews));
    }
}

struct PreviewDirectory(PathBuf);
impl Drop for PreviewDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
enum PreviewMessage {
    Stage(usize, usize),
    Done(Result<PreviewDirectory, String>),
}

pub(super) struct BrowserPreviewJob {
    cancel: Arc<AtomicBool>,
    receiver: Option<mpsc::Receiver<PreviewMessage>>,
    baseline: String,
    digest: String,
    selection: ReaderExportSelection,
    profile: Option<ReaderPublicationProfile>,
    page: String,
}
impl BrowserPreviewJob {
    pub(super) fn request_cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
    pub(super) fn drain_for_close(mut self) {
        self.request_cancel();
        if let Some(receiver) = self.receiver.take() {
            for message in receiver.iter() {
                drop(message);
            }
        }
    }

    fn next_message(&self) -> Result<Option<PreviewMessage>, String> {
        match self.receiver.as_ref().ok_or("预览通道已关闭")?.try_recv() {
            Ok(message) => Ok(Some(message)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("实际页面预览任务意外中断；未请求打开页面，请重试。".into())
            }
        }
    }
    fn matches_review(&self, state: &ReaderPublishState, baseline: &str) -> bool {
        self.baseline == baseline
            && self.selection == state.selection()
            && self.profile == state.current_profile()
            && state.reviewed.as_ref().is_some_and(|reviewed| {
                reviewed.preview.plan_digest == self.digest
                    && state.matches_review(reviewed, baseline)
            })
    }
}
impl Drop for BrowserPreviewJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(receiver) = self.receiver.take() {
            // 已排队Done可能持有大量文件的目录，不能在关闭窗口这一帧同步删除。
            close::spawn_cleanup(move || {
                for message in receiver.iter() {
                    drop(message);
                }
            });
        }
    }
}

fn private_directory() -> Result<PreviewDirectory, String> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "worldedit-reader-preview-{}-{nonce}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(&path)
        .map_err(|error| format!("无法创建私有预览目录：{error}"))?;
    Ok(PreviewDirectory(path))
}

fn materialize(
    files: &archive::Files,
    cancelled: &AtomicBool,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<PreviewDirectory, String> {
    crate::reader_zip::validate(files)?;
    let directory = private_directory()?;
    let total = files.values().map(Vec::len).sum();
    let mut completed = 0;
    for (relative, bytes) in files {
        if cancelled.load(Ordering::Acquire) {
            return Err("READER_CANCELLED：已取消临时预览。".into());
        }
        crate::reader_zip::safe_name(relative)?;
        let target = directory.0.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("预览目录创建失败：{error}"))?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|error| format!("临时预览文件创建失败：{error}"))?;
        for chunk in bytes.chunks(64 * 1024) {
            if cancelled.load(Ordering::Acquire) {
                return Err("READER_CANCELLED：已取消临时预览。".into());
            }
            file.write_all(chunk)
                .map_err(|error| format!("临时预览写入失败：{error}"))?;
            completed += chunk.len();
            progress(completed, total);
        }
    }
    if cancelled.load(Ordering::Acquire) {
        return Err("READER_CANCELLED：已取消临时预览。".into());
    }
    Ok(directory)
}

fn request_open(directory: &Path, page: &str) -> Result<(), String> {
    preview::validate_page_path(page)?;
    #[cfg(test)]
    {
        let _ = directory;
        OPEN_REQUESTS.with(|count| count.set(count.get() + 1));
        Ok(())
    }
    #[cfg(not(test))]
    {
        crate::media::open_reference(directory, &directory.join(page))
    }
}

impl WorldeditApp {
    pub(super) fn open_reader_browser_preview(&mut self, page: String) {
        if let Err(error) = preview::validate_page_path(&page) {
            self.reader_publish.status = Some(error);
            return;
        }
        let Some(reviewed) = self.reader_publish.reviewed.as_ref() else {
            return;
        };
        let baseline = self.project.content_baseline();
        if !self.reader_publish.matches_review(reviewed, &baseline) {
            self.reader_publish.invalidate_review();
            self.reader_publish.status = Some("临时页面审核已过期，请重新生成预览。".into());
            return;
        }
        if !reviewed
            .preview
            .content
            .iter()
            .any(|content| content.output_path == page)
        {
            self.reader_publish.status = Some("只能打开本次已审核的公开页面。".into());
            return;
        }
        let digest = reviewed.preview.plan_digest.clone();
        let cached = PREVIEWS.with(|previews| {
            previews
                .borrow()
                .get(&digest)
                .map(|directory| request_open(&directory.0, &page))
        });
        if let Some(result) = cached {
            self.reader_publish.status = Some(match result {
                Ok(()) => "已请求打开临时页面；不等于最终发布。".into(),
                Err(error) => format!("临时页面打开失败：{error}"),
            });
            return;
        }
        let files = reviewed.files.clone();
        let selection = reviewed.selection.clone();
        let profile = reviewed.profile.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut last = std::time::Instant::now();
            let result = materialize(&files, &worker_cancel, &mut |completed, total| {
                if completed == total || last.elapsed().as_millis() >= 100 {
                    let _ = sender.send(PreviewMessage::Stage(completed, total));
                    last = std::time::Instant::now();
                }
            });
            let _ = sender.send(PreviewMessage::Done(result));
        });
        self.reader_publish.browser_preview_job = Some(BrowserPreviewJob {
            cancel,
            receiver: Some(receiver),
            baseline,
            digest,
            selection,
            profile,
            page,
        });
        self.reader_publish.status =
            Some("正在后台物化已审核文件；可取消，尚未请求系统打开。".into());
    }

    pub(super) fn poll_reader_browser_preview(&mut self) {
        loop {
            let Some(job) = self.reader_publish.browser_preview_job.as_ref() else {
                return;
            };
            let message = match job.next_message() {
                Ok(Some(message)) => message,
                Ok(None) => return,
                Err(error) => {
                    self.reader_publish.browser_preview_job = None;
                    self.reader_publish.status = Some(error);
                    return;
                }
            };
            match message {
                PreviewMessage::Stage(completed, total) => {
                    self.reader_publish.status =
                        Some(format!("临时页面物化：{completed} / {total} B"))
                }
                PreviewMessage::Done(result) => {
                    let current =
                        job.matches_review(&self.reader_publish, &self.project.content_baseline());
                    let page = job.page.clone();
                    let digest = job.digest.clone();
                    self.reader_publish.browser_preview_job = None;
                    if !current || !self.reader_publish.open {
                        close::spawn_cleanup(move || drop(result));
                        self.reader_publish.status =
                            Some("临时预览已取消或审核过期，未请求打开。".into());
                        return;
                    }
                    match result {
                        Ok(directory) => {
                            let result = request_open(&directory.0, &page);
                            let previous = PREVIEWS
                                .with(|previews| previews.borrow_mut().insert(digest, directory));
                            if let Some(previous) = previous {
                                close::spawn_cleanup(move || drop(previous));
                            }
                            self.reader_publish.status = Some(match result {
                                Ok(()) => "已请求打开临时页面；关闭发布窗口不会立即移除资源，不等于最终发布。".into(),
                                Err(error) => format!("临时页面打开失败：{error}"),
                            });
                        }
                        Err(error) => self.reader_publish.status = Some(error),
                    }
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
