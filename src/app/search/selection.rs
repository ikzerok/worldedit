use std::{
    ops::Range,
    path::{Path, PathBuf},
};
use worldline_core::TargetRef;
#[derive(Clone)]
pub(crate) struct EditorSelection {
    pub path: PathBuf,
    pub target: Option<TargetRef>,
    pub range: Range<usize>,
    pub id: egui::Id,
    pub source: String,
}
#[derive(Clone)]
struct Pending {
    path: PathBuf,
    source: String,
    range: Range<usize>,
    generation: Option<u64>,
    diagnostic: bool,
}
fn selection_id() -> egui::Id {
    egui::Id::new("search-editor-selection")
}
fn restored_focus_id() -> egui::Id {
    egui::Id::new("search-restored-widget")
}
pub(super) fn clear_restored_focus(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove::<egui::Id>(restored_focus_id()));
}
pub(super) fn take_restored_focus(ctx: &egui::Context) -> Option<egui::Id> {
    ctx.data_mut(|data| {
        let id = data.get_temp::<egui::Id>(restored_focus_id());
        data.remove::<egui::Id>(restored_focus_id());
        id
    })
}
pub(crate) fn record_navigation_focus(ctx: &egui::Context, id: egui::Id) {
    ctx.data_mut(|data| data.insert_temp(restored_focus_id(), id));
}
fn pending_id() -> egui::Id {
    egui::Id::new("search-editor-pending")
}
/// 显示正文去缩进后逐行与原稿对齐；只接受准确对应，不猜位置。
fn mapping(full: &str, offset: usize, display: &str) -> Option<Vec<usize>> {
    let tail = full.get(offset..)?;
    if tail.starts_with(display) {
        return Some(
            display
                .char_indices()
                .map(|(at, _)| offset + at)
                .chain([offset + display.len()])
                .collect(),
        );
    }
    let mut result = Vec::new();
    let mut at = offset;
    for (index, line) in display.split_inclusive('\n').enumerate() {
        let end = full[at..]
            .find('\n')
            .map(|end| at + end + 1)
            .unwrap_or(full.len());
        let raw = full.get(at..end)?;
        let body = line.trim_end_matches(['\r', '\n']);
        let raw_body = raw.trim_end_matches(['\r', '\n']);
        let prefix = raw_body.strip_suffix(body)?;
        if !prefix.chars().all(|c| matches!(c, ' ' | '\t')) {
            return None;
        }
        let skip = if index == 0 { 0 } else { prefix.len() };
        if raw_body.get(skip..) != Some(body) {
            return None;
        }
        result.extend(body.char_indices().map(|(byte, _)| at + skip + byte));
        if line.ends_with('\n') {
            result.push(end - 1);
        }
        at = end;
    }
    result.push(if display.ends_with('\n') {
        at
    } else {
        offset
            + full[offset..]
                .char_indices()
                .nth(display.chars().count())
                .map(|(at, _)| at)
                .unwrap_or(full[offset..].len())
    });
    // 最终边界取末尾显示行实际源位置，兼容多行去缩进与 CRLF。
    if !display.ends_with('\n') {
        let last_len = display.rsplit('\n').next().unwrap_or("").len();
        let end = full[..at].trim_end_matches(['\r', '\n']).len();
        if last_len > 0 || display.is_empty() {
            *result.last_mut()? = if display.is_empty() { offset } else { end };
        }
    }
    Some(result)
}
pub(crate) fn selection_is_representable(
    full: &str,
    offset: usize,
    display: &str,
    range: &Range<usize>,
) -> bool {
    mapping(full, offset, display)
        .is_some_and(|map| map.contains(&range.start) && map.contains(&range.end))
}
pub(crate) fn clear_pending_selection(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove::<Pending>(pending_id()));
    clear_restored_focus(ctx);
}
// 显式携带原文、显示文与目标身份，避免跨视图复用失效选区。
#[allow(clippy::too_many_arguments)]
pub(crate) fn record_editor_selection(
    ctx: &egui::Context,
    id: egui::Id,
    path: &Path,
    target: Option<&TargetRef>,
    full_source: &str,
    block_offset: usize,
    display_text: &str,
    range: egui::text::CCursorRange,
) {
    let Some(map) = mapping(full_source, block_offset, display_text) else {
        return;
    };
    let a = range.primary.index.min(range.secondary.index);
    let b = range.primary.index.max(range.secondary.index);
    let Some((&start, &end)) = map.get(a).zip(map.get(b)) else {
        return;
    };
    ctx.data_mut(|d| {
        d.insert_temp(
            selection_id(),
            EditorSelection {
                path: path.to_owned(),
                target: target.cloned(),
                range: start..end,
                id,
                source: full_source.into(),
            },
        )
    });
}
pub(crate) fn editor_selection(ctx: &egui::Context) -> Option<EditorSelection> {
    ctx.data(|d| d.get_temp(selection_id()))
}
/// 源内容绑定的即时定位；草稿搜索必须另用携带真实代次的 request_writing_selection。
pub(crate) fn request_selection(
    ctx: &egui::Context,
    path: PathBuf,
    source: String,
    range: Range<usize>,
) {
    ctx.data_mut(|d| {
        d.insert_temp(
            pending_id(),
            Pending {
                path,
                source,
                range,
                generation: None,
                diagnostic: false,
            },
        )
    });
}
/// 诊断定位仍选择原文，但不把程序选区当作作者建档意图。
pub(crate) fn request_diagnostic_selection(
    ctx: &egui::Context,
    path: PathBuf,
    source: String,
    range: Range<usize>,
) {
    request_selection(ctx, path, source, range);
    mark_pending_selection_programmatic(ctx);
}

/// 复用精确程序选区 origin；人工重新选区会清除，显式 CtrlEnter 不受抑制。
pub(crate) fn mark_pending_selection_programmatic(ctx: &egui::Context) {
    ctx.data_mut(|data| {
        if let Some(mut pending) = data.get_temp::<Pending>(pending_id()) {
            pending.diagnostic = true;
            data.insert_temp(pending_id(), pending);
        }
    });
}

pub(crate) fn request_writing_selection(
    ctx: &egui::Context,
    path: PathBuf,
    source: String,
    range: Range<usize>,
    generation: u64,
) {
    ctx.data_mut(|data| {
        data.insert_temp(
            pending_id(),
            Pending {
                path,
                source,
                range,
                generation: Some(generation),
                diagnostic: false,
            },
        )
    });
}
pub(crate) fn restore_editor_selection(
    ui: &mut egui::Ui,
    id: egui::Id,
    path: &Path,
    full_source: &str,
    block_offset: usize,
    display_text: &str,
) -> bool {
    restore_selection(ui, id, path, full_source, block_offset, display_text, None)
}
pub(crate) fn restore_writing_selection(
    ui: &mut egui::Ui,
    id: egui::Id,
    buffer: &worldline_core::manuscript::WritingBuffer,
    block_offset: usize,
    display_text: &str,
) -> bool {
    restore_selection(
        ui,
        id,
        buffer.path(),
        buffer.source(),
        block_offset,
        display_text,
        Some(buffer.generation()),
    )
}
#[allow(clippy::too_many_arguments)]
fn restore_selection(
    ui: &mut egui::Ui,
    id: egui::Id,
    path: &Path,
    full_source: &str,
    block_offset: usize,
    display_text: &str,
    generation: Option<u64>,
) -> bool {
    let Some(pending) = ui.ctx().data(|d| d.get_temp::<Pending>(pending_id())) else {
        return false;
    };
    if pending.path != path {
        return false;
    }
    if pending.source != full_source
        || pending
            .generation
            .is_some_and(|expected| generation != Some(expected))
    {
        clear_pending_selection(ui.ctx());
        return false;
    }
    let Some(map) = mapping(full_source, block_offset, display_text) else {
        return false;
    };
    let Some(start) = map.iter().position(|at| *at == pending.range.start) else {
        return false;
    };
    let Some(end) = map.iter().position(|at| *at == pending.range.end) else {
        return false;
    };
    let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(end),
        )));
    super::selection_origin::restore_origin(
        ui.ctx(),
        id,
        state.cursor.char_range(),
        pending.diagnostic,
    );
    state.store(ui.ctx(), id);
    ui.memory_mut(|m| m.request_focus(id));
    ui.ctx().data_mut(|d| {
        d.remove::<Pending>(pending_id());
        d.insert_temp(restored_focus_id(), id);
        d.insert_temp(id.with("search-scroll"), true);
    });
    true
}
pub(crate) fn scroll_editor_selection(ui: &mut egui::Ui, output: &egui::text_edit::TextEditOutput) {
    if ui
        .ctx()
        .data_mut(|d| d.remove_temp::<bool>(output.response.id.with("search-scroll")))
        .unwrap_or(false)
    {
        if let Some(range) = output.cursor_range {
            let rect = output
                .galley
                .pos_from_cursor(range.primary)
                .translate(output.galley_pos.to_vec2());
            ui.scroll_to_rect(rect, Some(egui::Align::Center));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_mapping_handles_source_indent_prose_crlf_and_unicode() {
        let full = "event a\r\n  中文\r\n  次行\r\n";
        assert_eq!(
            mapping(full, 0, full).unwrap().len(),
            full.chars().count() + 1
        );
        let offset = full.find("中文").unwrap();
        let map = mapping(full, offset, "中文\n次行").unwrap();
        assert_eq!(&full[map[3]..map[5]], "次行");
        assert!(mapping(full, offset, "已变\n次行").is_none());
    }
}

#[cfg(test)]
mod generation_tests {
    use super::*;
    #[test]
    fn content_position_api_remains_compatible_and_explicit_draft_generation_is_strict() {
        let root = std::env::temp_dir().join("search-position-version-no-files");
        let mut project = worldline_core::project::Project::new(&root);
        let path = project.entry.clone();
        project
            .set_text(&path, "event start\n  中文needle\n".into())
            .unwrap();
        let buffer = project.open_source_writing_buffer(&path).unwrap();
        let source = buffer.source().to_owned();
        let start = source.find("needle").unwrap();
        let ctx = egui::Context::default();
        let id = egui::Id::new("compatible-source-position");
        request_writing_selection(
            &ctx,
            path.clone(),
            source.clone(),
            start..start + 6,
            buffer.generation() + 1,
        );
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!restore_writing_selection(ui, id, &buffer, 0, &source));
            });
        });
        request_selection(&ctx, path, source.clone(), start..start + 6);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(restore_writing_selection(ui, id, &buffer, 0, &source));
            });
        });
    }
}
