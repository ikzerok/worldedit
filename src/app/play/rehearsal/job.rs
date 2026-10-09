use super::*;
use crate::draft_rehearsal_worker::{Action, Response};

impl WorldeditApp {
    pub(in crate::app::play) fn poll_draft_rehearsal(&mut self, ctx: &egui::Context) {
        let key = self.rehearsal_author_key();
        if let Some(pending) = &mut self.draft_rehearsal.pending {
            if pending.key != key {
                pending.worker = None;
                pending.error = Some("准备期间作者输入或工程变化，请重新核对；原稿保留".into());
            } else if pending.worker.is_none()
                && pending.error.is_none()
                && SessionWorker::available()
            {
                match SessionWorker::start(
                    &self.project,
                    &pending.input,
                    pending.session_id.clone(),
                    ctx,
                ) {
                    Ok(worker) => pending.worker = Some(worker),
                    Err(error) if error.contains("上一份隔离编译仍在退出") => {}
                    Err(error) => pending.error = Some(error),
                }
            }
            if let Some(result) = pending.worker.as_mut().and_then(SessionWorker::poll) {
                match result {
                    Ok(response) if response.error.is_none() => pending.view = response.view,
                    Ok(response) => {
                        pending.error = response.error;
                        pending.worker = None;
                    }
                    Err(error) => {
                        pending.error = Some(error);
                        pending.worker = None;
                    }
                }
            }
            if pending.view.is_none() && pending.error.is_none() {
                ctx.request_repaint_after(std::time::Duration::from_millis(30));
            }
        }
        let mut source = None;
        if let Some(running) = &mut self.draft_rehearsal.running {
            running.stale |= running.key != key;
            if let Some(result) = running.worker.as_mut().and_then(SessionWorker::poll) {
                match result {
                    Ok(Response {
                        view,
                        source: hit,
                        error,
                        ..
                    }) => {
                        if let Some(mut view) = view {
                            for output in &view.outputs {
                                if output.new_line && !running.transcript.is_empty() {
                                    running.transcript.push('\n');
                                }
                                if let Some(speaker) = &output.speaker {
                                    running.transcript.push_str(speaker);
                                    running.transcript.push('：');
                                }
                                running.transcript.push_str(&output.content);
                            }
                            view.outputs.clear();
                            running.paused |=
                                view.outcome.is_some_and(|outcome| outcome.is_suspended());
                            running.stopped |= view.error.is_some();
                            running.view = view;
                        }
                        if let Some(error) = error {
                            self.draft_rehearsal.notice = Some(error);
                        }
                        source = hit;
                    }
                    Err(error) => {
                        running.stopped = true;
                        running.worker = None;
                        self.draft_rehearsal.notice = Some(error);
                    }
                }
            }
            if running.worker.as_ref().is_some_and(SessionWorker::busy) {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
            }
        }
        if let Some(source) = source {
            if self.tab == crate::app::Tab::Play && self.draft_rehearsal.active {
                self.return_rehearsal_source(ctx, source);
            } else {
                self.draft_rehearsal.notice =
                    Some("来源已核对，但你已离开试演；未改变新的作者位置".into());
            }
        }
    }

    pub(super) fn confirm_draft_rehearsal(&mut self, ctx: &egui::Context) {
        let Some(mut pending) = self.draft_rehearsal.pending.take() else {
            return;
        };
        let result = self.rehearsal_guard(&pending.input, &pending.key, true);
        if let Err(error) = result {
            pending.error = Some(error);
            pending.worker = None;
            self.draft_rehearsal.pending = Some(pending);
            return;
        }
        let Some(view) = pending.view.take() else {
            self.draft_rehearsal.pending = Some(pending);
            return;
        };
        let Some(mut worker) = pending.worker.take() else {
            self.draft_rehearsal.pending = Some(pending);
            return;
        };
        if let Err(error) = worker.submit(Action::Start {
            seed: self.draft_rehearsal.seed,
            budget: self.draft_rehearsal.budget,
        }) {
            self.draft_rehearsal.notice = Some(error);
            return;
        }
        self.draft_rehearsal.running = Some(Running {
            input: pending.input,
            key: pending.key,
            origin: pending.origin,
            worker: Some(worker),
            view,
            transcript: String::new(),
            query: StateInspectionQuery::default(),
            submitted_query: StateInspectionQuery::default(),
            stale: false,
            paused: false,
            stopped: false,
        });
        self.draft_rehearsal.active = true;
        self.draft_rehearsal.pane = Pane::Story;
        self.tab = crate::app::Tab::Play;
        ctx.request_repaint();
    }

    pub(super) fn send_rehearsal_action(&mut self, action: Action) {
        if self.rehearsal_composing() {
            self.draft_rehearsal.notice = Some("请先完成输入法组合；当前文字和试演位置保留".into());
            return;
        }
        let Some(running) = &self.draft_rehearsal.running else {
            return;
        };
        let navigation = matches!(
            action,
            Action::EvidenceSource { .. } | Action::DeclarationSource { .. }
        );
        let readonly = matches!(action, Action::Inspect { .. });
        if !readonly {
            if let Err(error) = self.rehearsal_guard(&running.input, &running.key, navigation) {
                self.draft_rehearsal.notice = Some(error);
                return;
            }
        }
        let running = self.draft_rehearsal.running.as_mut().unwrap();
        match running
            .worker
            .as_mut()
            .ok_or_else(|| "试演后台已关闭，只保留已收到的证据".to_owned())
            .and_then(|worker| worker.submit(action))
        {
            Ok(()) => self.draft_rehearsal.notice = None,
            Err(error) => self.draft_rehearsal.notice = Some(error),
        }
    }
}
