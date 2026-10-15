use super::*;
use crate::app::play::{evidence_navigation::EvidenceNavigationRequest, inspection::SourceRequest};
use crate::draft_rehearsal_worker::{Action, SourceHit};

impl WorldeditApp {
    pub(in crate::app) fn object_inspection_position(&self) -> Option<ObjectInspectionPosition> {
        let state = &self.replay_debugger.object_inspection;
        state
            .open
            .then(|| {
                Some(ObjectInspectionPosition {
                    target: state.target.clone()?,
                    run: state.run?,
                    stamp: state.stamp?,
                    selected: state.selected.clone(),
                    scroll: state.scroll,
                    focus: state.focus,
                })
            })
            .flatten()
    }
    pub(in crate::app) fn restore_object_inspection_position(
        &mut self,
        ctx: &egui::Context,
        position: Option<ObjectInspectionPosition>,
    ) {
        let Some(position) = position else {
            return;
        };
        let state = &mut self.replay_debugger.object_inspection;
        if state.target.as_ref() != Some(&position.target)
            || state.run != Some(position.run)
            || state.stamp != Some(position.stamp)
        {
            self.message = Some("对象旁查已换成另一身份；未把旧选择套到新结果".into());
            return;
        }
        state.open = true;
        state.selected = position.selected;
        state.scroll = position.scroll;
        state.restore_scroll = true;
        if let Some(focus) = position.focus {
            ctx.memory_mut(|memory| memory.request_focus(focus));
        }
    }
    fn object_source_guard(&self, stamp: InspectionStamp) -> Result<RunKind, String> {
        let state = &self.replay_debugger.object_inspection;
        if state.stamp != Some(stamp) || state.run_changed {
            return Err("对象结果的运行观测已变化；请明确选择现有运行重新查询".into());
        }
        if let Some(reason) = &state.source_reason {
            return Err(reason.clone());
        }
        state.run.ok_or_else(|| "尚未选择真实运行".into())
    }
    pub(in crate::app::play) fn jump_to_object_inspection_source(
        &mut self,
        ctx: &egui::Context,
        request: &SourceRequest,
    ) {
        let result = (|| {
            if self.object_inspection_composing(ctx) {
                return Err("输入法组合尚未完成，未离开对象旁查".into());
            }
            let kind = self.object_source_guard(request.stamp)?;
            let page = self
                .replay_debugger
                .object_inspection
                .page
                .as_ref()
                .ok_or("旁查页已失效")?;
            if !page.items.iter().any(|item| {
                item.key == request.key && item.source.as_ref() == Some(&request.source)
            }) {
                return Err("来源不属于这个对象的当前检查页".into());
            }
            match kind {
                RunKind::Ordinary => {
                    let play = self.play.as_ref().ok_or("当前没有真实运行")?;
                    self.play_object_source_navigation_guard(ctx, &play.scope)?;
                    let hit = self
                        .inspection_source_hit_from_after_source_navigation_guard(request, page)?;
                    self.go_author_source_position_after_local_ime_guard(ctx, &hit, true)?;
                    self.replay_debugger.object_inspection.open = false;
                    self.message = Some("已定位同一运行的状态声明；Alt+Left 返回原对象旁查".into());
                }
                RunKind::Draft => {
                    self.request_object_rehearsal_source(
                        ctx,
                        Action::DeclarationSource {
                            source: request.source.clone(),
                        },
                    )?;
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.replay_debugger.object_inspection.error = Some(error);
        }
    }
    pub(super) fn jump_to_object_evidence(
        &mut self,
        ctx: &egui::Context,
        request: &EvidenceNavigationRequest,
    ) {
        let Some(stamp) = self.replay_debugger.object_inspection.stamp else {
            return;
        };
        let result = self.object_source_guard(stamp);
        match result {
            Ok(RunKind::Ordinary) => {
                if self.jump_to_evidence_source(ctx, request) {
                    self.replay_debugger.object_inspection.open = false;
                } else {
                    self.replay_debugger.object_inspection.error =
                        self.replay_debugger.notice.clone();
                }
            }
            Ok(RunKind::Draft) => {
                match self.request_object_rehearsal_source(
                    ctx,
                    Action::EvidenceSource {
                        source: request.source.clone(),
                    },
                ) {
                    Ok(()) => {}
                    Err(error) => self.replay_debugger.object_inspection.error = Some(error),
                }
            }
            Err(error) => self.replay_debugger.object_inspection.error = Some(error),
        }
    }
    pub(in crate::app::play) fn receive_object_rehearsal_source(
        &mut self,
        ctx: &egui::Context,
        pending: PendingObjectSource,
        source: SourceHit,
    ) {
        let stamp = pending.stamp;
        let location = self.author_location(Some(ctx));
        let moved = location.tab != pending.origin.tab
            || location.file != pending.origin.file
            || location.target != pending.origin.target
            || location.editor != pending.origin.editor
            || location.event != pending.origin.event;
        let current = self
            .object_rehearsal_snapshot()
            .and_then(|snapshot| snapshot.stamp);
        if !self.replay_debugger.object_inspection.open
            || self.replay_debugger.object_inspection.target.as_ref() != Some(&pending.target)
            || moved
            || self.auxiliary_ime_active(ctx)
            || current != Some(stamp)
            || self.object_source_guard(stamp) != Ok(RunKind::Draft)
        {
            self.replay_debugger.object_inspection.error =
                Some("来源返回时对象或运行已变化；没有改动新的作者位置".into());
            return;
        }
        if self.return_rehearsal_source(ctx, source) {
            self.replay_debugger.object_inspection.open = false;
            self.message = Some("已定位同一草稿试演的真实来源；Alt+Left 返回原对象旁查".into());
        } else {
            self.replay_debugger.object_inspection.error = self.object_rehearsal_source_error();
        }
    }
}
