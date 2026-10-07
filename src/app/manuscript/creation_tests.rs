//! 真正的 egui 按钮、键盘和 IME 输入；不替代原生桌面验收。
use super::*;
mod native_ime;
mod play_scope;
mod retention;
mod scroll_geometry;
mod structure;
mod styles;
use crate::app::{Tab, WorldeditApp};
use egui::{pos2, vec2, Event, RawInput, Rect};
use std::sync::atomic::{AtomicUsize, Ordering};
use worldline_core::project::Project;

fn blank() -> (egui::Context, WorldeditApp) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = std::env::temp_dir().join(format!(
        "worldedit-create-chapter-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    app.project = Project::new(&root);
    app.active_file = app.project.entry.clone();
    app.reset_views();
    app.recompile();
    app.tab = Tab::Manuscript;
    (ctx, app)
}

fn frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 1500.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            let _theme = crate::theme::configure_appearance(ctx, app.personal.appearance());
            app.manuscript_tab(ctx);
        },
    )
}

fn text_shapes<'a>(shape: &'a egui::Shape, out: &mut Vec<&'a egui::epaint::TextShape>) {
    match shape {
        egui::Shape::Text(text) => out.push(text),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                text_shapes(shape, out);
            }
        }
        _ => {}
    }
}

fn labels(output: &egui::FullOutput) -> String {
    output
        .shapes
        .iter()
        .flat_map(|shape| {
            let mut texts = Vec::new();
            text_shapes(&shape.shape, &mut texts);
            texts
        })
        .map(|text| text.galley.text())
        .collect::<Vec<_>>()
        .join("\n")
}

fn settle(ctx: &egui::Context, app: &mut WorldeditApp) -> egui::FullOutput {
    for _ in 0..3 {
        frame(ctx, app, vec![]);
    }
    frame(ctx, app, vec![])
}

fn click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let output = settle(ctx, app);
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| {
            let mut texts = Vec::new();
            text_shapes(&shape.shape, &mut texts);
            texts
                .into_iter()
                .filter(|text| text.galley.text() == label)
                .map(|text| text.pos + text.galley.rect.center().to_vec2())
                .find(|pos| shape.clip_rect.contains(*pos))
        })
        .unwrap_or_else(|| panic!("missing {label}: {}", labels(&output)));
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

fn key(ctx: &egui::Context, app: &mut WorldeditApp, key: egui::Key) {
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
    }
}

fn titles(ctx: &egui::Context, app: &mut WorldeditApp) {
    click(ctx, app, "开始写作");
    settle(ctx, app);
    frame(ctx, app, vec![Event::Text("海边的灯".into())]);
    key(ctx, app, egui::Key::Tab);
    frame(ctx, app, vec![Event::Text("潮汐初起".into())]);
    let form = app.manuscript.creation.as_ref().unwrap();
    assert_eq!(form.book_title, "海边的灯");
    assert_eq!(form.chapter_title, "潮汐初起");
}

fn create_start(ctx: &egui::Context, app: &mut WorldeditApp) {
    titles(ctx, app);
    click(ctx, app, "明确使用现有起点 start");
    click(ctx, app, "预览创建计划");
    assert!(
        app.manuscript.creation.as_ref().unwrap().error.is_none(),
        "{:?}",
        app.manuscript.creation.as_ref().unwrap().error
    );
    let preview = settle(ctx, app);
    assert!(labels(&preview).contains("正式来源：event:start"));
    click(ctx, app, "创建并写作");
    settle(ctx, app);
    assert!(app.manuscript.creation.is_none());
}

#[test]
fn blank_start_requires_explicit_source_and_cancel_preserves_keyboard_input() {
    let (ctx, mut app) = blank();
    let baseline = app.project.content_baseline();
    titles(&ctx, &mut app);
    click(&ctx, &mut app, "预览创建计划");
    assert!(app
        .manuscript
        .creation
        .as_ref()
        .unwrap()
        .error
        .as_ref()
        .unwrap()
        .contains("明确选择正文来源"));
    assert_eq!(app.project.content_baseline(), baseline);
    key(&ctx, &mut app, egui::Key::Escape);
    assert!(app.manuscript.creation_dismissed);
    assert!(app.manuscript.has_unsubmitted_work());
    click(&ctx, &mut app, "继续创建首章");
    let form = app.manuscript.creation.as_ref().unwrap();
    assert_eq!(form.book_title, "海边的灯");
    assert_eq!(form.chapter_title, "潮汐初起");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn create_write_review_apply_save_reopen_is_one_source_and_one_creation_undo() {
    let (ctx, mut app) = blank();
    let original = app.project.document(&app.project.entry).unwrap().to_owned();
    let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
    create_start(&ctx, &mut app);
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.project.language_version(), "1.9");
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
    assert_eq!(app.project.document(&app.project.entry).unwrap(), original);
    frame(&ctx, &mut app, vec![Event::Text("海雾里，灯亮了。".into())]);
    let path = app.project.entry.clone();
    let body = app.manuscript.writing_buffers.get(&path).unwrap();
    assert!(body.is_changed());
    assert!(body.source().contains("海雾里，灯亮了。"));
    let output = settle(&ctx, &mut app);
    assert!(labels(&output).contains("当前稿 · 包含未应用输入"));
    assert_eq!(app.project.document(&path).unwrap(), original);
    click(&ctx, &mut app, "应用正文草稿");
    assert_eq!(app.history.len(), 2);
    assert!(app
        .project
        .document(&path)
        .unwrap()
        .contains("海雾里，灯亮了。"));
    app.project.save().unwrap();
    let reopened = Project::open(&app.project.root).unwrap();
    assert_eq!(reopened.language_version(), "1.9");
    assert_eq!(
        reopened.manuscript_index("book").unwrap().entries[0].target_ref,
        Some(TargetRef::new("event", "start"))
    );
    assert_eq!(
        reopened.document(&reopened.entry).unwrap(),
        app.project.document(&path).unwrap()
    );
    app.undo(false);
    assert_eq!(app.project.document(&path).unwrap(), original);
    app.undo(false);
    assert!(app.project.manuscript_indices().is_empty());
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn new_event_plan_discloses_fingerprint_and_repeated_apply_is_zero_change() {
    let (ctx, mut app) = blank();
    titles(&ctx, &mut app);
    click(&ctx, &mut app, "新建空白正文");
    click(&ctx, &mut app, "预览创建计划");
    let output = settle(&ctx, &mut app);
    assert!(labels(&output).contains("旧运行保存或检查点不能直接用于新指纹"));
    let mut form = app.manuscript.creation.take().unwrap();
    assert!(app.apply_chapter_creation(&mut form));
    let baseline = app.project.content_baseline();
    assert!(!app.apply_chapter_creation(&mut form));
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.history.len(), 1);
    assert_eq!(
        app.project.manuscript_index("book").unwrap().entries.len(),
        1
    );
    assert!(app
        .project
        .document(&app.project.entry)
        .unwrap()
        .contains("event chapter"));
    assert!(!app
        .project
        .document(&app.project.entry)
        .unwrap()
        .contains("故事从"));
    assert_eq!(app.snapshot.as_ref().unwrap().result.program.entry, "start");
}

#[test]
fn changed_project_rejects_old_plan_without_discarding_titles() {
    let (ctx, mut app) = blank();
    titles(&ctx, &mut app);
    click(&ctx, &mut app, "明确使用现有起点 start");
    click(&ctx, &mut app, "预览创建计划");
    let mut form = app.manuscript.creation.take().unwrap();
    let path = app.project.entry.clone();
    app.project
        .set_text(&path, "event start\n  外部新稿。\n  -> END\n".into())
        .unwrap();
    let baseline = app.project.content_baseline();
    assert!(!app.apply_chapter_creation(&mut form));
    assert_eq!(form.book_title, "海边的灯");
    assert_eq!(form.chapter_title, "潮汐初起");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn first_empty_prose_ime_keeps_widget_identity_and_committed_chinese() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    let focused = ctx.memory(|memory| memory.focused()).unwrap();
    frame(&ctx, &mut app, vec![Event::Ime(egui::ImeEvent::Enabled)]);
    frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Preedit("灯".into()))],
    );
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(focused));
    frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Commit("灯火".into()))],
    );
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(focused));
    let buffer = app
        .manuscript
        .writing_buffers
        .get(&app.project.entry)
        .unwrap();
    assert!(buffer.source().contains("灯火"));
    assert!(!buffer.source().contains("灯灯火"));
    assert_eq!(app.history.len(), 1);
}

#[test]
fn creation_ime_blocks_escape_and_preview_without_losing_form_text() {
    let (ctx, mut app) = blank();
    titles(&ctx, &mut app);
    app.ime_composing = true;
    key(&ctx, &mut app, egui::Key::Escape);
    assert!(!app.manuscript.creation_dismissed);
    click(&ctx, &mut app, "预览创建计划");
    assert!(app.manuscript.creation.as_ref().unwrap().preview.is_none());
    assert_eq!(
        app.manuscript.creation.as_ref().unwrap().chapter_title,
        "潮汐初起"
    );
    app.ime_composing = false;
    click(&ctx, &mut app, "返回，保留输入");
    assert!(app.manuscript.creation_dismissed);
}

#[test]
fn unfinished_body_blocks_new_chapter_and_cancel_returns_the_same_draft() {
    let (ctx, mut app) = blank();
    create_start(&ctx, &mut app);
    frame(&ctx, &mut app, vec![Event::Text("未应用的海雾".into())]);
    click(&ctx, &mut app, "新建章节");
    frame(&ctx, &mut app, vec![Event::Text("远灯".into())]);
    click(&ctx, &mut app, "新建空白正文");
    click(&ctx, &mut app, "预览创建计划");
    assert!(app.manuscript.creation.as_ref().unwrap().preview.is_none());
    assert!(labels(&settle(&ctx, &mut app)).contains("仍有未应用的书稿编排或正文"));
    click(&ctx, &mut app, "返回，保留输入");
    assert!(app
        .manuscript
        .writing_buffers
        .get(&app.project.entry)
        .unwrap()
        .source()
        .contains("未应用的海雾"));
    assert_eq!(
        app.project.manuscript_index("book").unwrap().entries.len(),
        1
    );
    assert_eq!(app.history.len(), 1);
}

#[test]
fn external_disk_edit_rejects_apply_and_keeps_the_preview_form() {
    let (ctx, mut app) = blank();
    app.project.save().unwrap();
    titles(&ctx, &mut app);
    click(&ctx, &mut app, "明确使用现有起点 start");
    click(&ctx, &mut app, "预览创建计划");
    std::fs::write(
        &app.project.entry,
        "event start\n  磁盘上的新稿。\n  -> END\n",
    )
    .unwrap();
    click(&ctx, &mut app, "创建并写作");
    let form = app.manuscript.creation.as_ref().unwrap();
    assert!(form.error.is_some());
    assert_eq!(form.book_title, "海边的灯");
    assert!(app.project.manuscript_indices().is_empty());
    assert!(app.history.is_empty());
    assert!(std::fs::read_to_string(&app.project.entry)
        .unwrap()
        .contains("磁盘上的新稿。"));
    std::fs::remove_dir_all(&app.project.root).unwrap();
}
