//! 原生 RMB 缺陷回归：真实指针与按键选词；不直接注入已捕获的 EditorSelection。
use super::*;
use crate::app::writing_workspace::Mode;

const NAME: &str = "新人😀";
const SENTENCE: &str = "我和新人😀。";

fn key_event(key: egui::Key, shift: bool, pressed: bool) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: if shift {
            egui::Modifiers::SHIFT
        } else {
            egui::Modifiers::NONE
        },
    }
}

fn press(ctx: &egui::Context, app: &mut WorldeditApp, key: egui::Key, shift: bool) {
    for pressed in [true, false] {
        frame(ctx, app, vec![key_event(key, shift, pressed)]);
    }
}

fn pointer_click(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    pos: egui::Pos2,
    button: egui::PointerButton,
) {
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

fn glyph_point(output: &egui::FullOutput, needle: &str, char_in_needle: usize) -> egui::Pos2 {
    for shape in &output.shapes {
        let mut texts = Vec::new();
        text_shapes(&shape.shape, &mut texts);
        for text in texts {
            let Some(byte) = text.galley.text().find(needle) else {
                continue;
            };
            let offset = text.galley.text()[..byte].chars().count() + char_in_needle;
            let left = text
                .galley
                .pos_from_cursor(egui::text::CCursor::new(offset));
            let right = text
                .galley
                .pos_from_cursor(egui::text::CCursor::new(offset + 1));
            let pos = text.pos + vec2((left.center().x + right.center().x) * 0.5, left.center().y);
            if shape.clip_rect.contains(pos) {
                return pos;
            }
        }
    }
    panic!(
        "找不到当前可见字形 {needle}[{char_in_needle}]：{}",
        labels(output)
    );
}

fn selected_fixture(mode: Mode) -> (egui::Context, WorldeditApp, egui::Pos2) {
    let (ctx, mut app) = fixture();
    app.personal.settings.reduce_motion = true;
    let path = app.project.entry.clone();
    app.manuscript
        .writing_buffers
        .get_mut(&path)
        .unwrap()
        .replace_source(TEXT.replace("我和林芜😀走向灯塔。", SENTENCE));
    frame(&ctx, &mut app, vec![]);
    click(
        &ctx,
        &mut app,
        match mode {
            Mode::Prose => "写作",
            Mode::Structure => "结构",
            Mode::Source => "源码",
        },
    );
    let output = frame(&ctx, &mut app, vec![]);
    let pos = glyph_point(&output, SENTENCE, 0);
    pointer_click(&ctx, &mut app, pos, egui::PointerButton::Primary);
    press(&ctx, &mut app, egui::Key::End, false);
    press(&ctx, &mut app, egui::Key::ArrowLeft, false);
    for _ in 0..3 {
        press(&ctx, &mut app, egui::Key::ArrowLeft, true);
    }
    let output = frame(&ctx, &mut app, vec![]);
    let selection = crate::app::search::editor_selection(&ctx).unwrap();
    assert_eq!(&selection.source[selection.range.clone()], NAME);
    assert_eq!(selection.target, Some(TargetRef::new("event", "start")));
    assert_eq!(app.manuscript.writing_view.session_mode(), mode);
    assert!(
        app.manuscript.world_links.is_none(),
        "RED 必须从无缓存表单开始"
    );
    let pos = glyph_point(&output, NAME, 1);
    (ctx, app, pos)
}

fn open_from_context(ctx: &egui::Context, app: &mut WorldeditApp, pos: egui::Pos2) {
    pointer_click(ctx, app, pos, egui::PointerButton::Secondary);
    assert!(egui::Popup::is_any_open(ctx), "须由真实右键打开文本菜单");
    click(ctx, app, "关联世界资料…");
    frame(ctx, app, vec![]);
}

fn fresh_selection_opens(mode: Mode) {
    let (ctx, mut app, pos) = selected_fixture(mode);
    let baseline = app.project.content_baseline();
    let before = crate::app::search::editor_selection(&ctx).unwrap();
    let generation = app.manuscript.writing_buffers[&app.project.entry].generation();
    open_from_context(&ctx, &mut app, pos);
    let state = app.manuscript.world_links.as_ref().unwrap_or_else(|| {
        panic!(
            "当前选中字形内 RMB 应打开新关联表单，mode={mode:?}, error={:?}",
            app.io_error
        )
    });
    assert!(state.open);
    assert_eq!(state.selection.expected_text, NAME);
    assert_eq!(state.selection.start..state.selection.end, before.range);
    assert_eq!(state.selection.path, before.path);
    assert_eq!(state.source, TargetRef::new("event", "start"));
    assert_eq!(state.generation, generation);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(
        app.manuscript.writing_buffers[&app.project.entry].source(),
        before.source
    );
}

#[test]
fn fresh_context_rmb_prose_keeps_chinese_emoji_selection() {
    fresh_selection_opens(Mode::Prose);
}

#[test]
fn fresh_context_rmb_structure_keeps_chinese_emoji_selection() {
    fresh_selection_opens(Mode::Structure);
}

#[test]
fn fresh_context_rmb_source_keeps_chinese_emoji_selection() {
    fresh_selection_opens(Mode::Source);
}

#[test]
fn context_rmb_outside_selection_never_reuses_the_old_text() {
    for mode in [Mode::Prose, Mode::Structure, Mode::Source] {
        let (ctx, mut app, _) = selected_fixture(mode);
        let baseline = app.project.content_baseline();
        let source = app.manuscript.writing_buffers[&app.project.entry]
            .source()
            .to_owned();
        let output = frame(&ctx, &mut app, vec![]);
        let outside = glyph_point(&output, SENTENCE, 0);
        open_from_context(&ctx, &mut app, outside);
        assert!(app.manuscript.world_links.is_none(), "mode={mode:?}");
        assert!(app
            .io_error
            .as_ref()
            .is_some_and(|error| error.contains("重新选择")));
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(
            app.manuscript.writing_buffers[&app.project.entry].source(),
            source
        );
    }
}

#[test]
fn context_rmb_changed_buffer_generation_rejects_the_old_selection() {
    for mode in [Mode::Prose, Mode::Structure, Mode::Source] {
        let (ctx, mut app, pos) = selected_fixture(mode);
        let baseline = app.project.content_baseline();
        let path = app.project.entry.clone();
        let buffer = app.manuscript.writing_buffers.get_mut(&path).unwrap();
        let source = format!("{}\n// 右键前刚收到的新稿\n", buffer.source());
        buffer.replace_source(source.clone());
        open_from_context(&ctx, &mut app, pos);
        assert!(app.manuscript.world_links.is_none(), "mode={mode:?}");
        assert!(app
            .io_error
            .as_ref()
            .is_some_and(|error| error.contains("重新选择")));
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(app.manuscript.writing_buffers[&path].source(), source);
    }
}

#[test]
fn context_rmb_during_ime_does_not_open_or_bind_old_selection() {
    for mode in [Mode::Prose, Mode::Structure, Mode::Source] {
        let (ctx, mut app, pos) = selected_fixture(mode);
        let baseline = app.project.content_baseline();
        let source = app.manuscript.writing_buffers[&app.project.entry]
            .source()
            .to_owned();
        frame(
            &ctx,
            &mut app,
            vec![
                Event::Ime(egui::ImeEvent::Enabled),
                Event::Ime(egui::ImeEvent::Preedit("组合中".into())),
            ],
        );
        pointer_click(&ctx, &mut app, pos, egui::PointerButton::Secondary);
        assert!(
            !egui::Popup::is_any_open(&ctx),
            "组合时不得触发菜单，mode={mode:?}"
        );
        assert!(app.manuscript.world_links.is_none());
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(
            app.manuscript.writing_buffers[&app.project.entry].source(),
            source
        );
    }
}

#[test]
fn context_rmb_reopens_cached_form_without_recapturing_a_collapsed_selection() {
    let (ctx, mut app, _) = selected_fixture(Mode::Prose);
    click(&ctx, &mut app, "选词工具");
    click(&ctx, &mut app, "关联世界资料…");
    let state = app.manuscript.world_links.as_mut().unwrap();
    state.kind = Kind::Entity;
    state.id = "kept_draft".into();
    state.touched = true;
    let original = (state.selection.start, state.selection.end, state.generation);
    click(&ctx, &mut app, "返回正文，保留输入");
    let output = frame(&ctx, &mut app, vec![]);
    let outside = glyph_point(&output, SENTENCE, 0);
    open_from_context(&ctx, &mut app, outside);
    let state = app.manuscript.world_links.as_ref().unwrap();
    assert!(state.open);
    assert_eq!(state.id, "kept_draft");
    assert_eq!(state.selection.expected_text, NAME);
    assert_eq!(
        (state.selection.start, state.selection.end, state.generation),
        original
    );
}
