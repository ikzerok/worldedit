use super::*;
use egui::{Event, Key};

struct Harness {
    ctx: egui::Context,
    directory: PageDirectory,
    preview: ReaderExportPreview,
}

impl Harness {
    fn new(count: usize) -> Self {
        let mut harness = Self {
            ctx: egui::Context::default(),
            directory: PageDirectory::default(),
            preview: preview(count),
        };
        harness.directory.sync(&harness.preview);
        harness.directory.show();
        harness.frame(vec![]);
        harness.frame(vec![]);
        harness
    }

    fn frame(&mut self, mut events: Vec<Event>) -> egui::FullOutput {
        // 每个非重复事件代表一次物理按下/释放，避免将下一帧的测试按下误作长按。
        let releases = events
            .iter()
            .filter_map(|event| match event {
                Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    ..
                } => Some(Event::Key {
                    key: *key,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        events.extend(releases);
        self.ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(440.0, 660.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let _ = ui.button("目录之外的按钮");
                    self.directory.directory_ui(ui, &self.preview, 570.0, true);
                });
            },
        )
    }
}

fn key(key: Key, repeat: bool) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn reader_directory_keyboard_moves_across_pages_and_enter_opens_the_path() {
    let mut h = Harness::new(26);
    for _ in 0..12 {
        h.frame(vec![key(Key::ArrowDown, false)]);
    }
    assert_eq!(h.directory.page, 1);
    assert!(h.directory.visible);
    h.frame(vec![key(Key::Enter, false)]);
    assert!(!h.directory.visible);
    assert_eq!(
        h.directory.opened_path.as_deref(),
        Some("objects/public-0012.html")
    );
    h.directory.show();
    h.frame(vec![]);
    assert_eq!(h.directory.page, 1);
    h.frame(vec![key(Key::Backspace, false)]);
    assert!(
        h.directory.visible,
        "Backspace is text input, never navigation"
    );
}

#[test]
fn reader_directory_ime_preedit_commit_and_repeat_cannot_open_or_move() {
    let mut h = Harness::new(26);
    for event in [
        egui::ImeEvent::Enabled,
        egui::ImeEvent::Preedit("中".into()),
        egui::ImeEvent::Commit("中".into()),
        egui::ImeEvent::Disabled,
    ] {
        h.frame(vec![
            Event::Ime(event),
            key(Key::ArrowDown, false),
            key(Key::Enter, false),
        ]);
        assert!(h.directory.visible);
        assert_eq!(h.directory.page, 0);
    }
    h.directory.query.clear();
    h.frame(vec![]);
    h.frame(vec![
        Event::Ime(egui::ImeEvent::Enabled),
        key(Key::Enter, true),
    ]);
    h.frame(vec![Event::Ime(egui::ImeEvent::Disabled)]);
    h.frame(vec![key(Key::Enter, true)]);
    assert!(h.directory.visible);
    h.frame(vec![Event::Key {
        key: Key::Enter,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    h.frame(vec![key(Key::ArrowDown, false)]);
    h.frame(vec![key(Key::Enter, false)]);
    assert!(!h.directory.visible);
    assert_eq!(h.directory.opened_index(), Some(1));
}

#[test]
fn reader_directory_result_focus_navigation_works_but_outside_focus_is_untouched() {
    let mut h = Harness::new(26);
    let row = h.directory.result_ids[4].0;
    h.ctx.memory_mut(|memory| memory.request_focus(row));
    h.frame(vec![key(Key::ArrowDown, false)]);
    assert_eq!(
        h.directory.candidate.as_deref(),
        Some("objects/public-0003.html")
    );
    assert!(h
        .directory
        .result_ids
        .iter()
        .any(|(id, path)| Some(*id) == h.ctx.memory(|m| m.focused())
            && path == "objects/public-0003.html"));
    let outside = egui::Id::new("unrelated-editor");
    h.ctx.memory_mut(|memory| memory.request_focus(outside));
    h.frame(vec![key(Key::ArrowDown, false), key(Key::Enter, false)]);
    assert!(h.directory.visible);
    assert_eq!(
        h.directory.candidate.as_deref(),
        Some("objects/public-0003.html")
    );
}

#[test]
fn reader_directory_query_edit_and_enter_never_open_an_old_match() {
    let mut h = Harness::new(26);
    h.frame(vec![
        Event::Text("no-public-match".into()),
        key(Key::Enter, false),
    ]);
    assert!(h.directory.visible);
    assert!(h.directory.matches.is_empty());
    h.frame(vec![key(Key::Enter, false)]);
    assert!(h.directory.visible);
    assert_eq!(h.directory.opened_index(), Some(0));
}
