//! 对象旁查复用现有隔离会话；独立 InspectObject 页不污染主检查筛选。
use super::*;
use crate::draft_rehearsal_worker::Action;
use worldline_runtime::{InspectionStamp, StateInspectionPage};

pub(in crate::app::play) struct ObjectRehearsalSnapshot {
    pub stamp: Option<InspectionStamp>,
    pub version: u64,
    pub baseline: String,
    pub page: Option<StateInspectionPage>,
    pub query: StateInspectionQuery,
    pub busy: bool,
    pub source_reason: Option<String>,
    pub query_error: Option<String>,
}
impl WorldeditApp {
    pub(in crate::app::play) fn object_rehearsal_snapshot(
        &self,
    ) -> Option<ObjectRehearsalSnapshot> {
        let running = self.draft_rehearsal.running.as_ref()?;
        let source_reason = if running.stale {
            Some("旧草稿快照只读保留；来源已变化，不能定位到新稿".into())
        } else if running.worker.is_none() {
            Some("草稿运行后台已关闭；仅保留已收到的真实页".into())
        } else if self.rehearsal_composing() {
            Some("请先完成输入法组合；作者输入保留".into())
        } else {
            None
        };
        Some(ObjectRehearsalSnapshot {
            stamp: running.view.inspection.as_ref().map(|page| page.stamp),
            version: running.key.version,
            baseline: running.view.scope.content_baseline.clone(),
            page: running.object_page.clone(),
            query: running.object_query.clone(),
            busy: running.worker.as_ref().is_some_and(SessionWorker::busy),
            source_reason,
            query_error: running.object_query_error.clone(),
        })
    }
    pub(in crate::app::play) fn query_object_rehearsal(
        &mut self,
        query: StateInspectionQuery,
    ) -> Result<(), String> {
        let running = self
            .draft_rehearsal
            .running
            .as_mut()
            .ok_or("草稿运行已关闭")?;
        let worker = running
            .worker
            .as_mut()
            .ok_or("草稿后台已关闭；旧查询页仍保留")?;
        if worker.busy() {
            return Err("上一条草稿请求尚未完成".into());
        }
        worker.submit(Action::InspectObject {
            query: query.clone(),
        })?;
        running.object_query = query;
        running.object_query_pending = true;
        running.object_query_error = None;
        Ok(())
    }
    pub(in crate::app::play) fn object_rehearsal_source_error(&self) -> Option<String> {
        self.draft_rehearsal.notice.clone()
    }
    pub(in crate::app::play) fn object_rehearsal_evidence(
        &self,
    ) -> Option<&[worldline_runtime::ChoiceExplanation]> {
        Some(&self.draft_rehearsal.running.as_ref()?.view.conditions)
    }
    pub(in crate::app::play) fn request_object_rehearsal_source(
        &mut self,
        ctx: &egui::Context,
        action: Action,
    ) -> Result<(), String> {
        if self.rehearsal_composing() {
            return Err("请先完成输入法组合；原输入保留".into());
        }
        let running = self
            .draft_rehearsal
            .running
            .as_ref()
            .ok_or("草稿试演已关闭")?;
        self.rehearsal_guard(&running.input, &running.key, true)?;
        let state = &self.replay_debugger.object_inspection;
        let pending = super::super::object_inspection::PendingObjectSource {
            stamp: state.stamp.ok_or("没有可验证的对象运行观测")?,
            target: state.target.clone().ok_or("没有完整对象身份")?,
            origin: self.author_location(Some(ctx)),
        };
        let running = self
            .draft_rehearsal
            .running
            .as_mut()
            .ok_or("草稿试演已关闭")?;
        let worker = running.worker.as_mut().ok_or("草稿试演后台已关闭")?;
        if worker.busy() {
            return Err("上一条草稿请求尚未完成，请稍后重试来源定位".into());
        }
        worker.submit(action)?;
        running.object_source = Some(pending);
        self.draft_rehearsal.notice = None;
        Ok(())
    }
}
