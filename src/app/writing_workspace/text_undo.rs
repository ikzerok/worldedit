//! 输入控件的局部撤销只能引用最后一次显示的同一稿件。
//! 正文/源码互改或Project事务改变内容后，清除该控件过期历史并以当前值为新基线。
use egui::{Context, Id};

pub(in crate::app) fn prepare_text_undo(ctx: &Context, id: Id, text: &str) {
    let key = id.with("text-undo-content");
    let fingerprint = super::session::fingerprint(text);
    if ctx.data(|data| data.get_temp::<String>(key)).as_ref() == Some(&fingerprint) {
        return;
    }
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    let cursor = state
        .cursor
        .char_range()
        .unwrap_or_else(|| egui::text::CCursorRange::one(egui::text::CCursor::new(0)));
    state.clear_undoer();
    let mut undoer = state.undoer();
    undoer.add_undo(&(cursor, text.to_owned()));
    state.set_undoer(undoer);
    state.store(ctx, id);
    ctx.data_mut(|data| data.insert_temp(key, fingerprint));
}

pub(in crate::app) fn remember_text_undo(ctx: &Context, id: Id, text: &str) {
    ctx.data_mut(|data| {
        data.insert_temp(
            id.with("text-undo-content"),
            super::session::fingerprint(text),
        )
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn seed(ctx: &Context, id: Id, text: &str) {
        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
        let mut undoer = state.undoer();
        undoer.add_undo(&(
            egui::text::CCursorRange::one(egui::text::CCursor::new(0)),
            text.into(),
        ));
        state.set_undoer(undoer);
        state.store(ctx, id);
        remember_text_undo(ctx, id, text);
    }
    #[test]
    fn external_draft_change_replaces_stale_local_undo_but_current_typing_keeps_history() {
        let ctx = Context::default();
        let id = Id::new("same-source-editor");
        prepare_text_undo(&ctx, id, "original");
        seed(&ctx, id, "freshneedle");
        // 另一个正文视图或Project撤销使当前原文改变。
        prepare_text_undo(&ctx, id, "livepreview");
        seed(&ctx, id, "livepreview if (");
        prepare_text_undo(&ctx, id, "livepreview if (");
        let state = egui::TextEdit::load_state(&ctx, id).unwrap();
        let mut undoer = state.undoer();
        let current = (
            egui::text::CCursorRange::one(egui::text::CCursor::new(0)),
            "livepreview if (".into(),
        );
        assert_eq!(undoer.undo(&current).unwrap().1, "livepreview");
        assert!(!undoer.has_undo(&(current.0, "livepreview".into())));
    }
}
