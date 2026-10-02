//! 最终ZIP后台复核与同目录无覆盖公布；取消不删除已有目标。
use super::super::WorldeditApp;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU8, Ordering},
    mpsc, Arc,
};

const ACTIVE: u8 = 0;
const CANCELLED: u8 = 1;
const COMMITTING: u8 = 2;
const COMPLETE: u8 = 3;
const CHUNK: usize = 64 * 1024;

pub(super) struct NativeDeliveryJob {
    state: Arc<AtomicU8>,
    receiver: Option<mpsc::Receiver<DeliveryMessage>>,
    baseline: String,
}
impl NativeDeliveryJob {
    pub(super) fn drain_for_close(mut self) -> Result<String, String> {
        self.cancel();
        let mut completion = None;
        let receiver = self
            .receiver
            .take()
            .ok_or("原生发布回执通道缺失，无法确认提交结果")?;
        for message in receiver.iter() {
            if let DeliveryMessage::Done(result) = message {
                completion = Some(match result {
                    Ok(path) => {
                        format!("阅读包已写入 {}；关闭没有撤销已完成发布。", path.display())
                    }
                    Err(error) if error.contains("READER_CANCELLED") => {
                        "读者发布已取消，本次暂存文件已结束清理。".into()
                    }
                    Err(error) => format!("读者发布结束：{error}"),
                });
            }
        }
        completion.ok_or_else(|| {
            "原生发布线程已中断且没有完成回执；请检查目标与暂存文件，尚不能确认取消或发布结果。"
                .into()
        })
    }

    pub(super) fn cancel(&self) -> bool {
        self.state
            .compare_exchange(ACTIVE, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
            || self.state.load(Ordering::Acquire) == CANCELLED
    }
}
impl Drop for NativeDeliveryJob {
    fn drop(&mut self) {
        self.cancel();
        if let Some(receiver) = self.receiver.take() {
            super::close::spawn_cleanup(move || {
                for message in receiver.iter() {
                    drop(message);
                }
            });
        }
    }
}

enum DeliveryMessage {
    Stage(String),
    Done(Result<PathBuf, String>),
}

struct TemporaryFile(PathBuf);
impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn publish_atomic(
    root: &Path,
    target: &Path,
    bytes: &[u8],
    state: &AtomicU8,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<PathBuf, String> {
    if !target.is_absolute()
        || target
            .extension()
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("zip"))
    {
        return Err("请输入工作区外新ZIP文件的完整绝对路径。".into());
    }
    let root = super::super::package::normalized_path(root);
    let target = super::super::package::normalized_path(target);
    if super::super::package::is_same_or_descendant(&root, &target) {
        return Err("阅读包目标必须在当前工作区外。".into());
    }
    if target.exists() {
        return Err("目标ZIP已存在，未覆盖。".into());
    }
    let parent = target.parent().ok_or("目标ZIP没有有效父目录")?;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let temporary_path = parent.join(format!(
        ".worldedit-reader-{}-{nonce}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    if state.load(Ordering::Acquire) != ACTIVE {
        return Err("READER_CANCELLED：发布已取消，未创建目标。".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)
        .map_err(|error| format!("无法创建本次临时ZIP：{error}"))?;
    // 仅在create_new成功后取得清理所有权；碰撞时不能删除别人的临时文件。
    let temp = TemporaryFile(temporary_path);
    let write_result = (|| -> Result<(), String> {
        for (index, chunk) in bytes.chunks(CHUNK).enumerate() {
            if state.load(Ordering::Acquire) != ACTIVE {
                return Err("READER_CANCELLED：发布已取消，未创建目标。".into());
            }
            file.write_all(chunk)
                .map_err(|error| format!("写入临时ZIP失败：{error}"))?;
            progress((index * CHUNK + chunk.len()).min(bytes.len()), bytes.len());
        }
        file.flush()
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("同步临时ZIP失败：{error}"))?;
        Ok(())
    })();
    drop(file); // Windows必须在清理guard之前关闭句柄。
    write_result?;
    state
        .compare_exchange(ACTIVE, COMMITTING, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "READER_CANCELLED：发布已取消，未创建目标。".to_owned())?;
    // hard_link是无覆盖原子创建；绝不使用会替换已有文件的rename。
    let result = fs::hard_link(&temp.0, &target)
        .map_err(|error| format!("无法原子公布ZIP（未覆盖旧目标）：{error}"));
    state.store(COMPLETE, Ordering::Release);
    result.map(|()| target)
}

impl WorldeditApp {
    pub(super) fn start_native_reader_delivery(&mut self) {
        let Some(reviewed) = self.reader_publish.reviewed.as_ref() else {
            return;
        };
        let project = self.project.clone();
        let baseline = project.content_baseline();
        let selection = reviewed.selection.clone();
        let profile = reviewed.profile.clone();
        let digest = reviewed.preview.plan_digest.clone();
        let zip = reviewed.zip.clone();
        let target = PathBuf::from(self.reader_publish.destination.trim());
        let state = Arc::new(AtomicU8::new(ACTIVE));
        let worker_state = state.clone();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| {
                let mut last = std::time::Instant::now();
                let mut progress = |value: &worldline_core::reader_export::ReaderExportProgress| {
                    if value.completed == value.total || last.elapsed().as_millis() >= 100 {
                        let _ = sender.send(DeliveryMessage::Stage(format!(
                            "最终复核 {} · {} / {}",
                            value.phase, value.completed, value.total
                        )));
                        last = std::time::Instant::now();
                    }
                    worker_state.load(Ordering::Acquire) == ACTIVE
                };
                let fresh = match profile {
                    Some(profile) => {
                        project.preview_reader_profile_with_progress(&profile, &mut progress)?
                    }
                    None => {
                        project.preview_reader_export_with_progress(&selection, &mut progress)?
                    }
                };
                if fresh.plan_digest != digest {
                    return Err("审核已过期，请重新预览并确认。".into());
                }
                publish_atomic(
                    &project.root,
                    &target,
                    &zip,
                    &worker_state,
                    &mut |completed, total| {
                        if completed == total || last.elapsed().as_millis() >= 100 {
                            let _ = sender.send(DeliveryMessage::Stage(format!(
                                "写入本次临时ZIP · {completed} / {total} B"
                            )));
                            last = std::time::Instant::now();
                        }
                    },
                )
            })();
            let _ = sender.send(DeliveryMessage::Done(result));
        });
        self.reader_publish.delivery_job = Some(NativeDeliveryJob {
            state,
            receiver: Some(receiver),
            baseline,
        });
        self.reader_publish.status = Some("后台最终复核并写入临时ZIP；原子公布前可取消。".into());
    }

    pub(super) fn poll_native_reader_delivery(&mut self) {
        let Some(job) = self.reader_publish.delivery_job.as_ref() else {
            return;
        };
        if job.baseline != self.project.content_baseline() {
            job.cancel();
        }
        let Some(receiver) = job.receiver.as_ref() else {
            return;
        };
        let result = loop {
            match receiver.try_recv() {
                Ok(DeliveryMessage::Stage(status)) => self.reader_publish.status = Some(status),
                Ok(DeliveryMessage::Done(result)) => break result,
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => {
                    break Err("发布任务意外中断；请检查目标并重新审核，未宣称取消成功。".into())
                }
            }
        };
        self.reader_publish.delivery_job = None;
        let message = match result {
            Ok(path) => format!("阅读包已写入 {}；工程保存状态未改变。", path.display()),
            Err(error) if error.contains("READER_CANCELLED") => {
                "阅读包发布已取消；未创建目标。".into()
            }
            Err(error) => format!("发布失败：{error}"),
        };
        self.reader_publish.status = Some(message.clone());
        self.message = Some(message);
        self.reader_publish.confirmed = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn paths(label: &str) -> (PathBuf, PathBuf) {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let parent = std::env::temp_dir().join(format!(
            "reader-delivery-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(parent.join("project")).unwrap();
        (parent.join("project"), parent.join("reader.zip"))
    }
    #[test]
    fn cancelled_delivery_never_creates_target_or_temporary_files() {
        let (root, target) = paths("cancel");
        assert!(publish_atomic(
            &root,
            &target,
            b"zip",
            &AtomicU8::new(CANCELLED),
            &mut |_, _| {}
        )
        .is_err());
        assert!(!target.exists());
        assert_eq!(fs::read_dir(root.parent().unwrap()).unwrap().count(), 1);
        fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }
    #[test]
    fn existing_target_is_never_overwritten() {
        let (root, target) = paths("existing");
        fs::write(&target, b"original").unwrap();
        assert!(publish_atomic(
            &root,
            &target,
            b"replacement",
            &AtomicU8::new(ACTIVE),
            &mut |_, _| {}
        )
        .is_err());
        assert_eq!(fs::read(&target).unwrap(), b"original");
        fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }
    #[test]
    fn completed_atomic_commit_is_not_reported_as_cancelled() {
        let (root, target) = paths("done");
        let state = AtomicU8::new(ACTIVE);
        assert_eq!(
            publish_atomic(&root, &target, b"complete", &state, &mut |_, _| {}).unwrap(),
            target
        );
        assert_eq!(state.load(Ordering::Acquire), COMPLETE);
        assert!(state
            .compare_exchange(ACTIVE, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
            .is_err());
        assert_eq!(fs::read(&target).unwrap(), b"complete");
        assert_eq!(fs::read_dir(root.parent().unwrap()).unwrap().count(), 2);
        fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }
    #[test]
    fn cancellation_during_chunks_removes_only_our_temporary_file() {
        let (root, target) = paths("during-write");
        let state = AtomicU8::new(ACTIVE);
        let result = publish_atomic(&root, &target, &vec![1; CHUNK * 3], &state, &mut |_, _| {
            state.store(CANCELLED, Ordering::Release);
        });
        assert!(result.unwrap_err().contains("READER_CANCELLED"));
        assert!(!target.exists());
        assert_eq!(fs::read_dir(root.parent().unwrap()).unwrap().count(), 1);
        fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }

    #[test]
    fn target_created_during_staging_wins_without_being_overwritten() {
        let (root, target) = paths("target-race");
        let result = publish_atomic(
            &root,
            &target,
            b"new package",
            &AtomicU8::new(ACTIVE),
            &mut |completed, total| {
                if completed == total {
                    fs::write(&target, b"another completed export").unwrap();
                }
            },
        );
        assert!(result.is_err());
        assert_eq!(fs::read(&target).unwrap(), b"another completed export");
        assert_eq!(fs::read_dir(root.parent().unwrap()).unwrap().count(), 2);
        fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }
    fn closing_app() -> (egui::Context, WorldeditApp) {
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        (ctx, WorldeditApp::new(&creation, None))
    }

    fn wait_close(ctx: &egui::Context, app: &mut WorldeditApp) {
        for _ in 0..1000 {
            if app.poll_reader_app_close(ctx) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("退出等待器未完成");
    }

    #[test]
    fn global_close_waits_for_a_completed_atomic_receipt_and_keeps_the_target() {
        let (ctx, mut app) = closing_app();
        let (root, target) = paths("close-completed");
        let state = Arc::new(AtomicU8::new(ACTIVE));
        publish_atomic(&root, &target, b"published", &state, &mut |_, _| {}).unwrap();
        let (sender, receiver) = mpsc::channel();
        assert!(sender
            .send(DeliveryMessage::Done(Ok(target.clone())))
            .is_ok());
        app.reader_publish.delivery_job = Some(NativeDeliveryJob {
            state,
            receiver: Some(receiver),
            baseline: app.project.content_baseline(),
        });
        app.perform_action(crate::app::Pending::Close, &ctx);
        assert!(app.reader_app_close_pending());
        assert!(!app.allow_close);
        assert!(app.reader_app_close_pending());
        drop(sender);
        wait_close(&ctx, &mut app);
        assert_eq!(fs::read(&target).unwrap(), b"published");
        assert!(app.message.as_ref().unwrap().contains("已写入"));
        fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }

    #[test]
    fn global_close_cancels_an_active_write_and_waits_for_its_stage_cleanup() {
        let (ctx, mut app) = closing_app();
        let (root, target) = paths("close-writing");
        let state = Arc::new(AtomicU8::new(ACTIVE));
        let worker_state = state.clone();
        let worker_root = root.clone();
        let worker_target = target.clone();
        let (sender, receiver) = mpsc::channel();
        let (entered, at_chunk) = mpsc::channel();
        let (resume, paused) = mpsc::channel();
        std::thread::spawn(move || {
            let result = publish_atomic(
                &worker_root,
                &worker_target,
                &vec![1; CHUNK * 3],
                &worker_state,
                &mut |completed, _| {
                    if completed == CHUNK {
                        entered.send(()).unwrap();
                        paused.recv().unwrap();
                    }
                },
            );
            let _ = sender.send(DeliveryMessage::Done(result));
        });
        app.reader_publish.delivery_job = Some(NativeDeliveryJob {
            state: state.clone(),
            receiver: Some(receiver),
            baseline: app.project.content_baseline(),
        });
        at_chunk
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        app.perform_action(crate::app::Pending::Close, &ctx);
        assert!(app.reader_app_close_pending());
        assert!(!app.allow_close);
        assert_eq!(state.load(Ordering::Acquire), CANCELLED);
        assert!(!target.exists());
        resume.send(()).unwrap();
        wait_close(&ctx, &mut app);
        assert!(!target.exists());
        assert_eq!(fs::read_dir(root.parent().unwrap()).unwrap().count(), 1);
        assert!(app.message.as_ref().unwrap().contains("已取消"));
        fs::remove_dir_all(root.parent().unwrap()).unwrap();
    }

    #[test]
    fn global_close_requires_a_real_atomic_delivery_receipt() {
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = crate::app::WorldeditApp::new(&creation, None);
        let (sender, receiver) = mpsc::channel();
        drop(sender);
        app.reader_publish.delivery_job = Some(NativeDeliveryJob {
            state: Arc::new(AtomicU8::new(COMMITTING)),
            receiver: Some(receiver),
            baseline: app.project.content_baseline(),
        });
        app.perform_action(crate::app::Pending::Close, &ctx);
        for _ in 0..1000 {
            assert!(!app.poll_reader_app_close(&ctx));
            if app.reader_app_close_failed() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(app.reader_app_close_failed());
        assert!(!app.allow_close);
        assert!(app.io_error.as_ref().unwrap().contains("没有完成回执"));
        assert!(!app.poll_reader_app_close(&ctx));
    }
}
