use super::{engine::Engine, protocol::*};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use worldline_core::{draft_rehearsal::DraftRehearsalRequest, project::Project};
use worldline_runtime::ReplayCancellation;

// 取消后编译器可能仍在退出；新请求在UI等待，不叠加不可抢占线程和Project副本。
static COMPILING: AtomicBool = AtomicBool::new(false);
struct CompilePermit;
impl Drop for CompilePermit {
    fn drop(&mut self) {
        COMPILING.store(false, Ordering::Release);
    }
}

pub(crate) struct SessionWorker {
    sender: Option<mpsc::Sender<Command>>,
    receiver: mpsc::Receiver<Result<Response, String>>,
    cancellation: ReplayCancellation,
    session_id: String,
    next: u64,
    expected: String,
    busy: bool,
}

impl SessionWorker {
    pub(crate) fn available() -> bool {
        !COMPILING.load(Ordering::Acquire)
    }

    pub(crate) fn start(
        project: &Project,
        input: &DraftRehearsalRequest,
        session_id: String,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        COMPILING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "上一份隔离编译仍在退出，请等待后重试")?;
        let permit = CompilePermit;
        let prepare = Prepare {
            schema_version: VERSION,
            session_id: session_id.clone(),
            request_id: "0".into(),
            input: input.clone(),
            entry: project
                .entry
                .strip_prefix(&project.root)
                .map_err(|_| "入口越界")?
                .into(),
            snapshot_state: project.snapshot_state()?,
        };
        let project = project.clone();
        let (sender, commands) = mpsc::channel();
        let (responses, receiver) = mpsc::channel();
        let cancellation = ReplayCancellation::new();
        let token = cancellation.clone();
        let context = ctx.clone();
        std::thread::spawn(move || {
            let result = Engine::prepare(&project, &prepare);
            drop(project);
            drop(permit);
            if token.is_cancelled() {
                return;
            }
            let mut engine = match result {
                Ok(engine) => engine,
                Err(error) => {
                    let _ = responses.send(Err(error));
                    context.request_repaint();
                    return;
                }
            };
            if send(&responses, engine.prepared(), &context).is_err() {
                return;
            }
            while let Ok(command) = commands.recv() {
                if token.is_cancelled() {
                    break;
                }
                let response = engine.execute(command, &token);
                if token.is_cancelled() || send(&responses, response, &context).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            sender: Some(sender),
            receiver,
            cancellation,
            session_id,
            next: 1,
            expected: "0".into(),
            busy: true,
        })
    }

    pub(crate) fn busy(&self) -> bool {
        self.busy
    }

    pub(crate) fn submit(&mut self, action: Action) -> Result<(), String> {
        if self.busy {
            return Err("上一条试演请求尚未返回".into());
        }
        let command = Command {
            schema_version: VERSION,
            session_id: self.session_id.clone(),
            request_id: self.next.to_string(),
            action,
        };
        command.validate()?;
        self.sender
            .as_ref()
            .ok_or("试演后台已关闭")?
            .send(command)
            .map_err(|_| "试演后台已退出")?;
        self.expected = self.next.to_string();
        self.next = self.next.checked_add(1).ok_or("试演请求代次耗尽")?;
        self.busy = true;
        Ok(())
    }

    pub(crate) fn poll(&mut self) -> Option<Result<Response, String>> {
        let response = match self.receiver.try_recv() {
            Ok(response) => response,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) if self.busy => Err("试演后台意外退出".into()),
            Err(_) => return None,
        };
        self.busy = false;
        Some(response.and_then(|response| {
            if response.schema_version != VERSION
                || response.session_id != self.session_id
                || response.request_id != self.expected
            {
                return Err("已拒绝不同会话或过期的试演结果".into());
            }
            Ok(response)
        }))
    }
}

fn send(
    sender: &mpsc::Sender<Result<Response, String>>,
    response: Response,
    ctx: &egui::Context,
) -> Result<(), ()> {
    let result = encode(&response).map(|_| response);
    let failed = result.is_err();
    sender.send(result).map_err(|_| ())?;
    ctx.request_repaint();
    if failed {
        Err(())
    } else {
        Ok(())
    }
}

impl Drop for SessionWorker {
    fn drop(&mut self) {
        self.cancellation.cancel();
        self.sender.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn held_compile_permit_prevents_another_project_clone_or_thread() {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            if COMPILING
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
            assert!(std::time::Instant::now() < until);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let permit = CompilePermit;
        assert!(!SessionWorker::available());
        let project = Project::new(&std::env::temp_dir().join("rehearsal-permit-no-clone"));
        let input = DraftRehearsalRequest {
            schema_version: 1,
            content_baseline: String::new(),
            drafts: vec![],
            excluded_inputs: vec![],
            composing: false,
        };
        // 若先克隆或处理DTO才取许可，这份故意无效请求会以其它原因失败。
        let error = SessionWorker::start(
            &project,
            &input,
            "blocked".into(),
            &egui::Context::default(),
        )
        .err()
        .expect("不能叠加线程");
        assert!(error.contains("上一份隔离编译"));
        drop(permit);
    }
}
