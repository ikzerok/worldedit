//! egui 0.32 的直接 Commit 需要当前选区凭证；不将 IME 改写为普通 Text。
use super::Form;
use egui::{Context, Event, Id, ImeEvent, Ui};

#[derive(Default)]
pub(super) struct ReceiverState {
    pending_preedit: bool,
    frame: Option<u64>,
    last_drawn: Option<u64>,
}

pub(super) struct ReceiverScope {
    ctx: Context,
}
impl ReceiverScope {
    pub(super) fn begin(ui: &Ui, form: &mut Form, id: Id) -> Option<Self> {
        let ctx = ui.ctx();
        if ctx.memory(|memory| memory.focused()) != Some(id) {
            // 真实失焦结束此字段会话；别的窗口不必把 Disabled 送回旧字段。
            form.ime.remove(&id);
            return None;
        }
        if !ui.is_enabled()
            || form.ime_owner != Some(id)
            || super::super::input_registry::drawn_this_frame(ctx, id)
        {
            return None;
        }
        let state = form.ime.entry(id).or_default();
        let frame = ctx.cumulative_frame_nr();
        if state
            .last_drawn
            .is_some_and(|previous| previous.saturating_add(1) < frame)
        {
            // 原输入框离开可见接收路径后重新进入，不复活隐藏期间结束的旧会话。
            state.pending_preedit = false;
        }
        state.last_drawn = Some(frame);
        if !ctx.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, Event::Ime(_)))
        }) {
            return None;
        }
        if state.frame == Some(frame) {
            return None;
        }
        state.frame = Some(frame);
        ctx.input_mut(|input| {
            let mut adapted = Vec::with_capacity(input.events.len());
            let mut ended = false;
            for event in std::mem::take(&mut input.events) {
                match &event {
                    Event::Ime(ImeEvent::Enabled) => {
                        state.pending_preedit = false;
                        ended = false;
                    }
                    Event::Ime(ImeEvent::Preedit(text)) if text != "\n" && text != "\r" => {
                        state.pending_preedit = true;
                        ended = false;
                    }
                    Event::Ime(ImeEvent::Commit(text)) if text != "\n" && text != "\r" => {
                        if !state.pending_preedit && !text.is_empty() {
                            // 原 Commit 只保留一次，由 TextEdit 自己替换字符选区、更新 Undo。
                            adapted.push(Event::Ime(ImeEvent::Enabled));
                        }
                        state.pending_preedit = false;
                        ended = false;
                    }
                    Event::Ime(ImeEvent::Disabled) => {
                        // Linux 的清理 Disabled 后可紧跟 Commit；同批不能丢掉已有范围校验。
                        ended = true;
                    }
                    _ => {}
                }
                adapted.push(event);
            }
            if ended {
                // 完成取消的一轮不污染下一次独立输入，且凭证随 Form 生命周期结束。
                state.pending_preedit = false;
            }
            input.events = adapted;
        });
        Some(Self { ctx: ctx.clone() })
    }
}
impl Drop for ReceiverScope {
    fn drop(&mut self) {
        // TextEdit 也可能消费 IME。不能恢复整个事件列表、复活已消费键或向下个字段重放。
        // 这里只消费已绘制原接收者的派生 IME；input.raw.events 始终保持原样。
        self.ctx
            .input_mut(|input| input.events.retain(|event| !matches!(event, Event::Ime(_))));
    }
}
