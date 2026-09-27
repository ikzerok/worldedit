use super::super::WorldeditApp;
use super::*;
use std::path::PathBuf;
use worldline_core::project::Project;

impl WorldeditApp {
    pub(super) fn start_reader_publish_preview(&mut self, selection: ReaderExportSelection) {
        self.reader_publish.invalidate_review();
        self.reader_publish.status = Some("正在按 core 公开清单生成静态阅读包……".into());
        #[cfg(not(target_arch = "wasm32"))]
        {
            let project = self.project.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let worker_cancel = cancel.clone();
            std::thread::spawn(move || {
                let result = build_reviewed_package_with_progress(
                    &project,
                    selection,
                    |stage| {
                        let _ = sender.send(ReaderPublishMessage::Stage(stage));
                    },
                    || worker_cancel.load(std::sync::atomic::Ordering::Acquire),
                );
                if !worker_cancel.load(std::sync::atomic::Ordering::Acquire) {
                    let _ = sender.send(ReaderPublishMessage::Done(Box::new(result)));
                }
            });
            self.reader_publish.job = Some(ReaderPublishJob { cancel, receiver });
        }
        #[cfg(target_arch = "wasm32")]
        {
            match build_reviewed_package(&self.project, selection) {
                Ok(reviewed) => {
                    self.reader_publish.status = Some("静态包已生成并逐文件核对。".into());
                    self.reader_publish.reviewed = Some(reviewed);
                }
                Err(error) => self.reader_publish.status = Some(format!("预览失败：{error}")),
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn poll_reader_publish_job(&mut self) {
        let Some(job) = self.reader_publish.job.as_ref() else {
            return;
        };
        let messages = job.receiver.try_iter().collect::<Vec<_>>();
        for message in messages {
            match message {
                ReaderPublishMessage::Stage(status) => {
                    self.reader_publish.status = Some(status.into());
                }
                ReaderPublishMessage::Done(result) => {
                    self.reader_publish.job = None;
                    match *result {
                        Ok(reviewed) => {
                            self.reader_publish.reviewed = Some(reviewed);
                            self.reader_publish.status =
                                Some("静态包预览已生成；尚未写入目标或启动下载。".into());
                        }
                        Err(error) => {
                            self.reader_publish.status = Some(format!("预览失败：{error}"));
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn poll_reader_publish_job(&mut self) {}
}
#[cfg(not(target_arch = "wasm32"))]
impl Drop for ReaderPublishJob {
    fn drop(&mut self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Release);
    }
}

#[cfg(target_arch = "wasm32")]
fn build_reviewed_package(
    project: &Project,
    selection: ReaderExportSelection,
) -> Result<ReviewedPackage, String> {
    build_reviewed_package_with_progress(project, selection, |_| {}, || false)
}

fn build_reviewed_package_with_progress(
    project: &Project,
    selection: ReaderExportSelection,
    mut report_progress: impl FnMut(&'static str),
    is_cancelled: impl Fn() -> bool,
) -> Result<ReviewedPackage, String> {
    report_progress("正在计算 core 预览与排除报告……");
    let preview = project.preview_reader_export(&selection)?;
    if is_cancelled() {
        return Err("阅读包预览已取消".into());
    }
    report_progress("正在构建仅包含已选内容的离线站点……");
    let files = project.build_reader_export(&selection, &preview.plan_digest)?;
    if is_cancelled() {
        return Err("阅读包预览已取消".into());
    }
    let raw_bytes = files.values().map(Vec::len).sum();
    report_progress("正在编码并逐文件核对 ZIP……");
    let zip = archive::encode(&files)?;
    let decoded = archive::decode(&zip)?;
    if is_cancelled() {
        return Err("阅读包预览已取消".into());
    }
    if decoded != files {
        return Err("静态包 ZIP 解包核对与预览文件不一致".into());
    }
    let search_index = files
        .get(&PathBuf::from("search-index.json"))
        .ok_or("静态包缺少公开搜索索引")?;
    let search_entries: Vec<SearchPreviewEntry> = serde_json::from_slice(search_index)
        .map_err(|error| format!("公开搜索索引格式无效：{error}"))?;
    let public_pages = search_entries
        .into_iter()
        .map(|entry| (entry.title, entry.url, entry.text))
        .collect();
    Ok(ReviewedPackage {
        selection,
        preview,
        files,
        zip,
        public_pages,
        raw_bytes,
    })
}

#[derive(serde::Deserialize)]
struct SearchPreviewEntry {
    title: String,
    url: String,
    text: String,
}
