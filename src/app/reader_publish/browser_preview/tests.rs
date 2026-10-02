use super::*;

fn files() -> archive::Files {
    BTreeMap::from([
        (PathBuf::from("index.html"), b"<p>reviewed</p>".to_vec()),
        (PathBuf::from("assets/image.png"), vec![1, 2, 3]),
    ])
}

fn wait_removed(path: &Path) {
    for _ in 0..200 {
        if !path.exists() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!path.exists(), "只清理本任务私有目录：{}", path.display());
}

#[test]
fn materialization_contains_only_reviewed_files_and_cleans_its_own_directory() {
    let files = files();
    let directory = materialize(&files, &AtomicBool::new(false), &mut |_, _| {}).unwrap();
    assert_eq!(
        fs::read(directory.0.join("index.html")).unwrap(),
        files[Path::new("index.html")]
    );
    assert!(!directory.0.join("world.wl").exists());
    let path = directory.0.clone();
    drop(directory);
    assert!(!path.exists());
    let unsafe_files = BTreeMap::from([(PathBuf::from("../escape.html"), vec![])]);
    assert!(materialize(&unsafe_files, &AtomicBool::new(false), &mut |_, _| {}).is_err());
}

#[test]
fn materialization_checks_cancellation_between_sixty_four_kib_chunks() {
    let files = BTreeMap::from([(PathBuf::from("index.html"), vec![b'x'; 64 * 1024 * 3])]);
    let cancel = AtomicBool::new(false);
    let mut bytes = 0;
    let result = materialize(&files, &cancel, &mut |completed, _| {
        bytes = completed;
        cancel.store(true, Ordering::Release);
    });
    assert!(result.is_err());
    assert_eq!(bytes, 64 * 1024);
}

fn app() -> (egui::Context, crate::app::WorldeditApp) {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = crate::app::WorldeditApp::new(&creation, None);
    app.reader_publish = ReaderPublishState::new();
    app.reader_publish.open = true;
    app.reader_publish.reviewed = Some(ReviewedPackage {
        selection: app.reader_publish.selection(),
        profile: None,
        preview: ReaderExportPreview {
            schema_version: 3,
            plan_digest: "browser-preview-fixture".into(),
            content_baseline: app.project.content_baseline(),
            included: vec![],
            exclusions: vec![],
            content: vec![worldline_core::reader_export::ReaderContentPreview {
                title: "公开页".into(),
                output_path: "index.html".into(),
                text: "reviewed".into(),
                empty_content: false,
            }],
        },
        files: Arc::new(files()),
        zip: Arc::new(vec![]),
        raw_bytes: 18,
    });
    (ctx, app)
}

fn queue_job(app: &mut crate::app::WorldeditApp, receiver: mpsc::Receiver<PreviewMessage>) {
    let reviewed = app.reader_publish.reviewed.as_ref().unwrap();
    app.reader_publish.browser_preview_job = Some(BrowserPreviewJob {
        cancel: Arc::new(AtomicBool::new(false)),
        receiver: Some(receiver),
        baseline: reviewed.preview.content_baseline.clone(),
        digest: reviewed.preview.plan_digest.clone(),
        selection: reviewed.selection.clone(),
        profile: reviewed.profile.clone(),
        page: "index.html".into(),
    });
}

#[test]
fn stale_reviewed_baseline_cannot_use_the_cached_fast_path() {
    let (_, mut app) = app();
    let directory = materialize(&files(), &AtomicBool::new(false), &mut |_, _| {}).unwrap();
    let digest = app
        .reader_publish
        .reviewed
        .as_ref()
        .unwrap()
        .preview
        .plan_digest
        .clone();
    PREVIEWS.with(|previews| {
        previews.borrow_mut().insert(digest.clone(), directory);
    });
    app.reader_publish
        .reviewed
        .as_mut()
        .unwrap()
        .preview
        .content_baseline = "older-baseline".into();
    let before = OPEN_REQUESTS.with(|count| count.get());
    app.open_reader_browser_preview("index.html".into());
    assert_eq!(OPEN_REQUESTS.with(|count| count.get()), before);
    assert!(app.reader_publish.browser_preview_job.is_none());
    assert!(app.reader_publish.status.as_ref().unwrap().contains("过期"));
    PREVIEWS.with(|previews| {
        previews.borrow_mut().remove(&digest);
    });
}

#[test]
fn disconnected_materialization_is_a_terminal_error_not_permanent_busy() {
    let (_, mut app) = app();
    let (sender, receiver) = mpsc::channel();
    drop(sender);
    queue_job(&mut app, receiver);
    app.poll_reader_browser_preview();
    assert!(app.reader_publish.browser_preview_job.is_none());
    assert!(app
        .reader_publish
        .status
        .as_ref()
        .unwrap()
        .contains("意外中断"));
}

#[test]
fn dropping_a_job_with_queued_directory_cleans_it_without_opening() {
    let (_, mut app) = app();
    let directory = materialize(&files(), &AtomicBool::new(false), &mut |_, _| {}).unwrap();
    let path = directory.0.clone();
    let (sender, receiver) = mpsc::channel();
    assert!(sender.send(PreviewMessage::Done(Ok(directory))).is_ok());
    drop(sender);
    queue_job(&mut app, receiver);
    let before = OPEN_REQUESTS.with(|count| count.get());
    app.cancel_reader_publish();
    wait_removed(&path);
    assert_eq!(OPEN_REQUESTS.with(|count| count.get()), before);
}

fn window_frame(
    ctx: &egui::Context,
    app: &mut crate::app::WorldeditApp,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1188.0, 848.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| app.reader_publish_window(ctx),
    )
}

fn close_cross(shape: &egui::Shape, rect: egui::Rect) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::LineSegment { points, .. } => {
            let delta = points[1] - points[0];
            let center = points[0] + delta * 0.5;
            (rect.contains(center)
                && center.x > rect.right() - 50.0
                && center.y < rect.top() + 45.0
                && delta.x.abs() > 5.0
                && (delta.x.abs() - delta.y.abs()).abs() < 0.5)
                .then_some(center)
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| close_cross(shape, rect)),
        _ => None,
    }
}

#[test]
fn queued_completion_and_real_window_close_click_never_open_the_page() {
    let (ctx, mut app) = app();
    for _ in 0..3 {
        window_frame(&ctx, &mut app, vec![]);
    }
    let bounds = ctx
        .memory(|memory| memory.area_rect(egui::Id::new("reader-publish-window")))
        .unwrap();
    let output = window_frame(&ctx, &mut app, vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| close_cross(&shape.shape, bounds))
        .expect("实际窗口必须绘制可点击的X");
    let directory = materialize(&files(), &AtomicBool::new(false), &mut |_, _| {}).unwrap();
    let path = directory.0.clone();
    let (sender, receiver) = mpsc::channel();
    assert!(sender.send(PreviewMessage::Done(Ok(directory))).is_ok());
    drop(sender);
    queue_job(&mut app, receiver);
    let before = OPEN_REQUESTS.with(|count| count.get());
    for pressed in [true, false] {
        window_frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(point),
                egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert!(!app.reader_publish.open, "真实X点击必须关闭窗口");
    assert_eq!(
        OPEN_REQUESTS.with(|count| count.get()),
        before,
        "已排队Done不能越过X打开页面"
    );
    wait_removed(&path);
}
