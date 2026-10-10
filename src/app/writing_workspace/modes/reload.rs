//! 重开已应用来源也须等原接收者结束，不能将本帧旧输入投向新 Buffer。
use super::*;

impl ViewState {
    pub(in crate::app::writing_workspace) fn request_reload(
        &mut self,
        ctx: &egui::Context,
        buffer: &WritingBuffer,
        target: &TargetRef,
    ) {
        if self.input_blocked(ctx) {
            return;
        }
        self.queue_mode_request(ctx, buffer, target, None, None, false);
        if let Some(request) = self
            .mode_request
            .as_mut()
            .filter(|request| request.matches(ctx, buffer, target) && !request.has_action())
        {
            request.reload_identity = Some(buffer.identity());
        }
    }

    pub(in crate::app::writing_workspace) fn finish_toolbar_request(
        &mut self,
        ctx: &egui::Context,
        project: &Project,
        buffer: &mut WritingBuffer,
        target: &TargetRef,
        action: &mut Action,
    ) {
        if !self
            .mode_request
            .as_ref()
            .is_some_and(|request| request.reload_identity.is_some())
        {
            self.finish_mode_request(ctx, buffer, target);
            return;
        }
        let request = self.mode_request.take().expect("已有来源重开请求");
        if !request.matches(ctx, buffer, target) || self.input_blocked(ctx) {
            return;
        }
        if buffer.is_changed()
            || request.reload_identity.as_deref() != Some(buffer.identity().as_str())
        {
            action.error =
                Some("本帧正文输入已变化，未重开来源；完整正文草稿和对白字段均保留。".into());
            return;
        }
        if buffer.baseline() == project.content_baseline()
            || !self.dialogue_retained_for(buffer.path())
        {
            action.error = Some("来源状态已变化，未重开；请重新核对当前正文与保留字段。".into());
            return;
        }
        match project.open_source_writing_buffer(buffer.path()) {
            Ok(current) => {
                *buffer = current;
                self.invalidate_projection();
                self.selection_mode = None;
                ctx.request_repaint();
            }
            Err(error) => action.error = Some(error),
        }
    }
}
