//! 当前正文的独立试演；不持有/替换普通 PlayState，不应用或保存作者输入。
mod job;
mod navigation;
mod shortcuts;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
mod view;
use crate::app::{personal::Location, WorldeditApp};
use crate::draft_rehearsal_worker::{SessionWorker, View};
use std::path::PathBuf;
use worldline_core::draft_rehearsal::{DraftRehearsalExcludedInput, DraftRehearsalRequest};
use worldline_runtime::{ReplayBudget, StateInspectionQuery};

#[derive(Clone, PartialEq, Eq)]
struct AuthorKey {
    root: PathBuf,
    version: u64,
    buffers: Vec<(PathBuf, u64, bool)>,
    retained: bool,
    blocked: bool,
}
struct Preparation {
    input: DraftRehearsalRequest,
    key: AuthorKey,
    origin: Location,
    session_id: String,
    worker: Option<SessionWorker>,
    view: Option<View>,
    error: Option<String>,
}
struct Running {
    input: DraftRehearsalRequest,
    key: AuthorKey,
    origin: Location,
    worker: Option<SessionWorker>,
    view: View,
    transcript: String,
    query: StateInspectionQuery,
    submitted_query: StateInspectionQuery,
    stale: bool,
    paused: bool,
    stopped: bool,
}
#[derive(Default, PartialEq, Eq)]
enum Pane {
    #[default]
    Story,
    State,
    Conditions,
    Scope,
}

pub(in crate::app) struct DraftRehearsalUi {
    pub(in crate::app) active: bool,
    pending: Option<Preparation>,
    preparation_frame: Option<u64>,
    running: Option<Running>,
    sequence: u64,
    seed: u64,
    budget: ReplayBudget,
    pane: Pane,
    notice: Option<String>,
}
impl DraftRehearsalUi {
    pub(super) fn has_session(&self) -> bool {
        self.running.is_some()
    }
    pub(in crate::app) fn has_pending(&self) -> bool {
        self.pending.is_some()
    }
}
impl Default for DraftRehearsalUi {
    fn default() -> Self {
        Self {
            active: false,
            pending: None,
            preparation_frame: None,
            running: None,
            sequence: 0,
            seed: 1,
            budget: ReplayBudget::new(100_000, 50),
            pane: Pane::default(),
            notice: None,
        }
    }
}

impl WorldeditApp {
    fn rehearsal_author_key(&self) -> AuthorKey {
        AuthorKey {
            root: self.project.root.clone(),
            version: self.version,
            buffers: self
                .manuscript
                .writing_buffer_stamps()
                .into_iter()
                .filter(|(_, _, changed)| *changed)
                .collect(),
            retained: self.manuscript.has_retained_writing_input(),
            blocked: !self.project.authoring_diagnostics().is_empty()
                || !self.project.recovery_conflicts().is_empty(),
        }
    }
    fn rehearsal_composing(&self) -> bool {
        self.ime_composing
            || self.command_palette.ime
            || self.command_palette.ime_frame
            || self.manuscript.has_retained_writing_input()
    }

    pub(in crate::app) fn request_draft_rehearsal(&mut self, ctx: &egui::Context) {
        if self.draft_rehearsal.pending.is_some() {
            return;
        }
        let result = (|| {
            if self.rehearsal_composing() {
                return Err("请先完成正文组合/保留输入，当前输入已完整保留".to_owned());
            }
            let buffers = self.manuscript.writing_buffers();
            let included: std::collections::BTreeSet<_> = buffers
                .iter()
                .filter(|b| b.is_changed())
                .map(|b| format!("正文 · {}", b.path().display()))
                .collect();
            let excluded = self
                .unapplied_export_inputs()
                .into_iter()
                .filter(|input| {
                    input.kind != "书稿 / 正文草稿" || !included.contains(&input.source)
                })
                .map(|input| DraftRehearsalExcludedInput {
                    kind: input.kind.into(),
                    source: input.source,
                })
                .collect();
            let input = DraftRehearsalRequest::from_writing_buffers(
                &self.project,
                &buffers,
                excluded,
                false,
            )?;
            let mut origin = self.author_location(Some(ctx));
            if origin.tab == Some(crate::app::Tab::Play) {
                let session = self.manuscript_session();
                if let Some(anchor) = &session.anchor {
                    origin.tab = Some(crate::app::Tab::Manuscript);
                    origin.file = anchor.path.clone();
                }
            }
            self.draft_rehearsal.sequence = self.draft_rehearsal.sequence.wrapping_add(1);
            Ok(Preparation {
                input,
                key: self.rehearsal_author_key(),
                origin,
                session_id: format!(
                    "draft-{}-{}",
                    ctx.cumulative_frame_nr(),
                    self.draft_rehearsal.sequence
                ),
                worker: None,
                view: None,
                error: None,
            })
        })();
        match result {
            Ok(pending) => {
                self.draft_rehearsal.pending = Some(pending);
                self.draft_rehearsal.notice = None;
                self.poll_draft_rehearsal(ctx);
            }
            Err(error) => {
                self.draft_rehearsal.notice = Some(error.clone());
                self.message = Some(error);
            }
        }
    }

    fn rehearsal_guard(
        &self,
        input: &DraftRehearsalRequest,
        key: &AuthorKey,
        disk: bool,
    ) -> Result<(), String> {
        if &self.rehearsal_author_key() != key {
            return Err("草稿或工作区已变化；旧试演只读保留，请重新试演".into());
        }
        input.verify_current(
            &self.project,
            &self.manuscript.writing_buffers(),
            self.rehearsal_composing(),
        )?;
        if disk {
            self.project.verify_review_navigation()?;
        }
        Ok(())
    }
}
