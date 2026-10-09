//! 普通外改的显式三方候选与单步采纳；所有语义、重验及保存基线来自core。
mod input;
mod jobs;
#[cfg(test)]
mod tests;
mod view;
use super::WorldeditApp;
use crate::theme::{self, *};
use std::collections::BTreeMap;
use std::path::PathBuf;
use worldline_core::project::reconciliation::{
    ReconciliationChoice, ReconciliationDecision, ReconciliationPlan, ReconciliationRequest,
    ReconciliationSession,
};
use worldline_core::project::Project;

pub(super) const DRAFT_KIND: &str = "外部改稿候选";

#[derive(Default)]
struct Draft {
    choice: Option<ReconciliationChoice>,
    manual: String,
}

#[derive(Default)]
pub(super) struct ConflictView {
    open: bool,
    workspace_root: PathBuf,
    session: Option<ReconciliationSession>,
    drafts: BTreeMap<PathBuf, Draft>,
    selected: usize,
    side: usize,
    preview: Option<ReconciliationPlan>,
    allow_incomplete_source: bool,
    error: Option<String>,
    notice: Option<String>,
    recapture_confirm: bool,
    request_focus: bool,
    return_focus: Option<egui::Id>,
    close_requested: bool,
    job: Option<jobs::Job>,
    composition: input::Composition,
}

#[derive(Clone, Copy)]
enum Action {
    Preview,
    Apply,
    Recapture,
    Close,
    CancelJob,
}

impl ConflictView {
    pub(super) fn capture(project: &Project) -> Self {
        let mut view = Self {
            workspace_root: project.root.clone(),
            open: true,
            request_focus: true,
            ..Self::default()
        };
        match jobs::Job::start(project, || jobs::Work::Capture, egui::Context::default()) {
            Ok(job) => view.job = Some(job),
            Err(error) => view.error = Some(error),
        }
        view
    }

    pub(super) fn open_or_capture(&mut self, project: &Project) {
        if self.workspace_root == project.root && self.session.is_some() {
            self.open = true;
            self.request_focus = true;
        } else {
            *self = Self::capture(project);
        }
    }

    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(super) fn has_unsubmitted_work(&self) -> bool {
        self.drafts
            .values()
            .any(|draft| draft.choice.is_some() || !draft.manual.is_empty())
    }

    /// 仅由作者明确丢弃未应用输入的既有生命周期调用；Close/Escape不调用。
    pub(super) fn discard_drafts(&mut self) {
        *self = Self::default();
    }

    pub(super) fn resume_draft(&mut self) {
        self.open = true;
        self.close_requested = false;
        self.request_focus = true;
    }

    /// 在底层快捷键之前调用；Escape关闭材料但不丢弃候选，也不穿透原正文。
    pub(super) fn prepare_frame(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        if self.request_focus && self.return_focus.is_none() {
            self.return_focus = ctx.memory(|memory| memory.focused());
        }
        self.composition.observe(ctx);
        if !self.composition.blocks_actions()
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.close_requested = true;
        }
    }

    fn request(&self) -> ReconciliationRequest {
        ReconciliationRequest {
            choices: self
                .drafts
                .iter()
                .filter_map(|(path, draft)| {
                    draft.choice.as_ref().map(|choice| ReconciliationDecision {
                        path: path.clone(),
                        choice: match choice {
                            ReconciliationChoice::Manual { .. } => ReconciliationChoice::Manual {
                                text: draft.manual.clone(),
                            },
                            choice => choice.clone(),
                        },
                    })
                })
                .collect(),
            allow_incomplete_source: self.allow_incomplete_source,
        }
    }

    fn close(&mut self, ctx: &egui::Context) {
        self.open = false;
        self.close_requested = false;
        self.job = None;
        if let Some(id) = self.return_focus.take() {
            ctx.memory_mut(|memory| memory.request_focus(id));
        }
        ctx.request_repaint();
    }

    fn start_job(&mut self, project: &Project, apply: bool, ctx: &egui::Context) {
        if self.job.is_some() || self.composition.blocks_actions() {
            return;
        }
        self.error = None;
        self.notice = None;
        let Some(session) = self.session.as_ref() else {
            return;
        };
        if apply && self.preview.is_none() {
            return;
        }
        let started = jobs::Job::start(
            project,
            || {
                if let Some(plan) = self.preview.as_ref().filter(|_| apply) {
                    jobs::Work::Prepare(Box::new(plan.clone()))
                } else {
                    jobs::Work::Preview(Box::new((session.clone(), self.request())))
                }
            },
            ctx.clone(),
        );
        match started {
            Ok(job) => self.job = Some(job),
            Err(error) => self.error = Some(error),
        }
    }
}

impl WorldeditApp {
    pub(super) fn show_reconciliation(&mut self, ctx: &egui::Context) {
        let mut view = std::mem::take(&mut self.conflict_view);
        if !view.open {
            self.conflict_view = view;
            return;
        }
        view.prepare_frame(ctx);
        if view.close_requested {
            view.close(ctx);
            self.conflict_view = view;
            return;
        }
        let mut draft_names = self.dirty_draft_names();
        draft_names.retain(|name| *name != DRAFT_KIND);
        if view.composition.blocks_actions() {
            draft_names.push("外部候选输入法组合");
        }
        for name in &self.frame_dirty_drafts {
            if *name != DRAFT_KIND && !draft_names.contains(name) {
                draft_names.push(name);
            }
        }
        if let Some(outcome) = view.job.as_ref().and_then(jobs::Job::poll) {
            view.job = None;
            match outcome {
                jobs::Outcome::Captured(Ok(session)) => {
                    for draft in view.drafts.values_mut() {
                        draft.choice = None;
                    }
                    view.session = Some(*session);
                    view.preview = None;
                    view.selected = 0;
                    view.error = None;
                    view.notice = Some("已捕获真实三方；手工原文保留，请明确选择并预览".into());
                }
                jobs::Outcome::Captured(Err(error)) => view.error = Some(error),
                jobs::Outcome::Preview(Ok(plan)) => view.preview = Some(*plan),
                jobs::Outcome::Preview(Err(error)) | jobs::Outcome::Prepared(Err(error)) => {
                    view.error = Some(error)
                }
                jobs::Outcome::Prepared(Ok(prepared)) => {
                    if !draft_names.is_empty() {
                        view.error = Some(format!(
                            "请先处理未应用输入：{}。原候选与输入保留",
                            draft_names.join("、")
                        ));
                    } else {
                        match self.project.commit_prepared_reconciliation(*prepared) {
                            Ok(applied) => {
                                self.clear_edit_history();
                                self.remember(applied.undo);
                                self.recompile();
                                self.manuscript.rebase_clean(&self.project);
                                self.stale_form = self.has_open_authoring_form();
                                view.preview = None;
                                view.session = None;
                                view.drafts.clear();
                                view.notice = Some(
                                    "已采纳到工程内存；可撤销，尚未保存。关闭后使用“保存全部”。"
                                        .into(),
                                );
                                self.io_error = None;
                            }
                            Err(error) => view.error = Some(error),
                        }
                    }
                }
            }
        }
        if view.job.is_some() || !jobs::Job::available() {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }
        let action = view.render(ctx, &draft_names);
        match action {
            Some(Action::Preview) => view.start_job(&self.project, false, ctx),
            Some(Action::Apply) if draft_names.is_empty() => {
                view.start_job(&self.project, true, ctx)
            }
            Some(Action::Apply) => view.error = Some("尚有未应用输入，未采纳；请先处理原稿".into()),
            Some(Action::Recapture) => {
                match jobs::Job::start(&self.project, || jobs::Work::Capture, ctx.clone()) {
                    Ok(job) => view.job = Some(job),
                    Err(error) => view.error = Some(error),
                }
                view.recapture_confirm = false;
            }
            Some(Action::Close) => view.close(ctx),
            Some(Action::CancelJob) => {
                view.job = None;
                view.notice = Some("检查已取消；三方材料、手工候选与工程保持不变".into());
            }
            None => {}
        }
        self.conflict_view = view;
    }
}
