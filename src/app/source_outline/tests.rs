//! 实际 egui 帧测试；不冒充原生窗口、真实浏览器或物理 IME 验收。
use super::*;
use egui::{
    text::{CCursor, CCursorRange},
    Context, Event, Key, Modifiers, Rect, Vec2,
};
use std::sync::atomic::{AtomicUsize, Ordering};
mod compact;
mod geometry;
mod lifecycle;

struct Harness {
    ctx: Context,
    app: WorldeditApp,
    size: Vec2,
    tick: u32,
}
impl Harness {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let ctx = Context::default();
        let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        let root = std::env::temp_dir().join(format!(
            "source-outline-ui-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        app.project = worldline_core::project::Project::new(&root);
        app.active_file = app.project.entry.clone();
        app.project
            .documents
            .retain(|path, _| path == &app.active_file);
        app.project
            .set_text(&app.active_file, source.into())
            .unwrap();
        app.project.create_authoring_document(&root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.13","required_features":["content.entities.v1"]}"#.to_vec()).unwrap();
        app.project.save().unwrap();
        app.recompile();
        app.tab = Tab::Edit;
        app.personal.pending_restore = false;
        app.personal.settings.navigation = false;
        app.personal.settings.diagnostics = false;
        let mut harness = Self {
            ctx,
            app,
            size: egui::vec2(1188.0, 848.0),
            tick: 0,
        };
        harness.frame(vec![]);
        harness.ctx.style_mut(|style| {
            style.animation_time = 0.0;
            style.scroll_animation = egui::style::ScrollAnimation::none();
        });
        harness.settle();
        harness
    }
    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.tick += 1;
        self.ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                time: Some(f64::from(self.tick) / 60.0),
                events,
                ..Default::default()
            },
            |ctx| eframe::App::update(&mut self.app, ctx, &mut eframe::Frame::_new_kittest()),
        )
    }
    fn settle(&mut self) -> egui::FullOutput {
        for _ in 0..24 {
            self.frame(vec![]);
        }
        self.frame(vec![])
    }
    fn press(&mut self, key: Key, modifiers: Modifiers) {
        for pressed in [true, false] {
            self.frame(vec![key_event(key, modifiers, pressed)]);
        }
    }
    fn source_id(&self) -> egui::Id {
        egui::Id::new(("source", &self.app.active_file))
    }
    fn select(&mut self, primary: usize, secondary: usize) {
        let id = self.source_id();
        let mut state = egui::TextEdit::load_state(&self.ctx, id).unwrap_or_default();
        state.cursor.set_char_range(Some(CCursorRange {
            primary: CCursor::new(primary),
            secondary: CCursor::new(secondary),
            h_pos: None,
        }));
        state.store(&self.ctx, id);
        self.ctx.memory_mut(|memory| memory.request_focus(id));
        self.frame(vec![]);
    }
    fn range(&self) -> CCursorRange {
        egui::TextEdit::load_state(&self.ctx, self.source_id())
            .unwrap()
            .cursor
            .char_range()
            .unwrap()
    }
    fn open(&mut self) {
        self.press(Key::O, Modifiers::COMMAND | Modifiers::SHIFT);
        self.settle();
        assert!(self.app.source_outline.open);
    }
    fn source(&self) -> &str {
        self.app.project.document(&self.app.active_file).unwrap()
    }
    fn change(&mut self, source: &str) {
        self.app
            .project
            .set_text(&self.app.active_file.clone(), source.into())
            .unwrap();
        self.app.recompile();
    }
    fn click(&mut self, label: &str) {
        let out = self.settle();
        let point = text_rect(&out, label)
            .unwrap_or_else(|| panic!("missing {label}: {}", all_text(&out)))
            .center();
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ]);
        }
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.app.project.root);
    }
}
fn key_event(key: Key, modifiers: Modifiers, pressed: bool) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers,
    }
}
fn text_shapes(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        egui::Shape::Text(text) => out.push((
            text.galley.job.text.clone(),
            text.galley.rect.translate(text.pos.to_vec2()),
        )),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                text_shapes(shape, out);
            }
        }
        _ => {}
    }
}
fn text_rect(output: &egui::FullOutput, label: &str) -> Option<Rect> {
    output.shapes.iter().find_map(|shape| {
        let mut texts = Vec::new();
        text_shapes(&shape.shape, &mut texts);
        texts.into_iter().find_map(|(text, rect)| {
            (text == label && shape.clip_rect.intersects(rect)).then_some(rect)
        })
    })
}
fn all_text(output: &egui::FullOutput) -> String {
    let mut texts = Vec::new();
    for shape in &output.shapes {
        text_shapes(&shape.shape, &mut texts);
    }
    texts
        .into_iter()
        .map(|(text, _)| text)
        .collect::<Vec<_>>()
        .join("\n")
}
fn source() -> &'static str {
    "// 文件注释：不属于声明🌊\r\nentity keeper kind person as \"同名灯室\"\r\nentity keeper kind place as \"同名灯室\"\r\nevent first as \"第一章\"\r\n  scene room\r\n    scene inner\r\n      中文🌊正文\r\n  -> END\r\nevent second as \"第二章\"\r\n  scene room\r\n    中文尾稿\r\n  -> END\r\n\r\n"
}

#[test]
fn source_outline_exact_unicode_crlf_identity_navigation_and_repeated_back() {
    let mut h = Harness::new(source());
    h.select(8, 2);
    let before = h.range();
    let baseline = h.app.project.content_baseline();
    let undo = h.app.history.len();
    h.open();
    let cache = h.app.source_outline.cache.as_ref().unwrap();
    assert_eq!(
        cache.outline.status,
        SourceOutlineStatus::Ready,
        "{:?}",
        cache.outline.message
    );
    assert_eq!(filtered(&cache.outline, "同名灯室"), vec![0, 1]);
    assert_eq!(filtered(&cache.outline, "first.room.inner"), vec![4]);
    assert_eq!(filtered(&cache.outline, "second.room"), vec![6]);
    assert_eq!(filtered(&cache.outline, "人物"), Vec::<usize>::new());
    assert_eq!(filtered(&cache.outline, "place"), vec![1]);
    h.press(Key::End, Modifiers::NONE);
    h.settle();
    assert_eq!(h.app.source_outline.selected, 6);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(!h.app.source_outline.open);
    let start = source().rfind("scene room").unwrap();
    let range = h.range().as_sorted_char_range();
    assert_eq!(
        range,
        source()[..start].chars().count()..source()[..start + "scene room".len()].chars().count()
    );
    let last = h.range();
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.source_id()));
    h.open();
    h.press(Key::Home, Modifiers::NONE);
    h.settle();
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    h.press(Key::ArrowLeft, Modifiers::ALT);
    h.settle();
    assert_eq!(h.range(), last);
    h.press(Key::ArrowLeft, Modifiers::ALT);
    h.settle();
    assert_eq!(h.range(), before);
    assert_eq!(h.source(), source());
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert_eq!(h.app.history.len(), undo);
    assert!(!h.app.project.is_dirty());
}

#[test]
fn source_outline_cancel_toggle_query_change_and_empty_filter_are_safe() {
    let mut h = Harness::new(source());
    h.select(7, 3);
    let before = h.range();
    h.open();
    h.frame(vec![
        Event::Text("second.room".into()),
        key_event(Key::Enter, Modifiers::NONE, true),
    ]);
    assert!(h.app.source_outline.open, "同帧更改查询不能执行旧选择");
    h.settle();
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(!h.app.source_outline.open);
    h.press(Key::ArrowLeft, Modifiers::ALT);
    h.settle();
    assert_eq!(h.range(), before);
    h.open();
    h.frame(vec![Event::Text("没有这个结构".into())]);
    assert!(all_text(&h.settle()).contains("没有匹配的声明"));
    h.press(Key::Enter, Modifiers::NONE);
    assert!(h.app.source_outline.open);
    h.press(Key::Escape, Modifiers::NONE);
    assert!(!h.app.source_outline.open);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.source_id()));
    assert_eq!(h.range(), before);
    h.open();
    h.press(Key::O, Modifiers::COMMAND | Modifiers::SHIFT);
    assert!(!h.app.source_outline.open);
    h.open();
    h.click("收起并回到源码");
    assert!(!h.app.source_outline.open);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.source_id()));
}

#[test]
fn source_outline_current_tracks_source_cursor_not_selected_row_or_runtime() {
    let mut h = Harness::new(source());
    let cursor = source()[..source().find("中文🌊正文").unwrap()]
        .chars()
        .count()
        + 1;
    h.select(cursor, cursor);
    h.app.focus_event = Some("second".into());
    h.app.catalog_target = Some(worldline_core::TargetRef::new("entity", "keeper"));
    assert_eq!(h.app.source_outline_current(&h.ctx), Some(4));
    h.open();
    h.press(Key::End, Modifiers::NONE);
    assert_eq!(h.app.source_outline.selected, 6);
    assert_eq!(h.app.source_outline_current(&h.ctx), Some(4));
    h.press(Key::Escape, Modifiers::NONE);
    h.select(4, 4);
    assert_eq!(h.app.source_outline_current(&h.ctx), None);
    h.select(source().chars().count(), source().chars().count());
    assert_eq!(h.app.source_outline_current(&h.ctx), None);
}

#[test]
fn source_outline_task_command_opens_the_same_source_without_changing_bytes() {
    let mut h = Harness::new(source());
    h.select(8, 3);
    let before = h.range();
    h.press(Key::P, Modifiers::COMMAND | Modifiers::SHIFT);
    h.settle();
    assert!(h.app.command_palette.open);
    h.frame(vec![Event::Text("本文件结构".into())]);
    h.settle();
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(!h.app.command_palette.open);
    assert!(h.app.source_outline.open);
    assert_eq!(h.app.tab, Tab::Edit);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(query_id()));
    h.press(Key::Escape, Modifiers::NONE);
    h.settle();
    assert_eq!(h.range(), before);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.source_id()));
    assert_eq!(h.source(), source());
}
