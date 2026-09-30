use super::*;

#[test]
fn sorting_retires_inflight_query_and_cancel_discards_completed_results_without_writes() {
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let app = WorldeditApp::new(&creation, None);
    let baseline = app.project.content_baseline();
    let sources = app.project.sources();
    let mut state = WorkbenchState::default();
    let old_query = state.query.clone();
    let old_page = app
        .project
        .query_catalog(&old_query, current_options(&state))
        .unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    state.running = Some(RunningQuery {
        cancel: cancel.clone(),
        receiver,
        query: old_query,
        options: current_options(&state),
        cancel_requested: false,
    });
    state.apply_sort(
        &app,
        &ctx,
        Some(CatalogQuerySort {
            field: CatalogSortField::Name,
            direction: CatalogSortDirection::Descending,
        }),
    );
    assert!(cancel.load(std::sync::atomic::Ordering::Relaxed));
    assert!(sender.send((baseline.clone(), Ok(old_page))).is_err());
    state.cancel_query();
    for _ in 0..100 {
        state.poll_query(&app, &ctx);
        if state.running.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(state.running.is_none());
    assert!(state.page.is_none());
    assert!(state.error.as_ref().unwrap().contains("已取消"));
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.sources(), sources);
    assert!(app.history.is_empty());
}
