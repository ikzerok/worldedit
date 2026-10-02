//! 发布配置只在后台生成计划；返回主线程后仍调用core原有完整重验应用。
use super::super::WorldeditApp;
use super::*;
use worldline_core::reader_export::ReaderProfileSavePlan;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ProfileInput {
    pub(super) generation: u64,
    pub(super) baseline: String,
    pub(super) selection: ReaderExportSelection,
    pub(super) profile: Option<ReaderPublicationProfile>,
    pub(super) id: String,
    pub(super) title: String,
}

pub(super) struct ProfileJob {
    pub(super) input: ProfileInput,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) receiver: Option<std::sync::mpsc::Receiver<ProfileMessage>>,
    #[cfg(target_arch = "wasm32")]
    pub(super) worker: crate::worker_host::WorkerJob,
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) enum ProfileMessage {
    Stage(String),
    Done(Box<Result<ReaderProfileSavePlan, String>>),
}

impl Drop for ProfileJob {
    fn drop(&mut self) {
        #[cfg(target_arch = "wasm32")]
        self.worker.cancel();
        #[cfg(not(target_arch = "wasm32"))]
        {
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
}

impl WorldeditApp {
    pub(super) fn reader_profile_input(&self) -> ProfileInput {
        let state = &self.reader_publish;
        let selection = state.selection();
        let title = if state.profile_title.trim().is_empty() {
            selection.site_title.clone()
        } else {
            state.profile_title.clone()
        };
        let profile = state.profile.clone().map(|mut profile| {
            profile.selection = selection.clone();
            profile.title = state.profile_title.clone();
            profile
        });
        ProfileInput {
            generation: state.generation,
            baseline: self.project.content_baseline(),
            selection,
            profile,
            id: state.profile_id.trim().into(),
            title,
        }
    }

    pub(super) fn start_reader_profile_save(&mut self, ctx: &egui::Context) {
        if self.reader_publish.busy() {
            return;
        }
        let input = self.reader_profile_input();
        if self
            .reader_publish
            .profile_plan
            .as_ref()
            .is_some_and(|(old, _)| old == &input)
        {
            self.apply_reader_profile_plan();
            return;
        }
        self.reader_publish.profile_plan = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let project = self.project.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let worker_cancel = cancel.clone();
            let worker_input = input.clone();
            let worker_ctx = ctx.clone();
            std::thread::spawn(move || {
                let cancelled = || worker_cancel.load(std::sync::atomic::Ordering::Acquire);
                let result = (|| {
                    if cancelled() {
                        return Err("配置任务已取消".into());
                    }
                    let _ = sender.send(ProfileMessage::Stage("正在检查发布配置 · 1 / 2".into()));
                    worker_ctx.request_repaint();
                    let mut profile = match worker_input.profile.clone() {
                        Some(profile) => profile,
                        None => project
                            .create_reader_profile(&worker_input.id, &worker_input.selection)?,
                    };
                    if cancelled() {
                        return Err("配置任务已取消".into());
                    }
                    profile.title = worker_input.title.clone();
                    profile.selection = worker_input.selection.clone();
                    let _ =
                        sender.send(ProfileMessage::Stage("正在生成配置保存计划 · 2 / 2".into()));
                    worker_ctx.request_repaint();
                    project.preview_save_reader_profile(&profile)
                })();
                if !cancelled() {
                    let _ = sender.send(ProfileMessage::Done(Box::new(result)));
                }
                worker_ctx.request_repaint();
            });
            self.reader_publish.profile_job = Some(ProfileJob {
                input,
                cancel,
                receiver: Some(receiver),
            });
        }
        #[cfg(target_arch = "wasm32")]
        if let Err(error) = self.start_web_profile_plan(input, ctx) {
            self.reader_publish.status = Some(format!("配置后台任务未启动：{error}"));
            return;
        }
        self.reader_publish.status =
            Some("正在后台核对发布配置；可以取消，尚未应用或保存。".into());
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn poll_reader_profile_job(&mut self) {
        loop {
            let Some(job) = &self.reader_publish.profile_job else {
                return;
            };
            let Some(receiver) = &job.receiver else {
                return;
            };
            let message = match receiver.try_recv() {
                Ok(message) => message,
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.reader_publish.profile_job = None;
                    self.reader_publish.status = Some("配置后台任务已中断；未应用。".into());
                    return;
                }
            };
            match message {
                ProfileMessage::Stage(stage) => self.reader_publish.status = Some(stage),
                ProfileMessage::Done(result) => {
                    let job = self
                        .reader_publish
                        .profile_job
                        .take()
                        .expect("本帧任务存在");
                    self.accept_reader_profile_plan(job.input.clone(), *result);
                    return;
                }
            }
        }
    }

    pub(super) fn accept_reader_profile_plan(
        &mut self,
        input: ProfileInput,
        result: Result<ReaderProfileSavePlan, String>,
    ) {
        if input != self.reader_profile_input() {
            self.reader_publish.status =
                Some("配置或工程在后台核对期间改变；未应用，请重新核对。".into());
            return;
        }
        match result {
            Ok(plan) if plan_matches(&input, &plan) => {
                self.reader_publish.profile_plan = Some((input, plan));
                self.apply_reader_profile_plan();
            }
            Ok(_) => self.reader_publish.status = Some("后台配置计划与请求不一致；未应用。".into()),
            Err(error) => self.reader_publish.status = Some(format!("配置未应用：{error}")),
        }
    }
}

pub(super) fn plan_matches(input: &ProfileInput, plan: &ReaderProfileSavePlan) -> bool {
    if plan.content_baseline != input.baseline
        || plan.profile.id != input.id
        || plan.profile.title != input.title
        || plan.profile.selection != input.selection
    {
        return false;
    }
    input.profile.as_ref().is_none_or(|old| {
        let routes: BTreeSet<_> = plan.profile.routes.iter().map(route_key).collect();
        old.id == plan.profile.id
            && old.schema_version == plan.profile.schema_version
            && old.required_features == plan.profile.required_features
            && old
                .routes
                .iter()
                .all(|route| routes.contains(&route_key(route)))
    })
}

type RouteKey<'a> = (
    Option<&'a TargetRef>,
    Option<&'a str>,
    Option<&'a str>,
    &'a str,
);
fn route_key(route: &worldline_core::reader_export::ReaderProfileRoute) -> RouteKey<'_> {
    (
        route.target.as_ref(),
        route.manuscript_id.as_deref(),
        route.chapter_id.as_deref(),
        route.output_path.as_str(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldline_core::reader_export::ReaderProfileRoute;

    #[test]
    fn indexed_route_check_preserves_every_old_route_and_required_feature() {
        let old_route = ReaderProfileRoute {
            target: Some(TargetRef::new("entity", "a")),
            manuscript_id: None,
            chapter_id: None,
            output_path: "objects/o0001.html".into(),
        };
        let profile = ReaderPublicationProfile {
            schema_version: 1,
            required_features: vec!["future.required.v99".into()],
            id: "public".into(),
            title: "公开配置".into(),
            selection: ReaderPublishState::new().selection(),
            routes: vec![old_route],
        };
        let input = ProfileInput {
            generation: 7,
            baseline: "baseline".into(),
            selection: profile.selection.clone(),
            profile: Some(profile.clone()),
            id: profile.id.clone(),
            title: profile.title.clone(),
        };
        let mut plan = ReaderProfileSavePlan {
            profile,
            content_baseline: input.baseline.clone(),
            document_path: ".world/reader-profiles/public.json".into(),
            document_before_hash: None,
            plan_digest: "digest".into(),
        };
        plan.profile.routes.push(ReaderProfileRoute {
            target: Some(TargetRef::new("entity", "b")),
            manuscript_id: None,
            chapter_id: None,
            output_path: "objects/o0002.html".into(),
        });
        assert!(
            plan_matches(&input, &plan),
            "可新增route，未知能力继续留给core拒绝"
        );
        let mut changed = plan.clone();
        changed.profile.routes[0].output_path = "objects/o0003.html".into();
        assert!(!plan_matches(&input, &changed));
        let mut changed = plan.clone();
        changed.profile.routes[0].chapter_id = Some("unexpected".into());
        assert!(!plan_matches(&input, &changed));
        plan.profile.required_features.clear();
        assert!(!plan_matches(&input, &plan), "后台不得删未知必需能力");
    }
}
