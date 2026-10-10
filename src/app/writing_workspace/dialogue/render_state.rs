//! 绘制期只收集切换意图；旧字段消费本帧输入后再决定是否替换。
use super::*;

pub(super) const PAGE_ROWS: usize = 80;
pub(super) struct Pending {
    key: Key,
    snapshot: String,
    buffer_identity: String,
    operation: DialogueOperation,
    label: String,
}
impl State {
    pub(super) fn queue_replacement(
        &mut self,
        buffer: &WritingBuffer,
        projection: &DialogueProjection,
        operation: DialogueOperation,
        label: String,
    ) {
        // 一帧只接受第一个意图；不会将鼠标和快捷键意图排到后续帧重放。
        if self.pending.is_none() {
            self.pending = Some(Pending {
                key: Key::new(buffer, &projection.target),
                snapshot: projection.snapshot.clone(),
                buffer_identity: buffer.identity(),
                operation,
                label,
            });
        }
    }
    pub(super) fn begin_render(&mut self, key: &Key) {
        self.pending = None;
        if self.page_key.as_ref() != Some(key) {
            self.page_key = Some(key.clone());
            self.row_offset = 0;
        }
        #[cfg(test)]
        {
            self.rendered_rows = 0;
        }
    }
    pub(super) fn finish_pending(
        &mut self,
        ctx: &egui::Context,
        buffer: &WritingBuffer,
        projection: &DialogueProjection,
    ) {
        let Some(pending) = self.pending.take() else {
            return;
        };
        if pending.key != Key::new(buffer, &projection.target)
            || pending.snapshot != projection.snapshot
            || pending.buffer_identity != buffer.identity()
        {
            return;
        }
        if let Some(form) = self
            .forms
            .get_mut(&pending.key)
            .filter(|form| form.protected())
        {
            form.error = Some("本帧输入已保留；请先纳入或明确取消当前语句，再切换语句。".into());
            return;
        }
        self.insert_requested = false;
        self.begin_context(ctx, buffer, projection, pending.operation, pending.label);
    }
}

pub(super) fn pages(
    ui: &mut egui::Ui,
    state: &mut State,
    projection: &DialogueProjection,
    enabled: bool,
    bottom: bool,
) {
    let total = projection.rows.len();
    state.row_offset = state
        .row_offset
        .min(total.saturating_sub(1) / PAGE_ROWS * PAGE_ROWS);
    if total <= PAGE_ROWS {
        return;
    }
    ui.push_id(("dialogue-pages", bottom), |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "连续正文 {}–{} / {} 段 · 全部 {} 句",
                state.row_offset + 1,
                (state.row_offset + PAGE_ROWS).min(total),
                total,
                projection.statements.len()
            ));
            if theme::add_enabled(
                ui,
                enabled && state.row_offset > 0,
                egui::Button::new("前80段正文"),
            )
            .clicked()
            {
                state.row_offset = state.row_offset.saturating_sub(PAGE_ROWS);
            }
            if theme::add_enabled(
                ui,
                enabled && state.row_offset + PAGE_ROWS < total,
                egui::Button::new("后80段正文"),
            )
            .clicked()
            {
                state.row_offset += PAGE_ROWS;
            }
        });
        ui.label(theme::muted(
            "其余正文可翻页连续阅读；分页不改变草稿、生成或交付范围。未完成的语句输入始终保留。",
        ));
    });
}
