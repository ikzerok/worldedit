//! The full exchange is authoritative; only a bounded UTF-8 slice reaches text layout.
use super::*;
use egui::TextBuffer;
use std::ops::Range;

pub(super) const EDIT_BYTES: usize = 16 * 1024;
const SUMMARY_BYTES: usize = 2048;

#[derive(Default)]
pub(super) struct JsonInput {
    start: usize,
    editing: bool,
    generation: u64,
    pending: Option<String>,
    notice: Option<String>,
}

impl JsonInput {
    pub(super) fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn reset(&mut self) {
        *self = Self {
            generation: self.generation.wrapping_add(1),
            ..Default::default()
        };
    }
}

pub(super) enum Change {
    None,
    Edited,
    Pending,
}

pub(super) fn boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

pub(super) fn ceil_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while !text.is_char_boundary(offset) {
        offset += 1;
    }
    offset
}

fn span(text: &str, start: usize, limit: usize) -> Range<usize> {
    let start = boundary(text, start);
    start..boundary(text, start.saturating_add(limit))
}

fn summary(ui: &mut Ui, raw: &str) {
    let range = span(raw, 0, SUMMARY_BYTES);
    ui.small(format!(
        "只显示前 {} 字节；完整 {} 字节仍保留，预览导入使用全文",
        range.len(),
        raw.len()
    ));
    ui.add(egui::Label::new(RichText::new(&raw[range]).monospace()).wrap());
}

pub(super) fn show(ui: &mut Ui, root: &Path, raw: &mut String, state: &mut JsonInput) -> Change {
    ui.small(format!(
        "交换 JSON · {} 字节 / 上限 {} 字节",
        raw.len(),
        MAX_LOCALIZATION_JSON_BYTES
    ));
    if let Some(pending) = &state.pending {
        let mut replace = false;
        let mut cancel = false;
        ui.label(format!(
            "待确认的大粘贴 · {} 字节。原 JSON 和当前编辑段均未改变；确认会替换全部 JSON。",
            pending.len()
        ));
        ui.horizontal_wrapped(|ui| {
            replace = ui
                .button("用此粘贴替换全部 JSON")
                .reveal_focus(ui)
                .clicked();
            cancel = ui.button("取消大粘贴，保留原文").reveal_focus(ui).clicked();
        });
        if replace {
            *raw = state.pending.take().unwrap();
            state.reset();
            return Change::Edited;
        }
        if cancel {
            state.pending = None;
            state.notice = None;
        } else {
            summary(ui, raw);
            return Change::None;
        }
    }
    if let Some(notice) = &state.notice {
        ui.colored_label(crate::theme::WARNING(), notice);
    }
    if raw.len() > EDIT_BYTES {
        if !state.editing {
            summary(ui, raw);
            if ui.button("分段编辑完整 JSON").reveal_focus(ui).clicked() {
                state.editing = true;
            } else {
                return Change::None;
            }
        }
        let range = span(raw, state.start, EDIT_BYTES);
        ui.small("每段最多排版 16 KiB；修改只替换本段，其余原文字节保持不变");
        ui.horizontal_wrapped(|ui| {
            if ui.button("首段").reveal_focus(ui).clicked() {
                state.start = 0;
            }
            if crate::theme::add_enabled(ui, range.start > 0, egui::Button::new("上一段"))
                .reveal_focus(ui)
                .clicked()
            {
                state.start = boundary(raw, range.start.saturating_sub(EDIT_BYTES));
            }
            if crate::theme::add_enabled(ui, range.end < raw.len(), egui::Button::new("下一段"))
                .reveal_focus(ui)
                .clicked()
            {
                state.start = range.end;
            }
            if ui.button("末段").reveal_focus(ui).clicked() {
                state.start = ceil_boundary(raw, raw.len().saturating_sub(EDIT_BYTES));
            }
            if ui.button("返回有限摘要").reveal_focus(ui).clicked() {
                state.editing = false;
            }
        });
        if !state.editing {
            return Change::None;
        }
    } else {
        state.start = 0;
    }
    let range = span(raw, state.start, EDIT_BYTES);
    state.start = range.start;
    if raw.len() > EDIT_BYTES {
        ui.label(format!(
            "当前字节 {}–{} / {}",
            range.start + 1,
            range.end,
            raw.len()
        ));
    }
    let id = egui::Id::new(("localization-json", root, state.generation, range.start));
    if ui.memory(|memory| memory.has_focus(id))
        || ui
            .ctx()
            .read_response(id)
            .is_some_and(|response| response.clicked())
    {
        // Move a large event out before TextEdit clones filtered events or deletes a selection.
        let paste = ui.input_mut(|input| {
            input.events.iter_mut().find_map(|event| match event {
                egui::Event::Paste(text) | egui::Event::Text(text) if text.len() > EDIT_BYTES => {
                    Some(std::mem::take(text))
                }
                _ => None,
            })
        });
        if let Some(paste) = paste {
            if paste.len() > MAX_LOCALIZATION_JSON_BYTES {
                state.notice = Some("粘贴超过 8 MiB 预算；原完整 JSON 与当前段均未改变".into());
            } else {
                state.pending = Some(paste);
                return Change::Pending;
            }
        }
    }
    let previous = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
    let capacity =
        EDIT_BYTES.min(MAX_LOCALIZATION_JSON_BYTES.saturating_sub(raw.len() - range.len()));
    let mut buffer = BoundedBuffer {
        text: raw[range.clone()].to_owned(),
        capacity,
        rejected: None,
    };
    let response = ui
        .add(
            egui::TextEdit::multiline(&mut buffer)
                .id(id)
                .code_editor()
                .desired_rows(8)
                .hint_text("选择 .json 文件，或粘贴 UTF-8 JSON"),
        )
        .reveal_focus(ui);
    if let Some(rejected) = buffer.rejected {
        // TextEdit deletes a selection before insertion. Reject the whole staged edit,
        // including that deletion, and restore its cursor instead of committing a partial value.
        let mut previous = previous;
        previous.clear_undoer();
        previous.store(ui.ctx(), id);
        state.notice = Some(rejected.notice);
        if let Some(paste) = rejected.paste {
            state.pending = Some(paste);
            return Change::Pending;
        }
        return Change::None;
    }
    if response.changed() {
        raw.replace_range(range, &buffer.text);
        state.notice = None;
        return Change::Edited;
    }
    Change::None
}

struct Rejected {
    paste: Option<String>,
    notice: String,
}

struct BoundedBuffer {
    text: String,
    capacity: usize,
    rejected: Option<Rejected>,
}

impl TextBuffer for BoundedBuffer {
    fn is_mutable(&self) -> bool {
        true
    }
    fn as_str(&self) -> &str {
        &self.text
    }
    fn insert_text(&mut self, text: &str, char_index: usize) -> usize {
        if self.rejected.is_some() {
            return 0;
        }
        if text.len() > MAX_LOCALIZATION_JSON_BYTES {
            self.rejected = Some(Rejected {
                paste: None,
                notice: "粘贴超过 8 MiB 预算；原完整 JSON 与当前段均未改变".into(),
            });
            return 0;
        }
        if self.text.len().saturating_add(text.len()) > self.capacity {
            self.rejected = Some(Rejected {
                paste: (text.chars().take(2).count() > 1).then(|| text.to_owned()),
                notice: "输入超过当前段或全文预算；原完整 JSON 与当前段均未改变。可先删减本段或处理待确认粘贴".into(),
            });
            return 0;
        }
        self.text.insert_text(text, char_index)
    }
    fn delete_char_range(&mut self, char_range: Range<usize>) {
        if self.rejected.is_none() {
            self.text.delete_char_range(char_range);
        }
    }
    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<Self>()
    }
}

#[cfg(test)]
mod tests;
