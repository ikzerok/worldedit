//! 只记录一轮诊断定位的选择来源；不改变光标、正文或焦点。
use egui::{text::CCursorRange, Context, Id, Key};

fn origin_id(id: Id) -> Id {
    id.with("diagnostic-selection-origin")
}

pub(crate) fn restore_origin(
    ctx: &Context,
    id: Id,
    range: Option<CCursorRange>,
    diagnostic: bool,
) {
    ctx.data_mut(|data| {
        data.remove::<CCursorRange>(origin_id(id));
        if diagnostic {
            if let Some(range) = range {
                data.insert_temp(origin_id(id), range);
            }
        }
    });
}

pub(crate) fn selection_is_diagnostic(ctx: &Context, id: Id, range: Option<CCursorRange>) -> bool {
    let saved = ctx.data(|data| data.get_temp::<CCursorRange>(origin_id(id)));
    saved.zip(range).is_some_and(|(saved, range)| {
        saved.primary.index == range.primary.index
            && saved.secondary.index == range.secondary.index
    })
}

pub(crate) fn observe_manual_selection(
    ctx: &Context,
    output: &egui::text_edit::TextEditOutput,
    restored_this_frame: bool,
) {
    if restored_this_frame {
        return;
    }
    let manual = output.response.has_focus()
        && ctx.input(|input| {
            let pointer = input.pointer.primary_down()
                && input.pointer.interact_pos().is_some_and(|pos| output.response.rect.contains(pos));
            pointer || input.events.iter().any(|event| matches!(event,
                egui::Event::Key { key, pressed: true, modifiers, .. }
                    if matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown
                        | Key::Home | Key::End | Key::PageUp | Key::PageDown)
                        || *key == Key::A && modifiers.command))
        });
    if manual || output.response.changed() {
        restore_origin(ctx, output.response.id, None, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_tracks_exact_selection_direction_and_can_be_restored_by_back() {
        let ctx = Context::default();
        let id = Id::new("source-origin");
        let range = CCursorRange::two(egui::text::CCursor::new(2), egui::text::CCursor::new(7));
        restore_origin(&ctx, id, Some(range), true);
        assert!(selection_is_diagnostic(&ctx, id, Some(range)));
        assert!(!selection_is_diagnostic(&ctx, Id::new("other"), Some(range)));
        let reversed = CCursorRange { primary: range.secondary, secondary: range.primary, h_pos: None };
        assert!(!selection_is_diagnostic(&ctx, id, Some(reversed)));
        restore_origin(&ctx, id, Some(range), false);
        assert!(!selection_is_diagnostic(&ctx, id, Some(range)));
        restore_origin(&ctx, id, Some(range), true);
        assert!(selection_is_diagnostic(&ctx, id, Some(range)));
    }
}
