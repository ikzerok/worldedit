//! 只在明确请求时构造完整快照；稳定帧仅比较 UI 代次，不重扫 AST。
use super::*;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(target_arch = "wasm32"))]
static BUSY: AtomicBool = AtomicBool::new(false);

pub(super) struct Job {
    input: Input,
    #[cfg(not(target_arch = "wasm32"))]
    receiver: std::sync::mpsc::Receiver<Result<ProductionScriptSnapshot, String>>,
    #[cfg(target_arch = "wasm32")]
    pending: Option<(
        worldline_core::project::Project,
        Vec<worldline_core::manuscript::WritingBuffer>,
    )>,
}
impl WorldeditApp {
    pub(in crate::app) fn begin_production_script(&mut self, ctx: &egui::Context) {
        let prepare = (|| {
            if let Some(error) = self.review_input_blocker(ctx) {
                return Err(error);
            }
            let input = self.production_input()?;
            if input.retained {
                return Err("请先处理受保护的未插入输入；未生成不完整的当前稿台本".into());
            }
            self.project.verify_review_navigation()?;
            Ok(input)
        })();
        let input = match prepare {
            Ok(input) => input,
            Err(error) => {
                self.manuscript.production.notice = Some(error);
                return;
            }
        };
        if self.manuscript.production.job.is_some() {
            return;
        }
        let buffers = self
            .manuscript
            .writing_buffers()
            .into_iter()
            .filter(|buffer| buffer.is_changed())
            .collect::<Vec<_>>();
        #[cfg(not(target_arch = "wasm32"))]
        let job = {
            if BUSY
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                .is_err()
            {
                self.manuscript.production.notice =
                    Some("前一个有界作业仍在结束，请稍后重试；未并行重复扫描".into());
                return;
            }
            let (sender, receiver) = std::sync::mpsc::channel();
            let project = self.project.clone();
            let request = input.request.clone();
            let drafts = input.drafts.clone();
            std::thread::spawn(move || {
                struct Release;
                impl Drop for Release {
                    fn drop(&mut self) {
                        BUSY.store(false, Ordering::Release);
                    }
                }
                let _release = Release;
                let result = project
                    .production_script_snapshot(&buffers, &drafts, &request)
                    .map_err(|error| error.to_string());
                let _ = sender.send(result);
            });
            Job { input, receiver }
        };
        #[cfg(target_arch = "wasm32")]
        let job = Job {
            input,
            pending: Some((self.project.clone(), buffers)),
        };
        let state = &mut self.manuscript.production;
        state.job = Some(job);
        state.confirmed = false;
        state.notice =
            Some("生成包含已进入正文缓冲的当前稿；按定义去重，条件和参数不执行。".into());
        ctx.request_repaint();
    }
    pub(in crate::app) fn poll_production_script(&mut self, ctx: &egui::Context) {
        if self.manuscript.production.job.is_none()
            || self.manuscript.production.polled_frame == Some(ctx.cumulative_frame_nr())
        {
            return;
        }
        self.manuscript.production.polled_frame = Some(ctx.cumulative_frame_nr());
        let current = self.production_input().ok();
        let state = &mut self.manuscript.production;
        if state
            .job
            .as_ref()
            .is_some_and(|job| Some(&job.input) != current.as_ref())
        {
            state.job = None;
            state.confirmed = false;
            state.notice =
                Some("本次生成已过期；稿件、范围或受保护输入已变化，请重新生成。".into());
            return;
        }
        let Some(job) = state.job.as_mut() else {
            return;
        };
        #[cfg(not(target_arch = "wasm32"))]
        let result = match job.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Some(Err("台本作业异常结束，没有新交付结果".into()))
            }
        };
        #[cfg(target_arch = "wasm32")]
        let result = job.pending.take().map(|(project, buffers)| {
            // 浏览器只在此明确作业执行有界同步 core；不宣称后台线程或持续磁盘观察。
            project
                .production_script_snapshot(&buffers, &job.input.drafts, &job.input.request)
                .map_err(|error| error.to_string())
        });
        match result {
            Some(result) => {
                let job = state.job.take().expect("当前生成作业存在");
                state.confirmed = false;
                match result {
                    Ok(snapshot) => {
                        state.offset = 0;
                        state.page = snapshot.page(0, 40).ok();
                        state.captured = Some(job.input);
                        state.snapshot = Some(Arc::new(snapshot));
                        state.artifact = None;
                        state.artifact_options = None;
                        state.notice =
                            Some("完整范围已生成；分页只改变显示，不缩小交付范围。".into());
                    }
                    Err(error) => {
                        state.captured = None;
                        state.notice = Some(error);
                    }
                }
            }
            None => ctx.request_repaint_after(std::time::Duration::from_millis(20)),
        }
    }
}
