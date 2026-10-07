use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
struct Fixture {
    app: WorldeditApp,
    ctx: egui::Context,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let ctx = egui::Context::default();
        let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        app.project = worldline_core::project::Project::new(&std::env::temp_dir().join(format!(
            "object-draft-search-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        app.active_file = app.project.entry.clone();
        app.project
            .set_text(
                &app.active_file.clone(),
                "event start as \"起点\"\n  正文\n  -> END\n".into(),
            )
            .unwrap();
        app.project.save().unwrap();
        app.recompile();
        Self { app, ctx }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.app.project.root);
    }
}

#[test]
fn current_catalog_compiles_once_per_source_generation_not_query_or_page() {
    let mut fixture = Fixture::new();
    let app = &mut fixture.app;
    app.project_query = "起点".into();
    let before = app.project.content_baseline();
    assert_eq!(
        app.search_objects_in_current_drafts().page.unwrap().total,
        1
    );
    assert_eq!(app.search_state.object_catalog.compilations, 1);
    for query in ["start", "EVENT", "world.wl", "起点", "无匹配"] {
        app.project_query = query.into();
        app.search_objects_in_current_drafts();
    }
    assert_eq!(app.search_state.object_catalog.compilations, 1);
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    buffer.replace_source("event start as \"当前稿新名称\"\n  还未应用\n  -> END\n".into());
    app.manuscript.restore_writing_buffers(&[buffer]);
    app.project_query = "当前稿新名称".into();
    let view = app.search_objects_in_current_drafts();
    assert!(!view.applied);
    assert_eq!(view.page.unwrap().total, 1);
    assert_eq!(app.search_state.object_catalog.compilations, 2);
    assert_eq!(app.project.content_baseline(), before);
    app.manuscript
        .writing_buffer_mut(&app.active_file)
        .unwrap()
        .replace_source("event start\n  {坏稿\n".into());
    app.project_query = "起点".into();
    let view = app.search_objects_in_current_drafts();
    assert!(view.applied);
    assert!(view.warning.unwrap().contains("已应用目录"));
    assert_eq!(view.page.unwrap().total, 1);
    assert_eq!(app.search_state.object_catalog.compilations, 3);
}

#[test]
fn object_navigation_rejects_external_source_changes_before_any_view_change() {
    let mut fixture = Fixture::new();
    let app = &mut fixture.app;
    app.project_query = "起点".into();
    let object = app
        .search_objects_in_current_drafts()
        .page
        .unwrap()
        .items
        .remove(0);
    let tab = app.tab;
    let active = app.active_file.clone();
    std::fs::write(&active, "event start as \"外部改写\"\n  -> END\n").unwrap();
    assert!(!app.navigate_object_candidate(&fixture.ctx, &object, false, true));
    assert_eq!(app.tab, tab);
    assert_eq!(app.active_file, active);
    assert!(app.reading_target.is_none());
    assert!(app.search_state.error.as_ref().unwrap().contains("外部"));
}

#[test]
fn stale_definition_line_is_rejected_without_guessing_same_id() {
    let mut fixture = Fixture::new();
    let app = &mut fixture.app;
    app.project_query = "起点".into();
    let object = app
        .search_objects_in_current_drafts()
        .page
        .unwrap()
        .items
        .remove(0);
    app.project
        .set_text(
            &app.active_file.clone(),
            "// 新首行\nevent start as \"起点\"\n  -> END\n".into(),
        )
        .unwrap();
    app.recompile();
    assert!(!app.navigate_object_candidate(&fixture.ctx, &object, false, true));
    assert!(!app.navigate_object_candidate(&fixture.ctx, &object, true, true));
    assert!(app.reading_target.is_none());
}

#[test]
fn replaced_buffer_with_same_generation_and_missing_fallback_never_reuses_old_catalog() {
    let mut fixture = Fixture::new();
    let app = &mut fixture.app;
    let base = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    let mut a = base.clone();
    let mut b = base;
    a.replace_source("event start as \"第一草稿\"\n  -> END\n".into());
    b.replace_source("event start as \"第二草稿\"\n  -> END\n".into());
    assert_eq!(a.generation(), b.generation());
    app.manuscript.restore_writing_buffers(&[a]);
    app.project_query = "第一草稿".into();
    assert_eq!(
        app.search_objects_in_current_drafts().page.unwrap().total,
        1
    );
    app.manuscript.restore_writing_buffers(&[b]);
    assert_eq!(
        app.search_objects_in_current_drafts().page.unwrap().total,
        0
    );
    app.project_query = "第二草稿".into();
    assert_eq!(
        app.search_objects_in_current_drafts().page.unwrap().total,
        1
    );
    app.manuscript
        .writing_buffer_mut(&app.active_file)
        .unwrap()
        .replace_source("event start\n  {坏稿\n".into());
    app.snapshot = None;
    let view = app.search_objects_in_current_drafts();
    assert!(view.applied);
    assert!(view.page.is_err());
    assert!(app.search_state.object_page.result.is_none());
}
