//! 实际 egui 帧测试；不冒充原生窗口、真实浏览器或物理 IME 验收。
use super::*;
use egui::{
    text::{CCursor, CCursorRange},
    Context, Event, Key, Modifiers, Rect, Vec2,
};
use std::sync::atomic::{AtomicUsize, Ordering};
mod focus;
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
            "source-jump-ui-{}-{}",
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
        self.press(Key::G, Modifiers::CTRL);
        self.settle();
        assert!(self.app.source_jump.open);
    }
    fn query(&mut self, request: &str) {
        self.app.source_jump.query = request.into();
        self.settle();
    }
    fn jump(&mut self, request: &str) {
        self.open();
        self.query(request);
        self.press(Key::Enter, Modifiers::NONE);
        self.settle();
        assert!(
            !self.app.source_jump.open,
            "{:?}",
            self.app.source_jump.notice
        );
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
fn source_jump_unicode_crlf_exact_zero_width_repeated_and_back_are_read_only() {
    let mut h = Harness::new(source());
    h.select(8, 2);
    let original = h.range();
    let baseline = h.app.project.content_baseline();
    let history = h.app.history.len();
    h.jump("7:8");
    let index = SourceCoordinates::new(source())
        .unwrap()
        .locate(source(), "7:8")
        .unwrap()
        .character_offset;
    assert_eq!(h.range(), CCursorRange::one(CCursor::new(index)));
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.source_id()));
    let previous = h.range();
    h.jump("11");
    h.press(Key::ArrowLeft, Modifiers::ALT);
    h.settle();
    assert_eq!(h.range(), previous);
    h.press(Key::ArrowLeft, Modifiers::ALT);
    h.settle();
    assert_eq!(h.range(), original);
    assert!(h.app.personal.history.is_empty());
    assert_eq!(h.source(), source());
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert_eq!(h.app.history.len(), history);
    assert!(!h.app.project.is_dirty());
}

#[test]
fn source_jump_live_position_uses_active_selection_endpoint_and_rejects_crlf_middle() {
    let mut h = Harness::new(source());
    let index = SourceCoordinates::new(source()).unwrap();
    let target = index.locate(source(), "7:8").unwrap();
    h.select(target.character_offset, 1);
    assert_eq!(h.app.source_jump_position(&h.ctx).unwrap(), target);
    assert!(all_text(&h.settle()).contains("第 7 行 · 第 8 列"));
    h.select(1, target.character_offset);
    assert_eq!(h.app.source_jump_position(&h.ctx).unwrap().column, 2);
    let cr = source()[..source().find('\r').unwrap()].chars().count();
    h.select(cr + 1, cr + 1);
    assert!(h.app.source_jump_position(&h.ctx).is_err());
    assert!(all_text(&h.settle()).contains("行列暂不可用"));
    assert_eq!(h.source(), source());
}

#[test]
fn source_jump_invalid_input_and_same_frame_enter_never_leave_source() {
    let mut h = Harness::new(source());
    h.select(7, 3);
    let before = h.range();
    h.open();
    for input in [
        "", "0", "1:0", "1:", "-1", "1:2:3", "1 :2", "9999", "1:9999", "一",
    ] {
        h.query(input);
        assert!(h.app.source_jump.review.as_ref().unwrap().preview.is_err());
        h.press(Key::Enter, Modifiers::NONE);
        assert!(h.app.source_jump.open);
        assert!(h.app.personal.history.is_empty());
        assert_eq!(h.range(), before);
    }
    h.app.source_jump.query.clear();
    h.settle();
    h.frame(vec![
        Event::Text("7:8".into()),
        key_event(Key::Enter, Modifiers::NONE, true),
    ]);
    assert!(h.app.source_jump.open, "新输入只能更新预览，不能吃旧Enter");
    assert!(h.app.personal.history.is_empty());
    h.settle();
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(!h.app.source_jump.open);
    assert_eq!(h.source(), source());
}

#[test]
fn source_jump_empty_source_and_trailing_empty_line_and_bad_syntax_remain_navigable() {
    for source in [
        "",
        "\n",
        "event \"坏语法\n  中文🌊\n",
        "中\t文e\u{301}🌊\r\n\n",
    ] {
        let mut h = Harness::new(source);
        let index = SourceCoordinates::new(source).unwrap();
        let request = format!("{}:1", index.line_count());
        h.jump(&request);
        assert_eq!(h.range().primary.index, source.chars().count());
        assert_eq!(h.range().primary, h.range().secondary);
        assert_eq!(h.source(), source);
    }
}

#[test]
fn source_jump_cancel_close_and_toggle_do_not_change_content_selection_or_history() {
    let mut h = Harness::new(source());
    h.select(8, 2);
    let before = h.range();
    for action in ["escape", "button", "toggle"] {
        h.open();
        h.query("7:8");
        match action {
            "escape" => h.press(Key::Escape, Modifiers::NONE),
            "button" => h.click("取消"),
            _ => h.press(Key::G, Modifiers::CTRL),
        }
        h.settle();
        assert!(!h.app.source_jump.open);
        assert_eq!(h.range(), before);
        assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.source_id()));
        assert!(h.app.personal.history.is_empty());
        assert_eq!(h.source(), source());
    }
}
