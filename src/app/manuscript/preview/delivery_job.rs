//! 原生后台逐章作业；浏览器复用同一core有界逐步作业。
use super::delivery::Input;
use crate::app::WorldeditApp;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
#[cfg(not(target_arch = "wasm32"))]
use worldline_core::manuscript::ManuscriptDeliveryReport;
use worldline_core::manuscript::{ManuscriptDeliveryProgress, ManuscriptDeliverySnapshot};

pub(super) struct Job {
    input: Input,
    cancel: Arc<AtomicBool>,
    pub progress: Option<ManuscriptDeliveryProgress>,
    #[cfg(not(target_arch = "wasm32"))]
    receiver: std::sync::mpsc::Receiver<Message>,
    #[cfg(target_arch = "wasm32")]
    session: worldline_core::manuscript::ManuscriptDeliveryJob,
}
#[cfg(not(target_arch = "wasm32"))]
enum Message {
    Progress(ManuscriptDeliveryProgress),
    Done(Box<Result<ManuscriptDeliveryReport, String>>),
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl WorldeditApp {
    pub(in crate::app) fn begin_manuscript_delivery(&mut self, ctx: &egui::Context) {
        if self.manuscript.preview_cache.delivery.job.is_some() {
            return;
        }
        let prepared = (|| {
            if let Some(error) = self.review_input_blocker(ctx) {
                return Err(error);
            }
            self.project.verify_review_navigation()?;
            let query = self.manuscript_delivery_snapshot()?;
            self.project
                .verify_manuscript_delivery_observation(&query)
                .map_err(|error| error.to_string())?;
            let input = self
                .manuscript_delivery_input()
                .ok_or("当前书稿筛选不可用")?;
            Ok((query, input))
        })();
        let (query, input) = match prepared {
            Ok(value) => value,
            Err(error) => {
                self.manuscript.preview_cache.delivery.notice = Some(error);
                return;
            }
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let excluded_inputs = self
            .unapplied_export_inputs()
            .into_iter()
            .filter(|input| {
                !(input.kind == "书稿 / 正文草稿"
                    && (input.source.starts_with("正文 · ")
                        || input.source.starts_with("书稿编排 · ")))
            })
            .map(|input| format!("{} · {}", input.kind, input.source))
            .collect();
        #[cfg(not(target_arch = "wasm32"))]
        let job = {
            let (sender, receiver) = std::sync::mpsc::channel();
            let token = cancel.clone();
            let request = input.request.clone();
            std::thread::spawn(move || {
                let result = ManuscriptDeliverySnapshot::new(query, &request)
                    .and_then(|snapshot| {
                        worldline_core::manuscript::generate_manuscript_delivery(
                            snapshot,
                            &mut |progress| {
                                let _ = sender.send(Message::Progress(progress.clone()));
                                !token.load(Ordering::Relaxed)
                            },
                        )
                    })
                    .map_err(|error| error.to_string());
                let _ = sender.send(Message::Done(Box::new(result)));
            });
            Job {
                input,
                cancel,
                progress: None,
                receiver,
            }
        };
        #[cfg(target_arch = "wasm32")]
        let job = {
            let session = ManuscriptDeliverySnapshot::new(query, &input.request)
                .and_then(worldline_core::manuscript::ManuscriptDeliveryJob::new);
            let session = match session {
                Ok(session) => session,
                Err(error) => {
                    self.manuscript.preview_cache.delivery.notice = Some(error.to_string());
                    return;
                }
            };
            Job {
                input,
                cancel,
                progress: None,
                session,
            }
        };
        let state = &mut self.manuscript.preview_cache.delivery;
        state.job = Some(job);
        state.confirmed = false;
        state.excluded_inputs = excluded_inputs;
        state.notice = None;
        ctx.request_repaint();
    }

    pub(in crate::app) fn cancel_manuscript_delivery(&mut self) {
        let state = &mut self.manuscript.preview_cache.delivery;
        if state.job.take().is_some() {
            state.confirmed = false;
            state.notice = Some("已取消本次生成，没有新交付产物；草稿与原位置保留".into());
        }
    }

    pub(in crate::app) fn poll_manuscript_delivery(&mut self, ctx: &egui::Context) {
        let current = self.manuscript_delivery_input();
        let previous_scroll = self.manuscript.review_scroll_y;
        let state = &mut self.manuscript.preview_cache.delivery;
        if state
            .job
            .as_ref()
            .is_some_and(|job| Some(&job.input) != current.as_ref())
        {
            state.job = None;
            state.confirmed = false;
            state.notice =
                Some("生成已过期：查询、书稿或正文变化；请重新生成。旧证据保留，未交付".into());
            return;
        }
        let Some(job) = state.job.as_mut() else {
            return;
        };
        #[cfg(not(target_arch = "wasm32"))]
        let finished = {
            let mut finished = None;
            loop {
                match job.receiver.try_recv() {
                    Ok(Message::Progress(progress)) => job.progress = Some(progress),
                    Ok(Message::Done(result)) => {
                        finished = Some(*result);
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        finished = Some(Err("审稿后台任务异常结束；没有生成产物".into()));
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                }
            }
            finished
        };
        #[cfg(target_arch = "wasm32")]
        let finished = {
            if job.cancel.load(Ordering::Relaxed) {
                job.session.cancel();
            }
            job.progress = Some(job.session.progress());
            match job.session.advance(1) {
                Ok(Some(report)) => Some(Ok(report)),
                Ok(None) => None,
                Err(error) => Some(Err(error.to_string())),
            }
        };
        if let Some(result) = finished {
            let job = state.job.take().expect("完成消息属于正在进行的作业");
            match result {
                Ok(report) => {
                    let same_range = state.captured.as_ref().is_some_and(|old| {
                        old.request.query == job.input.request.query
                            && old.request.chapter_ids == job.input.request.chapter_ids
                    });
                    let anchor = same_range
                        .then(|| state.reviewed.as_ref()?.scope().chapters.get(state.page))
                        .flatten();
                    let restored =
                        anchor.and_then(|old| {
                            report.scope().chapters.iter().position(|row| {
                                row.entry.id == old.entry.id && !row.identity_ambiguous
                            })
                        });
                    let pages = super::delivery_view::bounded_pages(&report);
                    state.page = restored
                        .and_then(|position| pages.iter().find(|page| page.contains(&position)))
                        .map_or(0, |page| page.start);
                    state.review_pages = pages;
                    state.notice = Some(
                        if report.complete() {
                            "同范围完整审稿已生成；Markdown可在下方逐字预览"
                        } else {
                            "范围或章节不完整；请核对逐章错误，Markdown交付已停用"
                        }
                        .into(),
                    );
                    state.reviewed = Some(Arc::new(report));
                    state.captured = Some(job.input.clone());
                    state.confirmed = false;
                    if !same_range {
                        state.markdown_page = 0;
                    }
                    self.manuscript.pending_review_scroll = Some(if restored.is_some() {
                        previous_scroll
                    } else {
                        0.0
                    });
                }
                Err(error) => state.notice = Some(error),
            }
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
}
