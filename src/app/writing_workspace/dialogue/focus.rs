//! 逐句子模式只恢复真实控件焦点，不从解码文字猜源码光标。
use super::*;

impl State {
    pub(in crate::app::writing_workspace) fn has_form(
        &self,
        buffer: &WritingBuffer,
        target: &TargetRef,
    ) -> bool {
        self.forms.contains_key(&Key::new(buffer, target))
    }
    pub(in crate::app::writing_workspace) fn request_insert(&mut self) {
        self.insert_requested = true;
    }
}
pub(super) fn prepare(
    ui: &egui::Ui,
    buffer: &WritingBuffer,
    view: &mut ViewState,
    projection: &DialogueProjection,
    key: &Key,
) {
    if !ui.is_enabled() || !view.pending_focus || view.ime_active || view.dialogue.insert_requested
    {
        return;
    }
    let ctx = ui.ctx();
    if !view.dialogue.forms.contains_key(key) {
        if let Some(statement) = projection.statements.first() {
            view.dialogue.begin_context(
                ctx,
                buffer,
                projection,
                DialogueOperation::Update {
                    statement_id: statement.id.clone(),
                    draft: statement.draft.clone(),
                },
                "继续编辑语句".into(),
            );
        }
    }
    if let Some(form) = view.dialogue.forms.get_mut(key) {
        form.refocus = true;
    }
    view.pending_focus = false;
}
pub(super) fn restore(ui: &egui::Ui, form: &mut Form, fallback: Option<egui::Id>) {
    if !ui.is_enabled() {
        return;
    }
    let ctx = ui.ctx();
    let id = form
        .last_input
        .filter(|id| form.drawn_inputs.contains(id))
        .or_else(|| form.drawn_inputs.first().copied())
        .or(fallback);
    if let Some(id) = id {
        ctx.memory_mut(|memory| memory.request_focus(id));
        crate::app::search::record_navigation_focus(ctx, id);
    }
    form.refocus = false;
}
