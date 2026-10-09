//! 准备层先于作者全局快捷键；取消后仍封住同一帧，保留接收控件的原始输入。
use super::{DraftRehearsalUi, WorldeditApp};

impl DraftRehearsalUi {
    pub(in crate::app) fn preparation_blocks_frame(&mut self, ctx: &egui::Context) -> bool {
        let frame = ctx.cumulative_frame_nr();
        if self.has_pending() {
            self.preparation_frame = Some(frame);
        }
        self.preparation_frame == Some(frame)
    }
}

impl WorldeditApp {
    // author_shortcuts 已先处理完整 IME 事件和完成帧；这里只接管非组合快捷键。
    pub(in crate::app) fn draft_rehearsal_shortcuts(&mut self, ctx: &egui::Context) -> bool {
        if !self.draft_rehearsal.preparation_blocks_frame(ctx) {
            return false;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.draft_rehearsal.pending = None;
        }
        true
    }
}
