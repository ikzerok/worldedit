use super::*;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use worldline_core::project::reconciliation::PreparedReconciliation;

#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // 浏览器没有磁盘冲突工作线程
pub(super) enum Outcome {
    Captured(Result<Box<ReconciliationSession>, String>),
    Preview(Result<Box<ReconciliationPlan>, String>),
    Prepared(Result<Box<PreparedReconciliation>, String>),
}

pub(super) enum Work {
    Capture,
    Preview(Box<(ReconciliationSession, ReconciliationRequest)>),
    Prepare(Box<ReconciliationPlan>),
}

#[cfg(not(target_arch = "wasm32"))]
static ACTIVE: AtomicBool = AtomicBool::new(false);

#[cfg(not(target_arch = "wasm32"))]
struct Slot;
#[cfg(not(target_arch = "wasm32"))]
impl Drop for Slot {
    fn drop(&mut self) {
        ACTIVE.store(false, Ordering::Release);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) struct Job {
    cancel: Arc<AtomicBool>,
    receiver: std::sync::mpsc::Receiver<Outcome>,
}
#[cfg(target_arch = "wasm32")]
pub(super) struct Job;

#[cfg(not(target_arch = "wasm32"))]
impl Job {
    pub(super) fn available() -> bool {
        !ACTIVE.load(Ordering::Acquire)
    }

    pub(super) fn start(
        project: &Project,
        work: impl FnOnce() -> Work,
        ctx: egui::Context,
    ) -> Result<Self, String> {
        ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "上一轮检查仍在停止，资源释放后再试；原候选保留")?;
        let slot = Slot;
        // 获得唯一槽位后才复制完整Project/会话；取消尚未收尾时无额外快照。
        let project = project.clone();
        let work = work();
        let cancel = Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("workspace-reconciliation".into())
            .spawn(move || {
                let _slot = slot;
                let mut progress = |_| !signal.load(Ordering::Relaxed);
                let outcome = match work {
                    Work::Capture => Outcome::Captured(
                        project
                            .capture_reconciliation_with_progress(&mut progress)
                            .map(Box::new),
                    ),
                    Work::Preview(input) => Outcome::Preview(
                        project
                            .preview_reconciliation_with_progress(&input.0, &input.1, &mut progress)
                            .map(Box::new),
                    ),
                    Work::Prepare(plan) => Outcome::Prepared(
                        project
                            .prepare_reconciliation_with_progress(&plan, &mut progress)
                            .map(Box::new),
                    ),
                };
                if !signal.load(Ordering::Relaxed) {
                    let _ = sender.send(outcome);
                }
                drop(project);
                ctx.request_repaint();
            })
            .map_err(|error| format!("无法启动外部改稿检查：{error}"))?;
        Ok(Self { cancel, receiver })
    }

    pub(super) fn poll(&self) -> Option<Outcome> {
        match self.receiver.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                Some(Outcome::Preview(Err("检查已停止，原候选保留".into())))
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(target_arch = "wasm32")]
impl Job {
    pub(super) fn available() -> bool {
        false
    }
    pub(super) fn start(
        _project: &Project,
        _work: impl FnOnce() -> Work,
        _ctx: egui::Context,
    ) -> Result<Self, String> {
        Err("浏览器为导入快照；请重新导入外改，不提供磁盘冲突会话".into())
    }
    pub(super) fn poll(&self) -> Option<Outcome> {
        None
    }
}
