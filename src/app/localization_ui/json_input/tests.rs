use super::*;
use crate::app::localization_ui::focus_tests::{click_label, ctrl_a, draw};
use crate::app::localization_ui::workbench_tests::fixture;

#[test]
fn localization_large_json_load_has_bounded_paint_and_full_core_preview() {
    let (mut project, mut state) = fixture(9);
    state.advanced = true;
    state.string_ids = (0..9)
        .map(|index| format!("line{index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut exchange = project
        .preview_localization_export(&state.selection())
        .unwrap()
        .exchange;
    for entry in &mut exchange.entries {
        entry.translation_parts = Some(entry.source_parts.clone());
    }
    let mut raw = serde_json::to_string(&exchange).unwrap();
    raw.push_str(&" ".repeat(MAX_LOCALIZATION_JSON_BYTES - 1 - raw.len()));
    state.load_import_file(Path::new("near.json"), raw.as_bytes().to_vec());
    let ctx = egui::Context::default();
    for _ in 0..3 {
        let output = draw(&ctx, &mut project, &mut state, vec![]);
        let painted: usize = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().len()),
                _ => None,
            })
            .sum();
        assert!(
            painted < EDIT_BYTES,
            "full JSON reached text layout: {painted}"
        );
        assert_eq!(state.exchange_json, raw);
    }
    assert!(!state.json_input.editing);
    exchange::preview_import(&project, &mut state);
    jobs::settle(&project, &mut state, 1);
    assert!(state.import_plan.as_ref().unwrap().can_apply);
    assert_eq!(state.import_exchange.as_ref().unwrap().entries.len(), 9);
    assert_eq!(state.exchange_json, raw);
    assert!(state.has_unsubmitted_work());
    state.load_import_file(
        Path::new("over.json"),
        vec![b' '; MAX_LOCALIZATION_JSON_BYTES + 1],
    );
    assert_eq!(state.exchange_json, raw);
    assert!(state.status.as_ref().is_some_and(Result::is_err));
}

#[test]
fn localization_json_chunks_cover_all_utf8_bytes_without_summary_substitution() {
    let original = "😀中a".repeat(EDIT_BYTES);
    let mut visited = String::new();
    let mut start = 0;
    while start < original.len() {
        let range = span(&original, start, EDIT_BYTES);
        assert!(range.len() <= EDIT_BYTES);
        assert!(original.is_char_boundary(range.start) && original.is_char_boundary(range.end));
        visited.push_str(&original[range.clone()]);
        start = range.end;
    }
    assert_eq!(visited, original);
}

#[test]
fn localization_large_paste_after_ctrl_a_keeps_original_until_explicit_whole_replacement() {
    let (mut project, mut state) = fixture(1);
    state.advanced = true;
    state.exchange_json = "{\"keep\":\"Original JSON 中文😀\"}".into();
    state.exchange_submitted = true;
    let original = state.exchange_json.clone();
    let ctx = egui::Context::default();
    click_label(&ctx, &mut project, &mut state, &original);
    ctrl_a(&ctx, &mut project, &mut state);
    let id = ctx.memory(|memory| memory.focused()).unwrap();
    let selection = egui::TextEdit::load_state(&ctx, id)
        .unwrap()
        .cursor
        .char_range();
    let paste = format!("{{}}{}", " ".repeat(EDIT_BYTES + 1));
    draw(
        &ctx,
        &mut project,
        &mut state,
        vec![egui::Event::Paste(paste.clone())],
    );
    assert_eq!(
        state.exchange_json, original,
        "selected original must not be deleted before rejecting insertion"
    );
    assert_eq!(state.json_input.pending.as_deref(), Some(paste.as_str()));
    assert_eq!(
        egui::TextEdit::load_state(&ctx, id)
            .unwrap()
            .cursor
            .char_range(),
        selection
    );
    assert!(state.has_unsubmitted_work());
    exchange::preview_import(&project, &mut state);
    assert!(!state.has_pending_work());
    click_label(&ctx, &mut project, &mut state, "取消大粘贴，保留原文");
    assert_eq!(state.exchange_json, original);
    assert_eq!(
        egui::TextEdit::load_state(&ctx, id)
            .unwrap()
            .cursor
            .char_range(),
        selection
    );
    assert!(!state.json_input.has_pending());
    assert!(
        !state.has_unsubmitted_work(),
        "取消待确认粘贴不能把原已提交 JSON 变成新稿"
    );
    click_label(&ctx, &mut project, &mut state, &original);
    ctrl_a(&ctx, &mut project, &mut state);
    draw(
        &ctx,
        &mut project,
        &mut state,
        vec![egui::Event::Paste(paste.clone())],
    );
    click_label(&ctx, &mut project, &mut state, "用此粘贴替换全部 JSON");
    assert_eq!(state.exchange_json, paste);
    assert!(!state.json_input.has_pending());
    assert!(!state.json_input.editing);
    assert!(state.has_unsubmitted_work());
}

#[test]
fn localization_oversized_paste_never_copies_into_a_pending_buffer_or_deletes_selection() {
    let (mut project, mut state) = fixture(1);
    state.advanced = true;
    state.exchange_json = "Keep this original".into();
    let ctx = egui::Context::default();
    click_label(&ctx, &mut project, &mut state, "Keep this original");
    ctrl_a(&ctx, &mut project, &mut state);
    draw(
        &ctx,
        &mut project,
        &mut state,
        vec![egui::Event::Paste(
            "x".repeat(MAX_LOCALIZATION_JSON_BYTES + 1),
        )],
    );
    assert_eq!(state.exchange_json, "Keep this original");
    assert!(!state.json_input.has_pending());
    assert!(state
        .json_input
        .notice
        .as_ref()
        .unwrap()
        .contains("超过 8 MiB"));
}

#[test]
fn localization_segment_rejection_keeps_full_raw_and_the_current_utf8_segment() {
    let (mut project, mut state) = fixture(1);
    state.advanced = true;
    state.exchange_json = format!(
        "{}middle😀{}",
        " ".repeat(EDIT_BYTES),
        " ".repeat(EDIT_BYTES)
    );
    state.json_input.editing = true;
    state.json_input.start = EDIT_BYTES;
    let original = state.exchange_json.clone();
    let range = span(&original, EDIT_BYTES, EDIT_BYTES);
    let segment = original[range].to_owned();
    let ctx = egui::Context::default();
    draw(&ctx, &mut project, &mut state, vec![]);
    let id = egui::Id::new((
        "localization-json",
        project.root.as_path(),
        0u64,
        EDIT_BYTES,
    ));
    ctx.memory_mut(|memory| memory.request_focus(id));
    ctrl_a(&ctx, &mut project, &mut state);
    draw(
        &ctx,
        &mut project,
        &mut state,
        vec![egui::Event::Paste("z".repeat(EDIT_BYTES + 1))],
    );
    assert_eq!(state.exchange_json, original);
    assert_eq!(state.json_input.start, EDIT_BYTES);
    click_label(&ctx, &mut project, &mut state, "取消大粘贴，保留原文");
    assert_eq!(state.exchange_json, original);
    assert_eq!(
        &state.exchange_json[span(&state.exchange_json, state.json_input.start, EDIT_BYTES)],
        segment
    );
}

#[test]
fn localization_segment_overflow_rolls_back_textedit_selection_deletion() {
    let (mut project, mut state) = fixture(1);
    state.advanced = true;
    state.exchange_json = "A".repeat(EDIT_BYTES);
    let original = state.exchange_json.clone();
    let ctx = egui::Context::default();
    draw(&ctx, &mut project, &mut state, vec![]);
    let id = egui::Id::new(("localization-json", project.root.as_path(), 0u64, 0usize));
    let mut editor = egui::TextEdit::load_state(&ctx, id).unwrap();
    editor
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(1),
        )));
    let selected = editor.cursor.char_range();
    editor.store(&ctx, id);
    ctx.memory_mut(|memory| memory.request_focus(id));
    let paste = "B".repeat(2048);
    draw(
        &ctx,
        &mut project,
        &mut state,
        vec![egui::Event::Paste(paste.clone())],
    );
    assert_eq!(state.exchange_json, original);
    assert_eq!(state.json_input.pending.as_deref(), Some(paste.as_str()));
    assert_eq!(
        egui::TextEdit::load_state(&ctx, id)
            .unwrap()
            .cursor
            .char_range(),
        selected
    );
}

#[test]
fn localization_json_segment_edit_preserves_all_other_bytes_and_file_reload_resets_only_on_success()
{
    let (mut project, mut state) = fixture(1);
    state.advanced = true;
    state.exchange_json = "前綴😀middle後綴".repeat(6000);
    state.json_input.editing = true;
    state.json_input.start = boundary(&state.exchange_json, EDIT_BYTES);
    let original = state.exchange_json.clone();
    let range = span(&original, state.json_input.start, EDIT_BYTES);
    let ctx = egui::Context::default();
    draw(&ctx, &mut project, &mut state, vec![]);
    let id = egui::Id::new((
        "localization-json",
        project.root.as_path(),
        0u64,
        range.start,
    ));
    ctx.memory_mut(|memory| memory.request_focus(id));
    ctrl_a(&ctx, &mut project, &mut state);
    draw(
        &ctx,
        &mut project,
        &mut state,
        vec![egui::Event::Text("Edited段😀".into())],
    );
    let expected = format!(
        "{}Edited段😀{}",
        &original[..range.start],
        &original[range.end..]
    );
    assert_eq!(state.exchange_json, expected);
    assert!(state.has_unsubmitted_work());
    state.load_import_file(Path::new("bad.json"), vec![255]);
    assert_eq!(state.exchange_json, expected);
    assert_eq!(state.json_input.start, range.start);
    assert!(state.json_input.editing);
    state.load_import_file(Path::new("fresh.json"), b"{}".to_vec());
    assert_eq!(state.exchange_json, "{}");
    assert_eq!(state.json_input.start, 0);
    assert!(!state.json_input.editing);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn localization_import_reader_stops_at_limit_plus_one_even_if_file_grew_after_metadata() {
    use std::io::Read;
    struct GrowingReader {
        read: usize,
        total: usize,
    }
    impl std::io::Read for GrowingReader {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            let count = output.len().min(self.total - self.read);
            output[..count].fill(b' ');
            self.read += count;
            Ok(count)
        }
    }
    let mut growing = GrowingReader {
        read: 0,
        total: MAX_LOCALIZATION_JSON_BYTES * 3,
    };
    assert!(exchange::read_import_bytes(&mut growing).is_err());
    assert_eq!(growing.read, MAX_LOCALIZATION_JSON_BYTES + 1);
    let exact =
        exchange::read_import_bytes(std::io::repeat(b' ').take(MAX_LOCALIZATION_JSON_BYTES as u64))
            .unwrap();
    assert_eq!(exact.len(), MAX_LOCALIZATION_JSON_BYTES);
}
