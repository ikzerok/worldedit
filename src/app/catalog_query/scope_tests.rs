use super::*;
use crate::app::Tab;

fn ready() -> (egui::Context, WorldeditApp, WorkbenchState) {
    let ctx = egui::Context::default();
    let app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let mut state = WorkbenchState::default();
    let snapshot = app
        .project
        .catalog_scope_snapshot(&state.query, state.max_candidates)
        .unwrap();
    state.page = Some(snapshot.query().page(0, state.page_size).unwrap());
    state.snapshot = Some(std::sync::Arc::new(snapshot));
    state.snapshot_key = Some((app.version, app.map_revision));
    state.snapshot_query = Some(state.query.clone());
    state.snapshot_observation = Some(app.project.catalog_scope_observation_key());
    (ctx, app, state)
}
#[test]
fn idle_query_frames_keep_same_snapshot_and_do_not_start_background_jobs() {
    let (ctx, mut app, mut state) = ready();
    let snapshot = state.snapshot.clone().unwrap();
    let baseline = app.project.content_baseline();
    for _ in 0..96 {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            },
            |ctx| state.render(&mut app, ctx),
        );
        assert!(state.running.is_none());
        assert!(!state.queued);
        assert!(std::sync::Arc::ptr_eq(
            state.snapshot.as_ref().unwrap(),
            &snapshot
        ));
    }
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}
#[test]
fn scope_clear_restores_exact_network_browsing_and_never_writes_project() {
    let (ctx, mut app, mut state) = ready();
    let focus = state.snapshot.as_ref().unwrap().query().matches()[0]
        .target
        .clone();
    app.network_state.set_focus(focus);
    app.network_state.camera.zoom = 1.75;
    app.network_state.camera.pan = [12.0, 23.0];
    app.network_view_id = "prior".into();
    app.network_view_title = "原视图".into();
    let baseline = app.project.content_baseline();
    app.tab = Tab::Catalog;
    state.enter_scope(&mut app, &ctx, true);
    assert!(state.scope.is_some());
    assert_eq!(app.tab, Tab::Network);
    app.network_state.camera.pan = [99.0, 99.0];
    state.clear_scope(&mut app, &ctx);
    assert!(state.scope.is_none());
    assert_eq!(app.tab, Tab::Catalog);
    assert_eq!(app.network_state.camera.pan, [12.0, 23.0]);
    assert_eq!(app.network_state.camera.zoom, 1.75);
    assert_eq!(app.network_view_id, "prior");
    assert_eq!(app.project.content_baseline(), baseline);
}
#[test]
fn stale_generation_and_ime_keep_scope_input_and_forbid_navigation() {
    let (ctx, mut app, mut state) = ready();
    app.tab = Tab::Catalog;
    state.enter_scope(&mut app, &ctx, false);
    assert!(state.scope.is_some());
    app.ime_composing = true;
    state.clear_scope(&mut app, &ctx);
    assert!(state.scope.is_some());
    app.ime_composing = false;
    app.version += 1;
    assert!(state.current_snapshot(&app).is_none());
    assert!(!state.scope_navigation_ready(&mut app));
    assert!(state.snapshot.is_some());
    state.clear_scope(&mut app, &ctx);
    assert!(state.scope.is_none());
}
#[test]
fn frozen_paging_does_not_recompile_and_options_change_marks_scope_stale() {
    let (_ctx, mut app, mut state) = ready();
    let before = state.snapshot.as_ref().unwrap().clone();
    state.page_size = 1;
    state.page = Some(before.query().page(0, 1).unwrap());
    let total = before.query().total();
    if total > 1 {
        state.next_page(&app);
        assert_eq!(state.page.as_ref().unwrap().offset, 1);
    }
    state.previous_page(&app, 0);
    assert_eq!(state.page.as_ref().unwrap().offset, 0);
    assert!(std::sync::Arc::ptr_eq(
        state.snapshot.as_ref().unwrap(),
        &before
    ));
    app.map_revision.presentation_generation += 1;
    state.previous_page(&app, 0);
    assert!(state.page.is_none());
}

#[test]
fn asset_observation_invalidates_both_cached_and_late_completed_scope_without_content_version_change(
) {
    let (ctx, mut app, mut state) = ready();
    let root = std::env::temp_dir().join(format!(
        "worldedit-scope-observation-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("world.wl"),
        "asset art image \"art.png\"\nevent start\n  -> END\n",
    )
    .unwrap();
    std::fs::write(root.join("art.png"), b"temporary scope fixture").unwrap();
    app.project = worldline_core::project::Project::open(&root).unwrap();
    app.recompile();
    let snapshot = app
        .project
        .catalog_scope_snapshot(&state.query, state.max_candidates)
        .unwrap();
    state.page = Some(snapshot.query().page(0, state.page_size).unwrap());
    state.snapshot = Some(std::sync::Arc::new(snapshot.clone()));
    state.snapshot_key = Some((app.version, app.map_revision));
    state.snapshot_query = Some(state.query.clone());
    state.snapshot_observation = Some(app.project.catalog_scope_observation_key());
    let baseline = app.project.content_baseline();
    let key = (app.version, app.map_revision);
    let (sender, receiver) = std::sync::mpsc::channel();
    state.running = Some(RunningQuery {
        cancel: Default::default(),
        receiver,
        query: state.query.clone(),
        options: current_options(&state),
        cancel_requested: false,
        key,
        observation: app.project.catalog_scope_observation_key(),
    });
    std::fs::remove_file(root.join("art.png")).unwrap();
    assert!(
        state.current_snapshot(&app).is_some(),
        "cache getters must not observe disk on their own"
    );
    assert!(app.project.refresh().unwrap().is_empty());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!((app.version, app.map_revision), key);
    assert!(state.current_snapshot(&app).is_none());
    sender.send((baseline, Ok(snapshot))).unwrap();
    state.poll_query(&app, &ctx);
    assert!(state.running.is_none());
    assert!(state.page.is_none());
    assert!(state
        .error
        .as_deref()
        .is_some_and(|error| error.contains("丢弃旧结果")));
    std::fs::remove_dir_all(root).unwrap();
}
