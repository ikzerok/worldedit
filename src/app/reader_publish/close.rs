//! 全局退出等待后台清理完成；不能把“已发出取消”当作“可终止进程”。
use super::super::WorldeditApp;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc,
};

#[cfg(not(target_arch = "wasm32"))]
pub(super) struct CloseDrain {
    receiver: mpsc::Receiver<Result<Option<String>, String>>,
    failed: bool,
}

#[cfg(not(target_arch = "wasm32"))]
static DETACHED_CLEANUPS: AtomicUsize = AtomicUsize::new(0);

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn spawn_cleanup(work: impl FnOnce() + Send + 'static) {
    struct Ticket;
    impl Drop for Ticket {
        fn drop(&mut self) {
            DETACHED_CLEANUPS.fetch_sub(1, Ordering::Release);
        }
    }
    DETACHED_CLEANUPS.fetch_add(1, Ordering::AcqRel);
    std::thread::spawn(move || {
        let _ticket = Ticket;
        work();
    });
}

impl WorldeditApp {
    pub(in crate::app) fn reader_app_close_failed(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.reader_publish
                .close_drain
                .as_ref()
                .is_some_and(|drain| drain.failed)
        }
        #[cfg(target_arch = "wasm32")]
        {
            false
        }
    }

    pub(in crate::app) fn reader_app_close_pending(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.reader_publish.close_drain.is_some()
        }
        #[cfg(target_arch = "wasm32")]
        {
            false
        }
    }

    pub(in crate::app) fn poll_reader_app_close(&mut self, ctx: &egui::Context) -> bool {
        self.prepare_reader_app_close(ctx)
    }

    /// true才可关闭应用；false时调用者保留关闭意图，后续帧继续轮询。
    /// 不打开页面、不下载，也不会撤销已经跨过原子公布点的ZIP。
    pub(in crate::app) fn prepare_reader_app_close(&mut self, ctx: &egui::Context) -> bool {
        self.reader_publish.open = false;
        self.reader_publish.page_directory = super::page_directory::PageDirectory::default();
        #[cfg(target_arch = "wasm32")]
        {
            let _ = ctx;
            self.cancel_reader_publish()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(drain) = &self.reader_publish.close_drain {
                if drain.failed {
                    return false;
                }
                match drain.receiver.try_recv() {
                    Ok(Ok(message)) => {
                        self.reader_publish.close_drain = None;
                        if let Some(message) = message {
                            self.message = Some(message);
                        }
                        return true;
                    }
                    Ok(Err(error)) => {
                        if let Some(drain) = self.reader_publish.close_drain.as_mut() {
                            drain.failed = true;
                        }
                        self.io_error = Some(error);
                        return false;
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        ctx.request_repaint_after(std::time::Duration::from_millis(50));
                        return false;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        if let Some(drain) = self.reader_publish.close_drain.as_mut() {
                            drain.failed = true;
                        }
                        // 保留失败栅栏；下一帧不能因Option被清空而误判已经清理完成。
                        ctx.request_repaint_after(std::time::Duration::from_secs(1));
                        self.io_error =
                            Some("读者任务退出核对意外中断；请检查目标与临时目录后再关闭。".into());
                        return false;
                    }
                }
            }
            super::browser_preview::clear_cached_for_app_close();
            let profile = self.reader_publish.profile_job.take();
            let preview = self.reader_publish.job.take();
            let browser = self.reader_publish.browser_preview_job.take();
            let delivery = self.reader_publish.delivery_job.take();
            if profile.is_none()
                && preview.is_none()
                && browser.is_none()
                && delivery.is_none()
                && DETACHED_CLEANUPS.load(Ordering::Acquire) == 0
            {
                return true;
            }
            if let Some(job) = &profile {
                job.cancel.store(true, std::sync::atomic::Ordering::Release);
            }
            if let Some(job) = &preview {
                job.cancel.store(true, std::sync::atomic::Ordering::Release);
            }
            if let Some(job) = &browser {
                job.request_cancel();
            }
            if let Some(job) = &delivery {
                job.cancel();
            }
            let (sender, receiver) = mpsc::channel();
            std::thread::spawn(move || {
                if let Some(mut job) = profile {
                    if let Some(receiver) = job.receiver.take() {
                        for message in receiver.iter() {
                            drop(message);
                        }
                    }
                }
                if let Some(mut job) = preview {
                    if let Some(receiver) = job.receiver.take() {
                        for message in receiver.iter() {
                            drop(message);
                        }
                    }
                }
                if let Some(job) = browser {
                    job.drain_for_close();
                }
                let message = delivery.map(|job| job.drain_for_close()).transpose();
                // 包含先前关闭向导时已脱离state的清理，不能只等当前三个Option。
                while DETACHED_CLEANUPS.load(Ordering::Acquire) != 0 {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                let _ = sender.send(message);
            });
            self.reader_publish.close_drain = Some(CloseDrain {
                receiver,
                failed: false,
            });
            self.message =
                Some("正在停止读者任务并清理本次临时文件；进入原子提交的发布会等真实结果。".into());
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            false
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::app::reader_publish::{ReaderPublishJob, ReaderPublishState};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    fn app_frame(
        ctx: &egui::Context,
        app: &mut WorldeditApp,
        close_requested: bool,
    ) -> egui::FullOutput {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1188.0, 848.0),
            )),
            ..Default::default()
        };
        if close_requested {
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .events
                .push(egui::ViewportEvent::Close);
        }
        ctx.run(input, |ctx| {
            eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest())
        })
    }

    #[test]
    fn clean_project_os_close_waits_in_real_update_until_reader_worker_ends() {
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = WorldeditApp::new(&creation, None);
        let root = std::env::temp_dir().join(format!(
            "reader-close-main-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        app.project = worldline_core::project::Project::new(&root);
        app.active_file = app.project.entry.clone();
        app.project.save().unwrap();
        app.recompile();
        app.reader_publish = ReaderPublishState::new();
        app.reader_publish.open = true;
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        app.reader_publish.job = Some(ReaderPublishJob {
            cancel: cancel.clone(),
            receiver: Some(receiver),
            generation: 0,
        });
        assert!(!app.project.is_dirty());
        assert!(!app.has_open_authoring_form());
        let output = app_frame(&ctx, &mut app, true);
        let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
        assert!(commands.contains(&egui::ViewportCommand::CancelClose));
        assert!(!commands.contains(&egui::ViewportCommand::Close));
        assert!(app.reader_app_close_pending());
        assert!(cancel.load(Ordering::Acquire));
        assert!(!app.allow_close);
        let output = app_frame(&ctx, &mut app, false);
        assert!(!output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::Close));
        drop(sender);
        for _ in 0..1000 {
            let output = app_frame(&ctx, &mut app, false);
            if app.allow_close {
                assert!(output.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .contains(&egui::ViewportCommand::Close));
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(app.allow_close);
        assert!(!app.reader_publish.open);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn disconnected_global_cleanup_fails_closed_across_frames() {
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = WorldeditApp::new(&creation, None);
        let (sender, receiver) = mpsc::channel();
        drop(sender);
        app.reader_publish.close_drain = Some(CloseDrain {
            receiver,
            failed: false,
        });
        for _ in 0..2 {
            assert!(!app.poll_reader_app_close(&ctx));
            assert!(app.reader_app_close_pending());
            assert!(app.reader_app_close_failed());
            assert!(app.io_error.as_ref().unwrap().contains("意外中断"));
        }
    }

    #[test]
    fn app_close_waits_for_cancelled_worker_to_finish_and_never_consumes_its_output() {
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = WorldeditApp::new(&creation, None);
        app.reader_publish = ReaderPublishState::new();
        app.reader_publish.open = true;
        let cancel = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::channel();
        app.reader_publish.job = Some(ReaderPublishJob {
            cancel: cancel.clone(),
            receiver: Some(receiver),
            generation: 0,
        });
        assert!(!app.prepare_reader_app_close(&ctx));
        assert!(cancel.load(Ordering::Acquire));
        assert!(!app.reader_publish.open);
        assert!(
            !app.prepare_reader_app_close(&ctx),
            "发送端还活着，不能提前允许进程退出"
        );
        // 正常向导帧/取消也不能丢掉全局清理栅栏。
        assert!(!app.cancel_reader_publish());
        drop(sender);
        let mut ready = false;
        for _ in 0..200 {
            if app.prepare_reader_app_close(&ctx) {
                ready = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(ready);
        assert!(app.reader_publish.close_drain.is_none());
    }
}
