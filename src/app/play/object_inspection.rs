//! 从世界对象进入已有真实运行；查询身份由 runtime 的冻结 catalog 决定。
mod keyboard;
mod navigation;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
mod view;
use crate::app::{personal::Location, WorldeditApp};
use worldline_core::TargetRef;
use worldline_runtime::{
    InspectionKey, InspectionStamp, StateInspectionPage, StateInspectionQuery,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::app) enum RunKind {
    Ordinary,
    Draft,
}
impl RunKind {
    fn label(self) -> &'static str {
        match self {
            Self::Ordinary => "普通试玩",
            Self::Draft => "草稿隔离试演",
        }
    }
}

#[derive(Default)]
pub(in crate::app) struct ObjectInspectionUi {
    pub open: bool,
    pub(in crate::app::play) target: Option<TargetRef>,
    run: Option<RunKind>,
    pub(in crate::app::play) stamp: Option<InspectionStamp>,
    version: u64,
    baseline: String,
    query: StateInspectionQuery,
    submitted: Option<StateInspectionQuery>,
    awaiting: bool,
    pub(in crate::app::play) page: Option<StateInspectionPage>,
    evidence: Vec<worldline_runtime::ChoiceExplanation>,
    pub(super) error: Option<String>,
    pub(in crate::app::play) source_reason: Option<String>,
    run_changed: bool,
    selected: Option<InspectionKey>,
    scroll: f32,
    restore_scroll: bool,
    focus: Option<egui::Id>,
    return_focus: Option<egui::Id>,
    pub(in crate::app) return_focus_pending: bool,
    keyboard: crate::app::writing_workspace::preview_navigation::FocusCycle,
    run_menu: keyboard::RunMenu,
    origin: Option<Box<Location>>,
}

pub(in crate::app::play) struct PendingObjectSource {
    pub stamp: InspectionStamp,
    pub target: TargetRef,
    pub origin: Location,
}

#[derive(Clone, PartialEq)]
pub(in crate::app) struct ObjectInspectionPosition {
    target: TargetRef,
    run: RunKind,
    stamp: InspectionStamp,
    selected: Option<InspectionKey>,
    scroll: f32,
    focus: Option<egui::Id>,
}

impl WorldeditApp {
    #[cfg(test)]
    pub(in crate::app) fn object_inspection_page_for_test(&self) -> Option<&StateInspectionPage> {
        self.replay_debugger.object_inspection.page.as_ref()
    }

    #[cfg(test)]
    pub(in crate::app::play) fn object_inspection_query_for_test(
        &mut self,
    ) -> &mut StateInspectionQuery {
        &mut self.replay_debugger.object_inspection.query
    }

    pub(in crate::app) fn open_object_inspection(
        &mut self,
        ctx: &egui::Context,
        target: TargetRef,
    ) {
        if self.auxiliary_ime_active(ctx) {
            self.message = Some("请先完成输入法组合；当前条件文字和作者位置保留".into());
            return;
        }
        self.begin_object_inspection(ctx, target);
    }

    // 只有条件表单的显式 Owner 动作使用其本地组合态；其他对象入口保留外层输入保护。
    pub(in crate::app) fn open_condition_object_inspection(
        &mut self,
        ctx: &egui::Context,
        target: TargetRef,
    ) {
        let composing = crate::app::condition_authoring::input::event_composing(
            ctx,
            &self.project.root,
            self.event_editor.is_some(),
        );
        if self.ime_composing || composing {
            return;
        }
        self.begin_object_inspection(ctx, target);
    }

    fn begin_object_inspection(&mut self, ctx: &egui::Context, target: TargetRef) {
        let origin = self.author_location(Some(ctx));
        self.replay_debugger.object_inspection = ObjectInspectionUi {
            open: true,
            target: Some(target.clone()),
            query: StateInspectionQuery {
                owner: Some(target),
                limit: 20,
                ..Default::default()
            },
            origin: Some(Box::new(origin)),
            focus: ctx.memory(|memory| memory.focused()),
            return_focus: ctx.memory(|memory| memory.focused()),
            ..Default::default()
        };
        self.sync_edit_layers(ctx);
    }

    pub(in crate::app::play) fn select_object_run(&mut self, kind: RunKind) {
        let identity = match kind {
            RunKind::Ordinary => self.play.as_ref().and_then(|play| {
                Some((
                    play.story.as_ref()?.inspection_stamp(),
                    play.version,
                    format!("已应用工程稿 #{}", play.version),
                ))
            }),
            RunKind::Draft => self
                .object_rehearsal_snapshot()
                .and_then(|snapshot| Some((snapshot.stamp?, snapshot.version, snapshot.baseline))),
        };
        let Some((stamp, version, baseline)) = identity else {
            self.replay_debugger.object_inspection.error =
                Some("此模式尚无可查询的真实运行；旁查不会启动故事".into());
            return;
        };
        let evidence = match kind {
            RunKind::Ordinary => self
                .play
                .as_ref()
                .and_then(|play| play.story.as_ref())
                .and_then(|story| story.choice_evidence())
                .unwrap_or_default()
                .to_vec(),
            RunKind::Draft => self
                .object_rehearsal_evidence()
                .unwrap_or_default()
                .to_vec(),
        };
        let state = &mut self.replay_debugger.object_inspection;
        state.evidence = evidence;
        state.run = Some(kind);
        state.stamp = Some(stamp);
        state.version = version;
        state.baseline = baseline;
        state.query.offset = 0;
        state.query.expected_stamp = Some(stamp);
        state.submitted = None;
        state.awaiting = false;
        state.page = None;
        state.error = None;
        state.run_changed = false;
        state.selected = None;
        state.scroll = 0.0;
        state.restore_scroll = true;
    }

    pub(in crate::app::play) fn object_inspection_composing(&self, ctx: &egui::Context) -> bool {
        let composing = crate::app::condition_authoring::input::object_composing(
            ctx,
            &self.project.root,
            self.replay_debugger.object_inspection.open,
        );
        self.ime_composing || composing
    }

    pub(in crate::app::play) fn refresh_object_inspection(&mut self, ctx: &egui::Context) {
        let composing = self.object_inspection_composing(ctx);
        self.poll_draft_rehearsal(ctx);
        let state = &self.replay_debugger.object_inspection;
        let (Some(kind), Some(stamp)) = (state.run, state.stamp) else {
            return;
        };
        let query = state.query.clone();
        match kind {
            RunKind::Ordinary => {
                let source_reason = self
                    .play
                    .as_ref()
                    .and_then(|play| self.play_object_source_guard(ctx, &play.scope).err());
                let current = self.play.as_ref().and_then(|play| play.story.as_ref());
                let changed = current.is_none_or(|story| story.inspection_stamp() != stamp);
                let state = &mut self.replay_debugger.object_inspection;
                state.run_changed = changed;
                state.source_reason = source_reason;
                if changed || composing || state.submitted.as_ref() == Some(&query) {
                    return;
                }
                match current.unwrap().inspect_state(&query) {
                    Ok(page) => {
                        state.page = Some(page);
                        state.error = None;
                    }
                    Err(error) => state.error = Some(format!("{}：{}", error.code, error.message)),
                }
                state.submitted = Some(query);
            }
            RunKind::Draft => {
                let Some(snapshot) = self.object_rehearsal_snapshot() else {
                    let state = &mut self.replay_debugger.object_inspection;
                    state.run_changed = true;
                    state.source_reason = Some("此草稿运行已关闭；旧页仅供阅读".into());
                    return;
                };
                let changed = snapshot.stamp != Some(stamp);
                let state = &mut self.replay_debugger.object_inspection;
                state.run_changed = changed;
                state.source_reason = snapshot.source_reason;
                if changed {
                    return;
                }
                if state.awaiting && !snapshot.busy {
                    state.awaiting = false;
                    if let Some(error) = snapshot.query_error {
                        state.error = Some(error);
                    } else if snapshot.query == query {
                        if let Some(page) = snapshot.page.filter(|page| {
                            page.stamp == stamp
                                && page.owner.as_ref().map(|owner| &owner.target)
                                    == query.owner.as_ref()
                        }) {
                            state.page = Some(page);
                            state.error = None;
                        } else {
                            state.error =
                                Some("返回页与本次对象或运行身份不一致；旧页未替换".into());
                        }
                    }
                }
                if !snapshot.busy && !composing && state.submitted.as_ref() != Some(&query) {
                    match self.query_object_rehearsal(query.clone()) {
                        Ok(()) => {
                            let state = &mut self.replay_debugger.object_inspection;
                            state.submitted = Some(query);
                            state.awaiting = true;
                            state.error = None;
                        }
                        Err(error) => self.replay_debugger.object_inspection.error = Some(error),
                    }
                }
            }
        }
    }
}
