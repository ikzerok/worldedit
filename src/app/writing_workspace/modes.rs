//! 视图意图在原输入控件绘完后生效，不把同帧文字交给新视图。
use super::*;
mod reload;
#[cfg(test)]
mod tests;

pub(super) struct Request {
    path: std::path::PathBuf,
    target: TargetRef,
    frame: u64,
    owner: Option<egui::Id>,
    mode: Option<Mode>,
    dialogue: Option<bool>,
    insert: bool,
    reload_identity: Option<String>,
}
impl Request {
    fn has_action(&self) -> bool {
        self.mode.is_some()
            || self.dialogue.is_some()
            || self.insert
            || self.reload_identity.is_some()
    }
    fn matches(&self, ctx: &egui::Context, buffer: &WritingBuffer, target: &TargetRef) -> bool {
        self.frame == ctx.cumulative_frame_nr()
            && self.path == buffer.path()
            && self.target == *target
    }
}
impl ViewState {
    pub(in crate::app) fn session_dialogue_mode(&self) -> bool {
        self.dialogue.enabled
    }
    pub(in crate::app) fn restore_dialogue_mode(&mut self, enabled: bool) {
        self.dialogue.enabled = enabled;
        self.mode_request = None;
    }
    pub(super) fn has_mode_request(
        &self,
        ctx: &egui::Context,
        buffer: &WritingBuffer,
        target: &TargetRef,
    ) -> bool {
        self.mode_request
            .as_ref()
            .is_some_and(|request| request.has_action() && request.matches(ctx, buffer, target))
    }
    fn queue_mode_request(
        &mut self,
        ctx: &egui::Context,
        buffer: &WritingBuffer,
        target: &TargetRef,
        mode: Option<Mode>,
        dialogue: Option<bool>,
        insert: bool,
    ) {
        if self.input_blocked(ctx) {
            return;
        }
        // 未被原文档绘制消费的旧意图不能跨帧复活。
        if self
            .mode_request
            .as_ref()
            .is_some_and(|request| request.frame != ctx.cumulative_frame_nr())
        {
            self.mode_request = None;
        }
        if self
            .mode_request
            .as_ref()
            .is_some_and(|request| !request.matches(ctx, buffer, target))
        {
            return;
        }
        if self
            .mode_request
            .as_ref()
            .is_none_or(|request| !request.has_action())
        {
            self.mode_request = Some(Request {
                path: buffer.path().into(),
                target: target.clone(),
                frame: ctx.cumulative_frame_nr(),
                owner: self
                    .frame_input_owner
                    .filter(|(frame, _)| *frame == ctx.cumulative_frame_nr())
                    .and_then(|(_, owner)| owner)
                    .filter(|id| ctx.memory(|memory| memory.focused()) == Some(*id)),
                mode,
                dialogue,
                insert,
                reload_identity: None,
            });
        }
    }
    pub(super) fn protect_toolbar_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        buffer: &WritingBuffer,
        target: &TargetRef,
    ) {
        // disabled 的动作不能执行，但在可编辑正文旁按下它也不能吞掉本帧输入。
        let pressed_here =
            response.contains_pointer() && response.ctx.input(|input| input.pointer.any_pressed());
        if ui.is_enabled()
            && (response.clicked() || response.is_pointer_button_down_on() || pressed_here)
        {
            self.queue_mode_request(&response.ctx, buffer, target, None, None, false);
        }
    }
    pub(super) fn request_mode(
        &mut self,
        ctx: &egui::Context,
        buffer: &WritingBuffer,
        target: &TargetRef,
        mode: Mode,
    ) {
        self.queue_mode_request(ctx, buffer, target, Some(mode), None, false);
    }
    pub(super) fn request_dialogue(
        &mut self,
        ctx: &egui::Context,
        buffer: &WritingBuffer,
        target: &TargetRef,
        enabled: bool,
        insert: bool,
    ) {
        self.queue_mode_request(
            ctx,
            buffer,
            target,
            Some(Mode::Prose),
            Some(enabled),
            insert,
        );
    }
    pub(super) fn mode_input_scope(
        &self,
        ui: &egui::Ui,
        buffer: &WritingBuffer,
        target: &TargetRef,
    ) -> ModeInputScope {
        let ctx = ui.ctx();
        let owner = self
            .mode_request
            .as_ref()
            .filter(|request| ui.is_enabled() && request.matches(ctx, buffer, target))
            .and_then(|request| request.owner);
        let pointer = owner
            .filter(|_| {
                ctx.input(|input| {
                    input
                        .events
                        .iter()
                        .any(|event| matches!(event, egui::Event::Text(_) | egui::Event::Paste(_)))
                })
            })
            .map(|owner| {
                ctx.memory_mut(|memory| memory.request_focus(owner));
                ctx.input_mut(|input| std::mem::take(&mut input.pointer))
            });
        ModeInputScope {
            ctx: ctx.clone(),
            pointer,
        }
    }
    pub(super) fn finish_mode_request(
        &mut self,
        ctx: &egui::Context,
        buffer: &WritingBuffer,
        target: &TargetRef,
    ) {
        let Some(request) = self.mode_request.take() else {
            return;
        };
        if !request.matches(ctx, buffer, target) || self.input_blocked(ctx) {
            return;
        }
        // 打开工具菜单只保护本帧旧输入，不切换视图或清除其选区。
        if !request.has_action() {
            return;
        }
        if self.mode == Mode::Prose && !self.dialogue.enabled {
            self.prose_return_cursor = self.cursor_for_buffer(buffer, target).or_else(|| {
                self.pending_cursor
                    .clone()
                    .filter(|cursor| cursor.mode == Mode::Prose.key())
            });
        }
        if let Some(mode) = request.mode {
            self.mode = mode;
            self.pending_focus = true;
        }
        if let Some(enabled) = request.dialogue {
            self.dialogue.enabled = enabled;
            if self.mode == Mode::Prose {
                // 初次进入用于阅读/选句；重入已有 F 才明确回其原字段。
                self.pending_focus = !enabled || self.dialogue.has_form(buffer, target);
            }
        }
        self.selection_mode = None;
        if self.mode == Mode::Prose && !self.dialogue.enabled {
            self.restore_cursor(self.prose_return_cursor.clone());
            self.prepare_restore(buffer, target);
        }
        if request.insert {
            self.dialogue.request_insert();
            self.pending_focus = false;
        }
        ctx.request_repaint();
    }
}

pub(super) struct ModeInputScope {
    ctx: egui::Context,
    pointer: Option<egui::PointerState>,
}
impl Drop for ModeInputScope {
    fn drop(&mut self) {
        if let Some(pointer) = self.pointer.take() {
            self.ctx.input_mut(|input| input.pointer = pointer);
        }
    }
}
