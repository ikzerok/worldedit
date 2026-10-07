use super::super::WorldeditApp;
use super::*;
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use worldline_core::project::Project;

impl WorldeditApp {
    pub(super) fn start_reader_publish_preview(
        &mut self,
        selection: ReaderExportSelection,
        _ctx: &egui::Context,
    ) {
        self.reader_publish.invalidate_review();
        self.reader_publish.status = Some("正在按 core 公开清单生成静态阅读包……".into());
        #[cfg(not(target_arch = "wasm32"))]
        let profile = self.reader_publish.current_profile();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let project = self.project.clone();
            let generation = self.reader_publish.generation;
            let (sender, receiver) = std::sync::mpsc::channel();
            let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let worker_cancel = cancel.clone();
            std::thread::spawn(move || {
                let mut last = std::time::Instant::now();
                let mut last_stage = String::new();
                let result = build_reviewed_package_with_progress(
                    &project,
                    selection,
                    profile,
                    |stage, completed, total| {
                        if stage != last_stage
                            || completed == total
                            || last.elapsed().as_millis() >= 100
                        {
                            let _ = sender.send(ReaderPublishMessage::Stage(format!(
                                "{stage} · {completed} / {total}"
                            )));
                            last_stage = stage.into();
                            last = std::time::Instant::now();
                        }
                    },
                    || worker_cancel.load(std::sync::atomic::Ordering::Acquire),
                );
                if !worker_cancel.load(std::sync::atomic::Ordering::Acquire) {
                    let _ = sender.send(ReaderPublishMessage::Done(Box::new(result)));
                }
            });
            self.reader_publish.job = Some(ReaderPublishJob {
                cancel,
                receiver: Some(receiver),
                generation,
            });
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.start_web_reader_job(selection, None, _ctx);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn poll_reader_publish_job(&mut self) {
        loop {
            let Some(job) = self.reader_publish.job.as_ref() else {
                return;
            };
            let generation = job.generation;
            let Some(receiver) = job.receiver.as_ref() else {
                return;
            };
            let message = match receiver.try_recv() {
                Ok(message) => message,
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.reader_publish.job = None;
                    self.reader_publish.status =
                        Some("阅读包任务意外中断；未发布，请重试。".into());
                    return;
                }
            };
            match message {
                ReaderPublishMessage::Stage(status) => self.reader_publish.status = Some(status),
                ReaderPublishMessage::Done(result) => {
                    self.reader_publish.job = None;
                    match *result {
                        Ok(reviewed)
                            if generation == self.reader_publish.generation
                                && self.reader_publish.matches_review(
                                    &reviewed,
                                    &self.project.content_baseline(),
                                ) =>
                        {
                            self.reader_publish.page_directory =
                                page_directory::PageDirectory::default();
                            self.reader_publish.reviewed = Some(reviewed);
                            self.reader_publish.step = PublishStep::Resources;
                            self.reader_publish.status =
                                Some("静态包已逐文件核对；尚未写入目标或启动下载。".into());
                        }
                        Ok(_) => {
                            self.reader_publish.status =
                                Some("审核期间内容或选择已变化；结果已丢弃，请重新生成。".into())
                        }
                        Err(error) => {
                            self.reader_publish.status = Some(format!("预览失败：{error}"))
                        }
                    }
                    return;
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn poll_reader_publish_job(&mut self) {
        self.poll_web_reader_job();
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for ReaderPublishJob {
    fn drop(&mut self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Release);
        if let Some(receiver) = self.receiver.take() {
            close::spawn_cleanup(move || {
                for message in receiver.iter() {
                    drop(message);
                }
            });
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn build_reviewed_package_with_progress(
    project: &Project,
    selection: ReaderExportSelection,
    profile: Option<ReaderPublicationProfile>,
    mut report: impl FnMut(&str, usize, usize),
    is_cancelled: impl Fn() -> bool,
) -> Result<ReviewedPackage, String> {
    if profile
        .as_ref()
        .is_some_and(|profile| profile.selection != selection)
    {
        return Err("发布配置与本次选择不一致".into());
    }
    let mut progress = |value: &worldline_core::reader_export::ReaderExportProgress| {
        report(&value.phase, value.completed, value.total);
        !is_cancelled()
    };
    let preview = match &profile {
        Some(profile) => project.preview_reader_profile_with_progress(profile, &mut progress)?,
        None => project.preview_reader_export_with_progress(&selection, &mut progress)?,
    };
    let files = match &profile {
        Some(profile) => project.build_reader_profile_with_progress(
            profile,
            &preview.plan_digest,
            &mut progress,
        )?,
        None => project.build_reader_export_with_progress(
            &selection,
            &preview.plan_digest,
            &mut progress,
        )?,
    };
    let raw_bytes = crate::reader_zip::validate(&files)?;
    validate_public_index(&files, &preview)?;
    let zip = crate::reader_zip::encode(&files, &mut |completed, total| {
        report("ZIP编码与逐文件核对", completed, total);
        !is_cancelled()
    })?;
    Ok(ReviewedPackage {
        selection,
        profile,
        preview,
        files: std::sync::Arc::new(files),
        zip: std::sync::Arc::new(zip),
        raw_bytes,
    })
}

pub(super) fn validate_page_path(path: &str) -> Result<(), String> {
    // URL字符串不能套用Windows原生文件分隔符的宽容规则。
    if path.contains('\\') || crate::reader_zip::safe_name(Path::new(path))? != path {
        return Err("公开页面URL路径不安全".into());
    }
    Ok(())
}

pub(super) fn validate_public_index(
    files: &archive::Files,
    preview: &ReaderExportPreview,
) -> Result<(), String> {
    let bytes = files
        .get(Path::new("search-index.json"))
        .ok_or("静态包缺少公开搜索索引")?;
    let entries: Vec<SearchPreviewEntry> =
        serde_json::from_slice(bytes).map_err(|error| format!("公开搜索索引格式无效：{error}"))?;
    // v1缺少正文投影，只能跳过内容比对，绝不能跳过URL边界校验。
    for entry in &entries {
        let (base, anchor) = entry
            .url
            .split_once('#')
            .map_or((entry.url.as_str(), None), |(base, anchor)| {
                (base, Some(anchor))
            });
        validate_page_path(base)?;
        if anchor.is_some_and(|anchor| {
            anchor.is_empty()
                || !anchor
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        }) {
            return Err("公开搜索索引锚点路径不安全".into());
        }
    }
    // v1没有文本投影；v2/v3必须一一核对主页面。额外项仅允许来自已审核地图的公开锚点。
    if preview.content.is_empty() {
        return Ok(());
    }
    for page in &preview.content {
        validate_page_path(&page.output_path)?;
    }
    let pages: BTreeMap<_, _> = preview
        .content
        .iter()
        .map(|page| (page.output_path.as_str(), page))
        .collect();
    let map_pages: BTreeSet<_> = preview
        .included
        .iter()
        .filter(|item| {
            item.target
                .as_ref()
                .is_some_and(|target| target.kind == "map")
        })
        .map(|item| item.output_path.as_str())
        .collect();
    let mut seen = BTreeSet::new();
    let mut main_pages = BTreeSet::new();
    let mut anchor_offsets = BTreeMap::<&str, usize>::new();
    for entry in &entries {
        validate_page_path(
            entry
                .url
                .split_once('#')
                .map_or(entry.url.as_str(), |(base, _)| base),
        )?;
        if !seen.insert(entry.url.as_str()) {
            return Err("公开搜索索引含重复URL".into());
        }
        if let Some(page) = pages.get(entry.url.as_str()) {
            if entry.title != page.title || entry.text != page.text {
                return Err("实际阅读页与 core 预览内容不一致".into());
            }
            main_pages.insert(entry.url.as_str());
            continue;
        }
        let Some((base, anchor)) = entry.url.split_once('#') else {
            return Err("公开搜索索引出现未审核页面".into());
        };
        let valid_anchor = !anchor.is_empty()
            && anchor
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
        if entry.kind != "map_placement"
            || !map_pages.contains(base)
            || !valid_anchor
            || !entry.text.contains(&entry.title)
        {
            return Err("公开搜索索引锚点不属于已审核地图正文".into());
        }
        let page = pages.get(base).ok_or("公开搜索锚点没有已审核地图页")?;
        // core按地图正文顺序发出锚点；单向游标避免每个图元重扫整张地图正文。
        let offset = anchor_offsets.entry(base).or_default();
        let found = page
            .text
            .get(*offset..)
            .and_then(|text| text.find(&entry.text))
            .ok_or("公开搜索锚点文字与地图审核正文不一致")?;
        *offset += found + entry.text.len();
    }
    if main_pages.len() != pages.len() {
        return Err("公开搜索索引缺少已审核主页面".into());
    }

    Ok(())
}

#[derive(serde::Deserialize)]
struct SearchPreviewEntry {
    title: String,
    url: String,
    text: String,
    #[serde(default)]
    kind: String,
}

#[cfg(test)]
mod tests;
