use super::super::super::WorldeditApp;
use std::path::Path;
pub(super) fn char_to_byte(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map(|(byte, _)| byte)
        .unwrap_or(text.len())
}

pub(super) fn active_mention(text: &str, cursor_char: usize) -> Option<(usize, String)> {
    let prefix: String = text.chars().take(cursor_char).collect();
    let (at_byte, query) = prefix.rsplit_once('@')?;
    if query.chars().any(char::is_whitespace) || query.contains('@') {
        return None;
    }
    Some((at_byte.chars().count(), query.to_owned()))
}

pub(super) fn source_link_at_cursor(
    source: &str,
    path: &Path,
    cursor_char: usize,
    links: &[worldline_core::navigation::TextLinkInfo],
) -> Option<worldline_core::TargetRef> {
    let cursor_byte = char_to_byte(source, cursor_char);
    let lines: Vec<_> = source.split_inclusive('\n').collect();
    for link in links.iter().filter(|link| Path::new(&link.file) == path) {
        let line_index = link.line.saturating_sub(1) as usize;
        let Some(raw_line) = lines.get(line_index) else {
            continue;
        };
        let line = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let Ok(markup) =
            worldline_core::navigation::link_source(&link.target, &link.label, &link.file)
        else {
            continue;
        };
        let line_start: usize = lines.iter().take(line_index).map(|line| line.len()).sum();
        for (offset, _) in line.match_indices(&markup) {
            let start = line_start + offset;
            let end = start + markup.len();
            if (start..=end).contains(&cursor_byte) {
                return Some(link.target.clone());
            }
        }
    }
    None
}

pub(super) fn source_selection(
    source: &str,
    path: &Path,
    range: egui::text::CCursorRange,
) -> Option<worldline_core::authoring_intents::TextSelection> {
    let start = range.primary.index.min(range.secondary.index);
    let end = range.primary.index.max(range.secondary.index);
    if start == end {
        return None;
    }
    let byte_start = char_to_byte(source, start);
    let byte_end = char_to_byte(source, end);
    let selected = source.get(byte_start..byte_end)?;
    if selected.trim().is_empty() || selected.contains(['\n', '\r']) {
        return None;
    }
    Some(worldline_core::authoring_intents::TextSelection {
        path: path.to_path_buf(),
        start: byte_start,
        end: byte_end,
        expected_text: selected.to_owned(),
    })
}

impl WorldeditApp {
    pub(super) fn apply_source_mention(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
        at_char: usize,
        query: &str,
        target: worldline_core::TargetRef,
    ) {
        use worldline_core::authoring_intents::{AuthoringIntent, IntentTarget, TextSelection};

        let Some(source) = self.project.document(path).ok().map(str::to_owned) else {
            self.io_error = Some("活动源码已关闭，未插入引用".into());
            return;
        };
        let trigger = char_to_byte(&source, at_char);
        let Some(after_trigger) = trigger.checked_add(1) else {
            self.io_error = Some("@ 引用位置已失效".into());
            return;
        };
        if source.get(trigger..after_trigger) != Some("@") {
            self.io_error = Some("@ 引用位置已变化，请重新输入".into());
            return;
        }
        let mut candidate = self.project.clone();
        let mut staged_source = source;
        staged_source.replace_range(trigger..after_trigger, "");
        if let Err(error) = candidate.set_text(path, staged_source.clone()) {
            self.io_error = Some(error);
            return;
        }
        let selection_start = char_to_byte(&staged_source, at_char);
        let selection_end = selection_start + query.len();
        let intent = AuthoringIntent {
            expected_baseline: candidate.content_baseline(),
            target: IntentTarget::Existing(target.clone()),
            selection: Some(TextSelection {
                path: path.to_path_buf(),
                start: selection_start,
                end: selection_end,
                expected_text: query.to_owned(),
            }),
            placement: None,
        };
        let before = self.project.clone();
        match candidate.apply_authoring_intent(&intent) {
            Ok(_) => {
                self.project = candidate;
                let applied = self.finish_content_command(
                    before,
                    Ok(()),
                    "正文引用已插入；保存全部可写入作品目录",
                );
                if applied {
                    let link = worldline_core::navigation::link_source(
                        &target,
                        query,
                        &path.to_string_lossy(),
                    );
                    if let Ok(link) = link {
                        let cursor = at_char + link.chars().count();
                        let id = egui::Id::new(("source", path));
                        let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(
                                egui::text::CCursor::new(cursor),
                            )));
                        egui::TextEdit::store_state(ctx, id, state);
                        ctx.memory_mut(|memory| memory.request_focus(id));
                    }
                }
            }
            Err(error) => self.io_error = Some(error),
        }
    }
}
