//! 作者来源位置同时覆盖真实UTF-8展示文档；不使用lossy预览建立坐标。
use super::*;
use std::path::Path;

fn app(bytes: &[u8]) -> (egui::Context, WorldeditApp, PathBuf) {
    let ctx = egui::Context::default();
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = std::env::temp_dir().join(format!("source-position-{}", std::process::id()));
    let files = BTreeMap::from([
        (PathBuf::from("main.wl"), b"event start\n  hello\n".to_vec()),
        (PathBuf::from(".world/project.json"), br#"{"schema_version":1,"language_version":"1.13","required_features":[],"maps":{"source":".world/maps/source.json"}}"#.to_vec()),
        (PathBuf::from(".world/maps/source.json"), bytes.to_vec()),
    ]);
    app.project =
        worldline_core::project::Project::from_snapshot(&root, Path::new("main.wl"), &files)
            .unwrap();
    let path = app.project.root.join(".world/maps/source.json");
    app.active_file = path.clone();
    app.tab = Tab::Edit;
    (ctx, app, path)
}

fn select(ctx: &egui::Context, id: egui::Id, primary: usize, secondary: usize) {
    let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    state.cursor.set_char_range(Some(egui::text::CCursorRange {
        primary: egui::text::CCursor::new(primary),
        secondary: egui::text::CCursor::new(secondary),
        h_pos: None,
    }));
    state.store(ctx, id);
}

#[test]
fn authoring_json_back_captures_real_editor_id_baseline_and_direction() {
    let text = "{\r\n  \"title\":\"中文🧭\",\r\n  \"body\":\"当前未保存 JSON\"\r\n}";
    let (ctx, mut app, path) = app(br#"{"schema_version":1}"#);
    app.project
        .set_authoring_document(&path, text.as_bytes().to_vec())
        .unwrap();
    let dirty = app.project.is_dirty();
    assert!(dirty);
    let id = egui::Id::new(("authoring-source", &path));
    select(&ctx, id, 19, 12);
    select(&ctx, egui::Id::new(("source", &path)), 1, 2);
    app.personal.source_scroll = [44.0, 128.0];
    let location = app.author_location(Some(&ctx));
    assert_eq!(
        (location.cursor, location.source_secondary),
        (Some(19), Some(12))
    );
    assert_eq!(
        location.source_baseline,
        Some(crate::app::writing_workspace::fingerprint(text))
    );
    app.remember_author_location(location);
    app.active_file = app.project.entry.clone();
    app.personal.source_scroll = [0.0, 0.0];
    select(&ctx, id, 0, 0);
    app.author_back(&ctx);
    assert_eq!(app.active_file, path);
    assert_eq!(app.personal.source_scroll, [44.0, 128.0]);
    assert!(app.personal.restore_source);
    let range = egui::TextEdit::load_state(&ctx, id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!((range.primary.index, range.secondary.index), (19, 12));
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    assert_eq!(
        app.project.authoring_document(&path).unwrap().bytes(),
        text.as_bytes()
    );
    assert_eq!(app.project.is_dirty(), dirty);
}

#[test]
fn changed_authoring_json_rejects_old_offsets_in_correct_editor_without_writing() {
    let (ctx, mut app, path) = app(br#"{"schema_version":1,"title":"before"}"#);
    let id = egui::Id::new(("authoring-source", &path));
    select(&ctx, id, 23, 29);
    app.remember_author_location(app.author_location(Some(&ctx)));
    let changed = "{\"schema_version\":1,\"title\":\"后来改过🧭\"}"
        .as_bytes()
        .to_vec();
    app.project
        .set_authoring_document(&path, changed.clone())
        .unwrap();
    app.active_file = app.project.entry.clone();
    app.author_back(&ctx);
    let range = egui::TextEdit::load_state(&ctx, id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!((range.primary.index, range.secondary.index), (0, 0));
    assert_eq!(app.personal.source_scroll, [0.0, 0.0]);
    assert!(app
        .message
        .as_deref()
        .unwrap()
        .contains("未恢复旧选区和滚动"));
    assert_eq!(
        app.project.authoring_document(&path).unwrap().bytes(),
        changed
    );
}

#[test]
fn unknown_authoring_json_keeps_readonly_bytes_and_can_capture_real_utf8_location() {
    let bytes = br#"{"schema_version":999,"unknown":"preserve exactly"}"#;
    let (ctx, mut app, path) = app(bytes);
    assert!(app
        .project
        .authoring_document(&path)
        .unwrap()
        .is_read_only());
    let id = egui::Id::new(("authoring-source", &path));
    select(&ctx, id, 7, 11);
    let location = app.author_location(Some(&ctx));
    assert_eq!(
        (location.cursor, location.source_secondary),
        (Some(7), Some(11))
    );
    assert!(location.source_baseline.is_some());
    app.remember_author_location(location);
    app.active_file = app.project.entry.clone();
    app.author_back(&ctx);
    assert!(app
        .project
        .authoring_document(&path)
        .unwrap()
        .is_read_only());
    assert_eq!(
        app.project.authoring_document(&path).unwrap().bytes(),
        bytes
    );
    assert!(!app.project.is_dirty());
}

#[test]
fn invalid_utf8_preview_never_captures_lossy_text_coordinates_or_baseline() {
    let bytes = b"{\"title\":\"\xff\xfe\"}";
    let (ctx, mut app, path) = app(bytes);
    select(&ctx, egui::Id::new(("authoring-source", &path)), 5, 9);
    select(&ctx, egui::Id::new(("source", &path)), 1, 2);
    app.personal.source_cursor = Some((path.clone(), 20));
    let location = app.author_location(Some(&ctx));
    assert_eq!(location.cursor, None);
    assert_eq!(location.source_secondary, None);
    assert_eq!(location.source_baseline, None);
    assert_eq!(location.source_view, None);
    app.remember_author_location(location);
    app.active_file = app.project.entry.clone();
    app.author_back(&ctx);
    assert_eq!(
        app.project.authoring_document(&path).unwrap().bytes(),
        bytes
    );
    assert!(!app.project.is_dirty());
    assert!(app
        .message
        .as_deref()
        .unwrap()
        .contains("没有可定位的有效UTF-8原文"));
}
