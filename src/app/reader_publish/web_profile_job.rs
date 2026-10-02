use super::super::WorldeditApp;
use super::profile_job::{ProfileInput, ProfileJob};
use crate::worker_protocol::{WorkEvent, WorkOutput, WorkRequest, WorkTask, SCHEMA_VERSION};

impl WorldeditApp {
    pub(super) fn start_web_profile_plan(
        &mut self,
        input: ProfileInput,
        ctx: &egui::Context,
    ) -> Result<(), String> {
        let (snapshot_state, files, entry) = self.reader_worker_snapshot()?;
        let request = WorkRequest {
            schema_version: SCHEMA_VERSION,
            job_id: format!("reader-profile-{}", input.generation),
            generation: input.generation,
            baseline: input.baseline.clone(),
            entry,
            snapshot_state: Some(snapshot_state),
            task: WorkTask::ReaderProfileSavePlan {
                selection: input.selection.clone(),
                profile: input.profile.clone(),
                id: input.id.clone(),
                title: input.title.clone(),
            },
        };
        let worker = crate::worker_host::WorkerJob::start(request, &files, ctx)?;
        self.reader_publish.profile_job = Some(ProfileJob { input, worker });
        Ok(())
    }

    pub(super) fn poll_reader_profile_job(&mut self) {
        loop {
            let Some(job) = self.reader_publish.profile_job.as_mut() else {
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
                    self.reader_publish.profile_job = None;
                    self.reader_publish.status = Some(format!("配置后台任务失败：{error}"));
                    return;
                }
                WorkEvent::Done { output, binaries } => {
                    let job = self
                        .reader_publish
                        .profile_job
                        .take()
                        .expect("本帧任务存在");
                    let result = match *output {
                        WorkOutput::ReaderProfileSavePlan { plan } if binaries.is_empty() => {
                            Ok(plan)
                        }
                        _ => Err("后台配置计划类型或负载不一致".into()),
                    };
                    self.accept_reader_profile_plan(job.input.clone(), result);
                    return;
                }
            }
        }
    }
}
