use super::{Mode, ViewState};
use worldline_core::catalog::TargetRef;
use worldline_core::manuscript::WritingBuffer;

/// 仅个人位置，不包含原文；字节块位置只在完整源指纹相同且目标相同时恢复。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(in crate::app) struct WritingCursor {
    pub target: TargetRef,
    pub source_baseline: String,
    pub mode: String,
    pub block_offset: usize,
    pub cursor: usize,
    #[serde(default)]
    pub secondary: Option<usize>,
    #[serde(default)]
    pub generation: Option<u64>,
    #[serde(default)]
    pub buffer_baseline: Option<String>,
}

pub(in crate::app) fn fingerprint(source: &str) -> String {
    let hash = source.bytes().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}:{}", source.len())
}

impl ViewState {
    pub(in crate::app) fn session_cursor(&self) -> Option<WritingCursor> {
        self.cursor
            .clone()
            .filter(|cursor| cursor.mode == self.mode.key())
    }
    pub(in crate::app) fn cursor_for_buffer(
        &self,
        buffer: &WritingBuffer,
        target: &TargetRef,
    ) -> Option<WritingCursor> {
        if !self.selection_is_current_mode() {
            return None;
        }
        self.session_cursor().filter(|cursor| {
            cursor.target == *target
                && cursor.source_baseline == fingerprint(buffer.source())
                && cursor.generation == Some(buffer.generation())
                && cursor.buffer_baseline.as_deref() == Some(buffer.baseline())
        })
    }
    pub(in crate::app) fn selection_is_current_mode(&self) -> bool {
        self.selection_mode == Some(self.mode)
    }
    pub(in crate::app) fn session_mode(&self) -> Mode {
        self.mode
    }
    pub(in crate::app) fn restore_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.selection_mode = None;
        self.cursor = None;
        self.pending_cursor = None;
        self.pending_focus = true;
    }
    pub(in crate::app) fn focus_existing_editor(&mut self) {
        self.pending_focus = true;
    }
    pub(in crate::app) fn restore_cursor(&mut self, cursor: Option<WritingCursor>) {
        self.pending_cursor = cursor.filter(|cursor| cursor.mode == self.mode.key());
    }
    pub(super) fn prepare_restore(&mut self, buffer: &WritingBuffer, target: &TargetRef) {
        let Some(saved) = &self.pending_cursor else {
            return;
        };
        if saved.target != *target
            || saved.source_baseline != fingerprint(buffer.source())
            || saved.mode != self.mode.key()
            || saved
                .generation
                .is_some_and(|generation| generation != buffer.generation())
            || saved
                .buffer_baseline
                .as_deref()
                .is_some_and(|baseline| baseline != buffer.baseline())
        {
            self.pending_cursor = None;
        }
    }
    pub(super) fn restore_editor(
        &mut self,
        ui: &mut egui::Ui,
        id: egui::Id,
        buffer: &WritingBuffer,
        block_offset: usize,
        text: &str,
    ) {
        let position_key = id.with("writing-position-version");
        let version = format!(
            "{}:{}:{}",
            buffer.baseline(),
            buffer.generation(),
            fingerprint(buffer.source())
        );
        if ui
            .ctx()
            .data(|data| data.get_temp::<String>(position_key))
            .is_some_and(|previous| previous != version)
        {
            let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(0),
                )));
            state.store(ui.ctx(), id);
        }
        super::prepare_text_undo(ui.ctx(), id, text);
        if super::super::search::restore_writing_selection(ui, id, buffer, block_offset, text) {
            self.pending_cursor = None;
            self.pending_focus = false;
            return;
        }
        if self.pending_cursor.is_none() && self.pending_focus {
            ui.memory_mut(|memory| memory.request_focus(id));
            super::super::search::record_navigation_focus(ui.ctx(), id);
            self.pending_focus = false;
        }
        if !self.pending_cursor.as_ref().is_some_and(|cursor| {
            cursor.block_offset == block_offset && cursor.mode == self.mode.key()
        }) {
            return;
        }
        let saved = self.pending_cursor.take().unwrap();
        let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        state.cursor.set_char_range(Some(egui::text::CCursorRange {
            primary: egui::text::CCursor::new(saved.cursor.min(text.chars().count())),
            secondary: egui::text::CCursor::new(
                saved
                    .secondary
                    .unwrap_or(saved.cursor)
                    .min(text.chars().count()),
            ),
            h_pos: None,
        }));
        state.store(ui.ctx(), id);
        ui.memory_mut(|memory| memory.request_focus(id));
        super::super::search::record_navigation_focus(ui.ctx(), id);
        self.pending_focus = false;
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn record_cursor(
        &mut self,
        ui: &mut egui::Ui,
        output: &egui::text_edit::TextEditOutput,
        buffer: &WritingBuffer,
        target: &TargetRef,
        block_offset: usize,
        text: &str,
    ) {
        super::register_input(&output.response);
        super::remember_text_undo(ui.ctx(), output.response.id, text);
        ui.ctx().data_mut(|data| {
            data.insert_temp(
                output.response.id.with("writing-position-version"),
                format!(
                    "{}:{}:{}",
                    buffer.baseline(),
                    buffer.generation(),
                    fingerprint(buffer.source())
                ),
            )
        });
        super::super::search::scroll_editor_selection(ui, output);
        if !output.response.has_focus() || self.composing() {
            return;
        }
        if let Some(range) = output.cursor_range {
            self.selection_mode = Some(self.mode);
            super::super::search::record_editor_selection(
                ui.ctx(),
                output.response.id,
                buffer.path(),
                Some(target),
                buffer.source(),
                block_offset,
                &output.galley.job.text,
                range,
            );
            self.cursor = Some(WritingCursor {
                target: target.clone(),
                source_baseline: fingerprint(buffer.source()),
                mode: self.mode.key().into(),
                block_offset,
                cursor: range.primary.index,
                secondary: Some(range.secondary.index),
                generation: Some(buffer.generation()),
                buffer_baseline: Some(buffer.baseline().into()),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_position_never_restores_against_different_source_or_target() {
        let root = std::env::temp_dir().join("writing-cursor-no-files");
        let mut project = worldline_core::project::Project::new(&root);
        let path = project.entry.clone();
        project
            .set_text(&path, "event first\n  原文。\n".into())
            .unwrap();
        let mut buffer = project.open_source_writing_buffer(&path).unwrap();
        let target = TargetRef::new("event", "first");
        let cursor = WritingCursor {
            target: target.clone(),
            source_baseline: fingerprint(buffer.source()),
            mode: "prose".into(),
            block_offset: 12,
            cursor: 2,
            secondary: None,
            generation: None,
            buffer_baseline: None,
        };
        let mut view = ViewState::default();
        view.restore_cursor(Some(cursor.clone()));
        view.prepare_restore(&buffer, &target);
        assert!(view.pending_cursor.is_some());
        buffer.replace_source("event first\n  新原文。\n".into());
        view.prepare_restore(&buffer, &target);
        assert!(view.pending_cursor.is_none());
        view.restore_cursor(Some(cursor));
        view.prepare_restore(&buffer, &TargetRef::new("event", "another"));
        assert!(view.pending_cursor.is_none());
    }
}

#[cfg(test)]
mod mode_tests {
    use super::*;
    #[test]
    fn cursor_metadata_never_changes_the_explicit_view_mode() {
        let cursor = WritingCursor {
            target: TargetRef::new("event", "first"),
            source_baseline: "unchanged".into(),
            mode: "source".into(),
            block_offset: 0,
            cursor: 3,
            secondary: None,
            generation: None,
            buffer_baseline: None,
        };
        let mut view = ViewState {
            cursor: Some(cursor.clone()),
            ..Default::default()
        };
        assert!(
            view.session_cursor().is_none(),
            "last-focused source cursor is not a prose cursor"
        );
        view.restore_mode(Mode::Prose);
        view.restore_cursor(Some(cursor.clone()));
        assert!(view.pending_cursor.is_none());
        assert_eq!(view.session_mode(), Mode::Prose);
        view.restore_mode(Mode::Source);
        view.restore_cursor(Some(cursor));
        assert!(view.pending_cursor.is_some());
        assert_eq!(view.session_mode(), Mode::Source);
    }
}
