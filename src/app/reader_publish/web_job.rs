//! Web只在同源Worker计算；结果复核后仍须显式确认，再后台复核并请求下载。
use super::super::WorldeditApp;
use super::*;
use crate::worker_protocol::{WorkEvent, WorkOutput, WorkRequest, WorkTask, SCHEMA_VERSION};

pub(super) struct WebReaderJob {
    worker: crate::worker_host::WorkerJob,
    generation: u64,
    baseline: String,
    selection: ReaderExportSelection,
    profile: Option<ReaderPublicationProfile>,
    final_digest: Option<String>,
}
impl Drop for WebReaderJob {
    fn drop(&mut self) {
        self.worker.cancel();
    }
}

impl WorldeditApp {
    pub(super) fn start_web_reader_job(
        &mut self,
        selection: ReaderExportSelection,
        final_digest: Option<String>,
        ctx: &egui::Context,
    ) {
        let result = (|| {
            let (snapshot_state, files, entry) = self.reader_worker_snapshot()?;
            let baseline = self.project.content_baseline();
            let generation = self.reader_publish.generation;
            let profile = self.reader_publish.current_profile();
            let request = WorkRequest {
                schema_version: SCHEMA_VERSION,
                job_id: format!(
                    "reader-{generation}-{}",
                    if final_digest.is_some() {
                        "delivery"
                    } else {
                        "preview"
                    }
                ),
                generation,
                baseline: baseline.clone(),
                entry,
                snapshot_state: Some(snapshot_state),
                task: WorkTask::ReaderPackage {
                    selection: selection.clone(),
                    profile: profile.clone(),
                },
            };
            let worker = crate::worker_host::WorkerJob::start(request, &files, ctx)?;
            Ok::<_, String>(WebReaderJob {
                worker,
                generation,
                baseline,
                selection,
                profile,
                final_digest,
            })
        })();
        match result {
            Ok(job) => {
                self.reader_publish.web_job = Some(job);
                self.reader_publish.status =
                    Some("同源后台任务正在复核并生成阅读包，可取消。".into());
            }
            Err(error) => self.reader_publish.status = Some(format!("后台任务未启动：{error}")),
        }
    }

    pub(super) fn poll_web_reader_job(&mut self) {
        loop {
            let Some(job) = self.reader_publish.web_job.as_mut() else {
                return;
            };
            let Some(event) = job.worker.take_event() else {
                return;
            };
            match event {
                WorkEvent::Progress {
                    stage,
                    completed,
                    total,
                } => self.reader_publish.status = Some(format!("{stage} · {completed} / {total}")),
                WorkEvent::Error(error) => {
                    self.reader_publish.web_job = None;
                    self.reader_publish.status = Some(format!("阅读包任务失败：{error}"));
                    return;
                }
                WorkEvent::Done { output, binaries } => {
                    let job = self
                        .reader_publish
                        .web_job
                        .take()
                        .expect("任务在当前帧存在");
                    if job.generation != self.reader_publish.generation
                        || job.baseline != self.project.content_baseline()
                        || job.selection != self.reader_publish.selection()
                        || job.profile != self.reader_publish.current_profile()
                    {
                        self.reader_publish.status =
                            Some("后台完成时审核基线或选择已变化；未下载。".into());
                        return;
                    }
                    let result = reviewed_from_worker(&job, *output, binaries);
                    match result {
                        Ok(reviewed) => {
                            if let Some(expected) = &job.final_digest {
                                let original_matches = self.reader_publish.confirmed
                                    && self.reader_publish.reviewed.as_ref().is_some_and(
                                        |original| {
                                            self.reader_publish
                                                .matches_review(original, &job.baseline)
                                                && original.preview.plan_digest == *expected
                                        },
                                    );
                                if !original_matches || reviewed.preview.plan_digest != *expected {
                                    self.reader_publish.confirmed = false;
                                    self.reader_publish.status =
                                        Some("最终复核与已确认审核不一致；未下载。".into());
                                    return;
                                }
                                self.reader_publish.status = Some(match crate::web::download("worldedit-reader-site.zip", &reviewed.zip, "application/zip") {
                                    Ok(()) => "浏览器下载已启动；最终保存位置由浏览器管理，工程保存状态未改变。".into(),
                                    Err(error) => format!("下载未启动：{error}"),
                                });
                                self.reader_publish.confirmed = false;
                            } else {
                                self.reader_publish.reviewed = Some(reviewed);
                                self.reader_publish.step = PublishStep::Resources;
                                self.reader_publish.status =
                                    Some("后台审核包已就绪；尚未请求下载。".into());
                            }
                        }
                        Err(error) => self.reader_publish.status = Some(error),
                    }
                    return;
                }
            }
        }
    }
}

fn reviewed_from_worker(
    job: &WebReaderJob,
    output: WorkOutput,
    mut binaries: Vec<Vec<u8>>,
) -> Result<ReviewedPackage, String> {
    let WorkOutput::ReaderPackage {
        preview,
        paths,
        raw_bytes,
    } = output
    else {
        return Err("后台返回了错误类型，未下载。".into());
    };
    if paths.len() > crate::reader_zip::MAX_FILES
        || binaries.len() != paths.len() + 1
        || preview.content_baseline != job.baseline
    {
        return Err("后台阅读包数量或审核基线不一致。".into());
    }
    let zip = binaries.pop().ok_or("后台缺少ZIP")?;
    if zip.len() > crate::reader_zip::MAX_BYTES {
        return Err("后台ZIP超过预算".into());
    }
    let count = paths.len();
    let files: archive::Files = paths.into_iter().zip(binaries).collect();
    if files.len() != count || crate::reader_zip::validate(&files)? != raw_bytes {
        return Err("后台文件路径重复或字节数不一致".into());
    }
    preview::validate_public_index(&files, &preview)?;
    Ok(ReviewedPackage {
        selection: job.selection.clone(),
        profile: job.profile.clone(),
        preview,
        files: std::sync::Arc::new(files),
        zip: std::sync::Arc::new(zip),
        raw_bytes,
    })
}
