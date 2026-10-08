//! 有界导航、完整依赖缓存与个人往返回归；由统一桌面 owner 执行。
use super::*;
use worldline_core::manuscript::{ManuscriptQueryDraft, ManuscriptQueryRequest};

fn thousand_chapters() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app_with_source(SOURCE);
    let entries: Vec<_> = (0..1000)
        .map(|n| {
            serde_json::json!({
                "id": format!("chapter{n:04}"), "kind":"chapter", "title":"同名章节",
                "summary":format!("摘要{n:04}"), "status": if n % 2 == 0 {"draft"} else {"done"},
                "pov":{"kind":"character","id":"lin"}, "goal":format!("目标{n:04}"),
                "target_ref":{"kind":"event","id":if n % 2 == 0 {"start"} else {"second"}}
            })
        })
        .collect();
    app.project.set_authoring_document(&app.project.root.join(".world/manuscripts/book.json"),
        serde_json::to_vec(&serde_json::json!({"schema_version":1,"id":"book","title":"千章书稿","entries":entries})).unwrap()).unwrap();
    app.manuscript = WorkbenchState::default();
    app.recompile();
    app.manuscript.reader_open = false;
    (ctx, app)
}

#[test]
fn thousand_chapter_filters_run_before_paging_and_idle_reuses_snapshots() {
    let (ctx, mut app) = thousand_chapters();
    let size = vec2(1280.0, 800.0);
    let initial = texts(&settle(&ctx, &mut app, size));
    assert!(initial.contains("匹配 1000 / 可识别 1000 章"), "{initial}");
    let builds = app.manuscript.query_cache.builds();
    for _ in 0..12 {
        frame(&ctx, &mut app, size, vec![]);
    }
    assert_eq!(app.manuscript.query_cache.builds(), builds);
    app.manuscript.navigation.session.text = "目标0999".into();
    let filtered = texts(&settle(&ctx, &mut app, size));
    assert!(filtered.contains("匹配 1 / 可识别 1000 章"), "{filtered}");
    assert_eq!(
        app.manuscript.query_cache.builds(),
        builds,
        "查询变化不重编全书"
    );
    assert_eq!(
        app.manuscript.books["book"].selected_entry.as_deref(),
        Some("chapter0000")
    );
    assert!(filtered.contains("当前章节不在筛选结果内"), "{filtered}");
    app.manuscript.navigation.session.status = "draft".into();
    assert!(texts(&settle(&ctx, &mut app, size)).contains("匹配 0 / 可识别 1000 章"));
}

#[test]
fn manuscript_query_cache_invalidates_draft_body_cancel_and_unknown_documents() {
    let (_, app) = app_with_source(SOURCE);
    let mut project = app.project.clone();
    let mut cache = query_cache::QueryCache::default();
    let first = cache.applied(&project).unwrap();
    let again = cache.applied(&project).unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &again));
    let mut draft = ManuscriptDraft::from_index(&first.indices()["book"]);
    draft.entries[0].summary = Some("未应用摘要".into());
    let input = ManuscriptQueryDraft {
        expected_baseline: project.content_baseline(),
        draft,
    };
    let changed = cache
        .current(&project, std::iter::empty(), std::slice::from_ref(&input))
        .unwrap();
    assert_ne!(first.key(), changed.key());
    assert!(std::sync::Arc::ptr_eq(
        &first,
        &cache.current(&project, std::iter::empty(), &[]).unwrap()
    ));
    let mut body = project
        .open_writing_buffer(&TargetRef::new("event", "start"))
        .unwrap();
    body.replace_source(format!("{}\n非法结构 {{", body.source()));
    let invalid = cache.current(&project, [&body], &[]).unwrap();
    let page = invalid
        .query(&ManuscriptQueryRequest {
            manuscript_id: "book".into(),
            ..Default::default()
        })
        .unwrap();
    assert!(!page.complete);
    assert!(page.rows.iter().all(|row| row
        .entry
        .source
        .as_ref()
        .is_none_or(|source| source.stats.is_none())));
    let builds = cache.builds();
    cache.current(&project, [&body], &[]).unwrap();
    assert_eq!(cache.builds(), builds, "同一失败输入不反复编译");
    let book_path = project.root.join(".world/manuscripts/book.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(project.authoring_document(&book_path).unwrap().bytes()).unwrap();
    value["future"] = serde_json::json!({"new_optional_field":true});
    project
        .set_authoring_document(&book_path, serde_json::to_vec(&value).unwrap())
        .unwrap();
    assert_ne!(first.key(), cache.applied(&project).unwrap().key());
    assert!(
        cache.current(&project, [&body], &[]).is_err(),
        "外部内容变化后草稿基线必须重验"
    );
}

#[test]
fn navigation_session_keeps_filters_columns_page_and_real_chapter_identity() {
    let (ctx, mut app) = thousand_chapters();
    let size = vec2(1280.0, 800.0);
    settle(&ctx, &mut app, size);
    app.manuscript.books.get_mut("book").unwrap().selected_entry = Some("chapter0999".into());
    app.manuscript.navigation.session.text = "摘要".into();
    app.manuscript.navigation.session.status = "done".into();
    app.manuscript.navigation.session.pov = "lin".into();
    app.manuscript.navigation.session.offset = 450;
    app.manuscript.navigation.session.columns.source = true;
    app.manuscript.layout = Layout::List;
    settle(&ctx, &mut app, size);
    app.manuscript.navigation.session.offset = 450;
    settle(&ctx, &mut app, size);
    let session = app.manuscript_session();
    let json = serde_json::to_string(&session).unwrap();
    let decoded: ManuscriptSession = serde_json::from_str(&json).unwrap();
    app.manuscript = WorkbenchState::default();
    app.restore_manuscript_session(decoded);
    settle(&ctx, &mut app, size);
    assert_eq!(
        app.manuscript.books["book"].selected_entry.as_deref(),
        Some("chapter0999")
    );
    assert_eq!(app.manuscript.navigation.session.text, "摘要");
    assert_eq!(app.manuscript.navigation.session.status, "done");
    assert_eq!(app.manuscript.navigation.session.pov, "lin");
    assert!(app.manuscript.navigation.session.columns.source);
    assert_eq!(app.manuscript.layout, Layout::List);
    assert_eq!(app.manuscript.navigation.session.offset, 450);
}

#[test]
fn keyboard_navigation_selects_real_ids_and_enter_returns_to_body() {
    let (ctx, mut app) = app_with_source(SOURCE);
    let size = vec2(1280.0, 800.0);
    app.manuscript.reader_open = false;
    settle(&ctx, &mut app, size);
    app.manuscript.navigation.request_row_focus = true;
    settle(&ctx, &mut app, size);
    key(
        &ctx,
        &mut app,
        size,
        egui::Key::ArrowDown,
        egui::Modifiers::NONE,
    );
    assert_eq!(
        app.manuscript.books["book"].selected_entry.as_deref(),
        Some("second")
    );
    key(
        &ctx,
        &mut app,
        size,
        egui::Key::Enter,
        egui::Modifiers::NONE,
    );
    assert!(!app.manuscript.narrow_preview);
    assert_eq!(
        app.manuscript.active_writing_target().unwrap().0,
        TargetRef::new("event", "second")
    );
}

#[test]
fn outline_respects_core_incomplete_and_zero_match_without_clearing_input() {
    let (ctx, mut app) = app_with_source(SOURCE);
    let size = vec2(1280.0, 800.0);
    settle(&ctx, &mut app, size);
    let path = app.active_file.clone();
    let before = app.project.document(&path).unwrap().to_owned();
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source(format!("{before}\nlet ="));
    app.manuscript.navigation.session.text = "不存在的章节".into();
    let text = texts(&settle(&ctx, &mut app, size));
    assert!(text.contains("范围未完整确认"), "{text}");
    assert!(text.contains("匹配 0 / 可识别 2 章"), "{text}");
    assert!(app
        .manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .source()
        .ends_with("let ="));
    assert_eq!(app.project.document(&path).unwrap(), before);
}

#[test]
fn draft_shrinking_last_page_clamps_range_without_switching_identity() {
    let (ctx, mut app) = thousand_chapters();
    let size = vec2(1280.0, 800.0);
    settle(&ctx, &mut app, size);
    app.manuscript.layout = Layout::List;
    settle(&ctx, &mut app, size);
    app.manuscript.navigation.session.offset = 950;
    {
        let local = app.manuscript.books.get_mut("book").unwrap();
        local.selected_entry = Some("chapter0999".into());
        local.draft.entries.truncate(70);
        local.changed = true;
    }
    let text = texts(&settle(&ctx, &mut app, size));
    assert!(text.contains("匹配 70 / 可识别 70 章"), "{text}");
    assert_eq!(app.manuscript.navigation.session.offset, 50);
    assert_eq!(
        app.manuscript.books["book"].selected_entry.as_deref(),
        Some("chapter0999")
    );
    assert!(text.contains("原选择已失效"), "{text}");
    assert_eq!(
        app.project.manuscript_index("book").unwrap().entries.len(),
        1000
    );
}

#[test]
fn deleted_or_retargeted_restore_never_opens_a_substitute_and_retains_body_draft() {
    let (ctx, mut app) = app_with_source(SOURCE);
    let size = vec2(1280.0, 800.0);
    settle(&ctx, &mut app, size);
    let path = app.active_file.clone();
    let original = app.project.document(&path).unwrap().to_owned();
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source(original.replace("灯塔亮起", "保留未应用文字"));
    let saved = app.manuscript_session();
    let manuscript_path = app.project.root.join(".world/manuscripts/book.json");
    let mut value: serde_json::Value = serde_json::from_slice(
        app.project
            .authoring_document(&manuscript_path)
            .unwrap()
            .bytes(),
    )
    .unwrap();
    value["entries"][0]["target_ref"]["id"] = serde_json::json!("second");
    app.project
        .set_authoring_document(&manuscript_path, serde_json::to_vec(&value).unwrap())
        .unwrap();
    app.restore_manuscript_session(saved);
    let text = texts(&settle(&ctx, &mut app, size));
    assert!(app.manuscript.books["book"].selected_entry.is_none());
    assert!(text.contains("保留的正文草稿"), "{text}");
    assert!(app
        .manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .source()
        .contains("保留未应用文字"));
    assert!(!app
        .project
        .document(&path)
        .unwrap()
        .contains("保留未应用文字"));
}

#[test]
fn malformed_duplicate_identity_never_opens_the_recognized_row_as_substitute() {
    let (ctx, mut app) = app_with_source(SOURCE);
    let size = vec2(1280.0, 800.0);
    settle(&ctx, &mut app, size);
    let path = app.project.root.join(".world/manuscripts/book.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&path).unwrap().bytes()).unwrap();
    value["entries"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"first","kind":"future_kind","title":"坏项同ID"}));
    app.project
        .set_authoring_document(&path, serde_json::to_vec(&value).unwrap())
        .unwrap();
    let text = texts(&settle(&ctx, &mut app, size));
    assert!(app.manuscript.books["book"].selected_entry.is_none());
    assert!(app.manuscript.active_writing_target().is_none());
    assert!(text.contains("身份重复"), "{text}");
    assert!(text.contains("范围未完整确认"), "{text}");
}

#[test]
fn metadata_refresh_and_host_invalidation_rebuild_pages_without_idle_io() {
    let (_, mut app) = app_with_source(SOURCE);
    let path = app.project.entry.clone();
    let source = format!(
        "asset art image \"art.png\"\n{}",
        app.project.document(&path).unwrap()
    );
    app.project.set_text(&path, source).unwrap();
    app.project.save().unwrap();
    let asset = app.project.root.join("art.png");
    std::fs::write(&asset, b"fixture bytes").unwrap();
    app.project.refresh().unwrap();
    let before = app.manuscript.query_cache.applied(&app.project).unwrap();
    let original_key = app.project.manuscript_query_key(&[], &[]);
    std::fs::remove_file(&asset).unwrap();
    assert_eq!(app.project.manuscript_query_key(&[], &[]), original_key);
    app.manuscript.invalidate_query_cache();
    let host_refresh = app.manuscript.query_cache.applied(&app.project).unwrap();
    assert_ne!(
        before.key(),
        host_refresh.key(),
        "浏览器/宿主recompile显式失效后内容key重新绑定元数据"
    );
    app.project.refresh().unwrap();
    let native_refresh = app.manuscript.query_cache.applied(&app.project).unwrap();
    let builds = app.manuscript.query_cache.builds();
    assert!(native_refresh
        .query(&ManuscriptQueryRequest {
            manuscript_id: "book".into(),
            ..Default::default()
        })
        .unwrap()
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "A215"));
    app.project.refresh().unwrap();
    let again = app.manuscript.query_cache.applied(&app.project).unwrap();
    assert!(std::sync::Arc::ptr_eq(&native_refresh, &again));
    assert_eq!(app.manuscript.query_cache.builds(), builds);
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn reopening_no_selection_keeps_filters_and_does_not_choose_first_chapter() {
    let (ctx, mut app) = app_with_source(SOURCE);
    let size = vec2(1280.0, 800.0);
    settle(&ctx, &mut app, size);
    app.manuscript.books.get_mut("book").unwrap().selected_entry = None;
    app.manuscript.navigation.session.text = "First".into();
    app.manuscript.navigation.session.columns.identity = true;
    let session = app.manuscript_session();
    app.manuscript = WorkbenchState::default();
    app.restore_manuscript_session(session);
    settle(&ctx, &mut app, size);
    assert!(app.manuscript.books["book"].selected_entry.is_none());
    assert!(app.manuscript.active_writing_target().is_none());
    assert_eq!(app.manuscript.navigation.session.text, "First");
    assert!(app.manuscript.navigation.session.columns.identity);
}
