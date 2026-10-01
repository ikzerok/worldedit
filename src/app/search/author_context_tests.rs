//! 完整 egui 帧的作者位置验收；不把事件模拟当作物理输入法测试。
use super::*;
use crate::app::{manuscript::ManuscriptSession, writing_workspace::Mode, Tab};

fn manuscript() -> (egui::Context, WorldeditApp, PathBuf) {
    let (ctx, mut app) = app();
    let root = app.project.root.clone();
    let other = app
        .project
        .add_file(std::path::Path::new("other.wl"))
        .unwrap();
    app.project
        .set_text(&other, "event third\n  跨文件 needle。\n  -> END\n".into())
        .unwrap();
    app.project.set_text(&app.active_file.clone(),
        "include \"other.wl\"\nevent start\n  序章 needle 中文。\n  if true\n    分支 needle。\n  -> END\n\nevent second\n  次章 needle。\n  -> END\n".into()).unwrap();
    app.project.create_authoring_document(&root.join(".world/project.json"),
        br#"{"schema_version":1,"language_version":"1.10","required_features":["presentation.manuscripts.v1"],"manuscripts":{"book":".world/book.json"}}"#.to_vec()).unwrap();
    app.project.create_authoring_document(&root.join(".world/book.json"),
        br#"{"schema_version":1,"id":"book","title":"Book","entries":[{"id":"first","kind":"chapter","title":"First","target_ref":{"kind":"event","id":"start"}},{"id":"shared","kind":"chapter","title":"Shared","target_ref":{"kind":"event","id":"start"}},{"id":"second","kind":"chapter","title":"Second","target_ref":{"kind":"event","id":"second"}},{"id":"third","kind":"chapter","title":"Third","target_ref":{"kind":"event","id":"third"}}]}"#.to_vec()).unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    app.tab = Tab::Manuscript;
    frame(&ctx, &mut app, vec![]);
    (ctx, app, other)
}

fn select(ctx: &egui::Context, app: &mut WorldeditApp, chapter: &str, mode: Mode) {
    app.restore_manuscript_session(ManuscriptSession {
        manuscript_id: Some("book".into()),
        selected_id: Some(chapter.into()),
        mode,
        ..Default::default()
    });
    frame(ctx, app, vec![]);
}

fn open(ctx: &egui::Context, app: &mut WorldeditApp, query: &str, source: bool, project: bool) {
    app.open_search(ctx, project, false);
    app.project_query = query.into();
    app.search_state.source = source;
}

#[test]
fn search_keeps_each_representable_mode_and_first_enter_targets_first_hit() {
    for mode in [Mode::Prose, Mode::Structure, Mode::Source] {
        let (ctx, mut app, _) = manuscript();
        select(&ctx, &mut app, "first", mode);
        let baseline = app.project.content_baseline();
        let before = app.manuscript.writing_buffers()[0].clone();
        open(&ctx, &mut app, "needle", false, false);
        let first = app.current_search_hits().unwrap()[0].clone();
        app.navigate_search(&ctx, false);
        frame(&ctx, &mut app, vec![]);
        assert_eq!(app.search_state.located.as_ref(), Some(&first));
        assert_eq!(app.manuscript_session().mode, mode);
        let selection = selection::editor_selection(&ctx).unwrap();
        assert_eq!(selection.range, first.range);
        assert_eq!(&selection.source[selection.range], "needle");
        assert_eq!(
            app.manuscript.writing_buffers()[0].generation(),
            before.generation()
        );
        assert_eq!(app.project.content_baseline(), baseline);
        app.navigate_search(&ctx, false);
        assert_eq!(app.search_state.selected, 1);
        assert_ne!(app.search_state.located.as_ref(), Some(&first));
    }
}

#[test]
fn search_same_source_multiple_chapters_and_cross_file_keep_one_buffer_per_file() {
    let (ctx, mut app, other) = manuscript();
    let path = app.active_file.clone();
    select(&ctx, &mut app, "shared", Mode::Prose);
    let original = app
        .manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .source()
        .to_owned();
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source(original.replace("序章", "未应用序章"));
    let mut other_buffer = app.project.open_source_writing_buffer(&other).unwrap();
    other_buffer.replace_source("event third\n  未应用跨文件 needle。\n  -> END\n".into());
    app.manuscript
        .restore_writing_buffers(&[other_buffer.clone()]);
    let baseline = app.project.content_baseline();
    open(&ctx, &mut app, "未应用序章", false, true);
    app.navigate_search(&ctx, false);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(
        app.manuscript_session().selected_id.as_deref(),
        Some("shared")
    );
    app.project_query = "次章".into();
    app.navigate_search(&ctx, false);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(
        app.manuscript_session().selected_id.as_deref(),
        Some("second")
    );
    app.project_query = "未应用跨文件".into();
    app.navigate_search(&ctx, false);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(
        app.manuscript_session().selected_id.as_deref(),
        Some("third")
    );
    assert_eq!(app.manuscript_session().mode, Mode::Prose);
    app.close_search(&ctx);
    app.author_back(&ctx);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(
        app.manuscript_session().selected_id.as_deref(),
        Some("second")
    );
    assert_eq!(app.manuscript.writing_buffers().len(), 2);
    assert!(app
        .manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .source()
        .contains("未应用序章"));
    assert_eq!(
        app.manuscript.writing_buffer_mut(&other).unwrap().source(),
        other_buffer.source()
    );
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn search_structure_cross_block_and_invalid_draft_fallback_has_reason_and_back() {
    for query in ["if true", "中文。\n  if true\n    分支"] {
        let (ctx, mut app, _) = manuscript();
        let baseline = app.project.content_baseline();
        open(&ctx, &mut app, query, true, false);
        assert_eq!(app.current_search_hits().unwrap().len(), 1);
        app.navigate_search(&ctx, false);
        frame(&ctx, &mut app, vec![]);
        assert_eq!(app.manuscript_session().mode, Mode::Source);
        assert!(app.message.as_deref().unwrap().contains("跨越正文编辑块"));
        app.close_search(&ctx);
        assert_eq!(app.manuscript_session().mode, Mode::Source);
        app.author_back(&ctx);
        frame(&ctx, &mut app, vec![]);
        assert_eq!(app.manuscript_session().mode, Mode::Prose);
        assert_eq!(
            app.manuscript_session().selected_id.as_deref(),
            Some("first")
        );
        assert_eq!(app.project.content_baseline(), baseline);
    }
    let (ctx, mut app, _) = manuscript();
    let path = app.active_file.clone();
    let invalid = "event start\n  未完成 needle {broken\n";
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source(invalid.into());
    open(&ctx, &mut app, "needle", true, false);
    app.navigate_search(&ctx, false);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.manuscript_session().mode, Mode::Source);
    assert!(app
        .message
        .as_deref()
        .unwrap()
        .contains("无法形成准确章节投影"));
    app.close_search(&ctx);
    app.author_back(&ctx);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.manuscript_session().mode, Mode::Prose);
    assert_eq!(
        app.manuscript.writing_buffer_mut(&path).unwrap().source(),
        invalid
    );
}

#[test]
fn search_second_query_empty_results_and_ime_never_replay_stale_navigation() {
    let (ctx, mut app, _) = manuscript();
    open(&ctx, &mut app, "needle", false, false);
    app.navigate_search(&ctx, false);
    app.navigate_search(&ctx, false);
    app.project_query = "中文".into();
    let first = app.current_search_hits().unwrap()[0].clone();
    app.navigate_search(&ctx, false);
    assert_eq!(app.search_state.located.as_ref(), Some(&first));
    let depth = app.personal.history.len();
    app.project_query = "不存在的结果".into();
    app.navigate_search(&ctx, false);
    assert!(app.search_state.located.is_none());
    assert_eq!(app.personal.history.len(), depth);
    app.go_search_hit(&ctx, &first);
    assert!(app.search_state.error.as_deref().unwrap().contains("过期"));
    assert_eq!(app.personal.history.len(), depth);
    for event in [
        egui::ImeEvent::Enabled,
        egui::ImeEvent::Preedit("中".into()),
        egui::ImeEvent::Commit("中".into()),
    ] {
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::Ime(event),
                key(egui::Key::Enter, egui::Modifiers::NONE),
                key(egui::Key::Escape, egui::Modifiers::NONE),
            ],
        );
        assert!(app.search_open);
        assert_eq!(app.personal.history.len(), depth);
    }
    frame(
        &ctx,
        &mut app,
        vec![egui::Event::Ime(egui::ImeEvent::Disabled)],
    );
    frame(
        &ctx,
        &mut app,
        vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
    );
    assert!(!app.search_open);
    assert_eq!(app.manuscript_session().mode, Mode::Prose);
    assert_eq!(app.personal.history.len(), depth);
}

#[path = "author_context_restore_tests.rs"]
mod restore;

#[test]
fn clean_stale_cache_uses_project_while_changed_cache_keeps_current_draft() {
    for draft in [false, true] {
        let (ctx, mut app, _) = manuscript();
        let path = app.active_file.clone();
        let old = app.project.document(&path).unwrap().to_owned();
        if draft {
            app.manuscript
                .writing_buffer_mut(&path)
                .unwrap()
                .replace_source(old.replace("序章", "未应用序章"));
        }
        let latest = old.replace("序章", "工程新版序章");
        app.project.set_text(&path, latest.clone()).unwrap();
        // 不先画帧或recompile：导航入口必须自己选择与core命中一致的源。
        let baseline = app.project.content_baseline();
        open(&ctx, &mut app, "序章", true, false);
        app.navigate_search(&ctx, false);
        frame(&ctx, &mut app, vec![]);
        let expected = if draft {
            old.replace("序章", "未应用序章")
        } else {
            latest
        };
        let selection = selection::editor_selection(&ctx).unwrap();
        assert_eq!(selection.source, expected);
        assert_eq!(&selection.source[selection.range], "序章");
        assert_eq!(
            app.manuscript.writing_buffer_mut(&path).unwrap().source(),
            expected
        );
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(app.manuscript.writing_buffers().len(), 1);
    }
}
