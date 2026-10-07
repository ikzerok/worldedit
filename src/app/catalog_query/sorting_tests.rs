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

#[test]
fn delayed_sender_stays_pending_past_eighty_real_ui_frames_then_delivers() {
    let ctx = egui::Context::default();
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let baseline = app.project.content_baseline();
    let sources = app.project.sources();
    let mut state = WorkbenchState::default();
    let query = state.query.clone();
    let options = current_options(&state);
    let (sender, receiver) = std::sync::mpsc::channel();
    let (release, gate) = std::sync::mpsc::channel();
    let project = app.project.clone();
    let worker_query = query.clone();
    let worker = std::thread::spawn(move || {
        if gate.recv().is_ok() {
            let snapshot = project.content_baseline();
            let result = project
                .query_catalog(&worker_query, options)
                .map_err(|error| error.to_string());
            let _ = sender.send((snapshot, result));
        }
    });
    state.running = Some(RunningQuery {
        cancel: Default::default(),
        receiver,
        query,
        options,
        cancel_requested: false,
    });
    let render = |state: &mut WorkbenchState, app: &mut WorldeditApp| {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 800.0),
                )),
                ..Default::default()
            },
            |ctx| state.render(app, ctx),
        );
    };
    for _ in 0..96 {
        render(&mut state, &mut app);
        assert!(
            state.test_query_state().pending,
            "未释放sender时必须保持真实pending"
        );
        assert!(state.page.is_none());
        assert!(state.error.is_none());
    }
    release.send(()).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        render(&mut state, &mut app);
        let status = state.test_query_state();
        assert!(
            std::time::Instant::now() < deadline,
            "延迟sender释放后未结束：pending={}; {}",
            status.pending,
            status.details
        );
        if !status.pending {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    worker.join().unwrap();
    assert!(state.error.is_none(), "{:?}", state.error);
    let page = state
        .page
        .as_ref()
        .expect("必须收到完整页，不能把pending当空结果");
    assert_eq!(page.snapshot, baseline);
    assert!(page.total > 0);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.sources(), sources);
}

#[test]
fn disconnected_sender_is_an_explicit_terminal_error_not_permanent_pending() {
    let ctx = egui::Context::default();
    let app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let baseline = app.project.content_baseline();
    let mut state = WorkbenchState::default();
    let (sender, receiver) = std::sync::mpsc::channel();
    state.running = Some(RunningQuery {
        cancel: Default::default(),
        receiver,
        query: state.query.clone(),
        options: current_options(&state),
        cancel_requested: false,
    });
    drop(sender);
    state.poll_query(&app, &ctx);
    assert!(!state.test_query_state().pending);
    assert!(state.page.is_none());
    assert!(state.error.as_ref().unwrap().contains("查询任务意外结束"));
    assert_eq!(app.project.content_baseline(), baseline);
}
