//! 仅保留当前有效选区内的右键意图；不改变 egui 的全局指针规则。
use super::{fingerprint, ViewState};
use egui::{text::CCursorRange, text_edit::TextEditOutput};
use worldline_core::{catalog::TargetRef, manuscript::WritingBuffer};

pub(super) struct SelectionPress {
    id: egui::Id,
    range: CCursorRange,
    point: egui::Pos2,
    generation: u64,
    baseline: String,
    source_fingerprint: String,
    display: String,
}

impl SelectionPress {
    pub fn capture(
        ui: &egui::Ui,
        id: egui::Id,
        view: &ViewState,
        buffer: &WritingBuffer,
        target: &TargetRef,
        offset: usize,
        text: &str,
    ) -> Option<Self> {
        let point = ui.input(|input| {
            if !input.focused
                || input.modifiers.any()
                || input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Text(_)
                            | egui::Event::Paste(_)
                            | egui::Event::Cut
                            | egui::Event::Ime(_)
                            | egui::Event::Touch { .. }
                            | egui::Event::Key { pressed: true, .. }
                            | egui::Event::PointerButton {
                                button: egui::PointerButton::Primary,
                                pressed: true,
                                ..
                            }
                    )
                })
            {
                return None;
            }
            input.events.iter().find_map(|event| match event {
                egui::Event::PointerButton {
                    button: egui::PointerButton::Secondary,
                    pressed: true,
                    pos,
                    ..
                } => Some(*pos),
                _ => None,
            })
        })?;
        if view.ime_active
            || view.composing()
            || egui::Popup::is_any_open(ui.ctx())
            || ui.memory(|memory| memory.focused()) != Some(id)
        {
            return None;
        }
        let cursor = view.cursor_for_buffer(buffer, target)?;
        let range = egui::TextEdit::load_state(ui.ctx(), id)?
            .cursor
            .char_range()?;
        let selected = crate::app::search::editor_selection(ui.ctx())?;
        if range.is_empty()
            || cursor.block_offset != offset
            || cursor.cursor != range.primary.index
            || cursor.secondary != Some(range.secondary.index)
            || selected.id != id
            || selected.path != buffer.path()
            || selected.target.as_ref() != Some(target)
            || selected.source != buffer.source()
            || selected.range.is_empty()
            || !crate::app::search::selection_is_representable(
                buffer.source(),
                offset,
                text,
                &selected.range,
            )
        {
            return None;
        }
        let [min, max] = range.sorted_cursors();
        let start = text
            .char_indices()
            .map(|(byte, _)| byte)
            .chain([text.len()])
            .nth(min.index)?;
        let end = text
            .char_indices()
            .map(|(byte, _)| byte)
            .chain([text.len()])
            .nth(max.index)?;
        if buffer.source().get(selected.range) != text.get(start..end) {
            return None;
        }
        Some(Self {
            id,
            range,
            point,
            generation: buffer.generation(),
            baseline: buffer.baseline().into(),
            source_fingerprint: cursor.source_baseline,
            display: text.into(),
        })
    }

    pub fn restore(self, ui: &egui::Ui, output: &mut TextEditOutput, buffer: &WritingBuffer) {
        // 目标、模式与块位置在同一 editor.show 调用内不变；草稿与实际输出仍重复核对。
        if output.response.id != self.id
            || output.response.changed()
            || !output.response.hovered()
            || !output.response.has_focus()
            || output.galley.text() != self.display
            || buffer.generation() != self.generation
            || buffer.baseline() != self.baseline
            || fingerprint(buffer.source()) != self.source_fingerprint
            || !output.text_clip_rect.contains(self.point)
            || !inside_selection(output, self.range, self.point)
        {
            return;
        }
        output.cursor_range = Some(self.range);
        output.state.cursor.set_char_range(Some(self.range));
        output.state.clone().store(ui.ctx(), self.id);
        ui.ctx().request_repaint();
    }
}

fn inside_selection(output: &TextEditOutput, range: CCursorRange, point: egui::Pos2) -> bool {
    let [min, max] = range.sorted_cursors();
    let min = output.galley.layout_from_cursor(min);
    let max = output.galley.layout_from_cursor(max);
    (min.row..=max.row).any(|index| {
        let Some(row) = output.galley.rows.get(index) else {
            return false;
        };
        let left = if index == min.row {
            row.x_offset(min.column)
        } else {
            0.0
        };
        let right = if index == max.row {
            row.x_offset(max.column)
        } else {
            row.size.x
        };
        // 按本次排版的选区行矩形命中，不能只用最近光标索引把邻字/留白当选区。
        let rect = egui::Rect::from_min_max(egui::pos2(left, 0.0), egui::pos2(right, row.size.y))
            .translate(row.pos.to_vec2() + output.galley_pos.to_vec2());
        rect.left() <= point.x
            && point.x < rect.right()
            && rect.top() <= point.y
            && point.y < rect.bottom()
    })
}
