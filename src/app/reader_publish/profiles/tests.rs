use super::*;
use std::sync::atomic::AtomicU64;

fn profile_fixture_root(
    parent: &std::path::Path,
    nonce: u128,
    next: &AtomicU64,
) -> std::io::Result<std::path::PathBuf> {
    for _ in 0..64 {
        let sequence = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = parent.join(format!(
            "reader-profile-ui-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        match std::fs::create_dir(&root) {
            Ok(()) => return Ok(root),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "无法在 64 次尝试内创建独占发布配置夹具目录",
    ))
}

fn app() -> crate::app::WorldeditApp {
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx);
    let mut app = crate::app::WorldeditApp::new(&creation, None);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);
    let root = profile_fixture_root(&std::env::temp_dir(), nonce, &NEXT_ROOT).unwrap();
    app.project = Project::new(&root);
    let entry = app.project.entry.clone();
    app.project.documents.retain(|path, _| path == &entry);
    app.project
        .set_text(&entry, "entity a kind place as \"公开资料\"\n".into())
        .unwrap();
    app.project.create_authoring_document(&app.project.root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.10","required_features":["content.entities.v1"],"maps":{}}"#.to_vec()).unwrap();
    app.active_file = entry;
    app.project.save().unwrap();
    app.recompile();
    app.open_reader_publish();
    app.reader_publish
        .objects
        .insert(TargetRef::new("entity", "a"));
    app.reader_publish.profile_id = "public".into();
    app.reader_publish.profile_title = "公开配置".into();
    app
}

#[test]
fn profile_fixture_roots_are_unique_for_concurrent_identical_nonces() {
    const WORKERS: usize = 8;
    const NONCE: u128 = 42;
    let parent = app().project.root.clone();
    let next = AtomicU64::new(0);
    let barrier = std::sync::Barrier::new(WORKERS);
    let roots = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..WORKERS)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    profile_fixture_root(&parent, NONCE, &next).unwrap()
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        roots
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        WORKERS,
        "相同 nonce 的并发夹具必须分配不同目录"
    );
    for root in roots {
        assert_eq!(root.parent(), Some(parent.as_path()));
        assert!(root.is_dir(), "分配成功前必须独占创建夹具目录");
        assert!(std::fs::read_dir(root).unwrap().next().is_none());
    }
}

#[test]
fn profile_fixture_root_does_not_reuse_an_existing_directory_or_sentinel() {
    const NONCE: u128 = 42;
    let parent = app().project.root.clone();
    let initial_sequence = AtomicU64::new(0);
    let existing = profile_fixture_root(&parent, NONCE, &initial_sequence).unwrap();
    std::fs::create_dir_all(&existing).unwrap();
    let sentinel = existing.join("sentinel.txt");
    std::fs::write(&sentinel, b"existing fixture must remain untouched").unwrap();
    let repeated_sequence = AtomicU64::new(0);
    let allocated = profile_fixture_root(&parent, NONCE, &repeated_sequence).unwrap();
    assert_eq!(
        std::fs::read(&sentinel).unwrap(),
        b"existing fixture must remain untouched"
    );
    assert_ne!(allocated, existing, "已有目录不得被新的夹具复用");
    assert_eq!(allocated.parent(), Some(parent.as_path()));
    assert!(allocated.is_dir(), "新目录必须在返回前完成独占创建");
    assert!(std::fs::read_dir(allocated).unwrap().next().is_none());
}

#[test]
fn profile_application_uses_normal_history_and_does_not_claim_disk_save() {
    let mut app = app();
    let baseline = app.project.content_baseline();
    let source = app.project.sources();
    let history = app.history.len();
    app.reader_profile_action(PublishAction::SaveProfile, &egui::Context::default());
    wait_for_profile(&mut app);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), history + 1);
    assert_eq!(
        app.history.last().unwrap().snapshot.content_baseline(),
        baseline
    );
    assert_ne!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.sources(), source);
    assert!(app.project.is_dirty());
    let profiles = app.project.reader_profiles().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].id, "public");
    assert_eq!(profiles[0].title, "公开配置");
    let path = app.project.reader_profile_paths()["public"].clone();
    assert!(!path.exists(), "应用配置不能自动写盘");
    assert!(app
        .reader_publish
        .status
        .as_ref()
        .unwrap()
        .contains("尚需保存全部"));
}

#[test]
fn profile_migration_rejects_a_changed_candidate_without_editing_the_project() {
    let mut app = app();
    let mut selection = app.reader_publish.selection();
    selection.schema_version = worldline_core::reader_export::READER_FIELDS_SCHEMA_VERSION;
    selection.required_features = vec![worldline_core::reader_export::READER_FIELDS_FEATURE.into()];
    let profile = app
        .project
        .create_reader_profile("legacy", &selection)
        .unwrap();
    app.reader_publish.load_profile(profile);
    let baseline = app.project.content_baseline();
    app.reader_profile_action(PublishAction::PreviewMigration, &egui::Context::default());
    assert!(app.reader_publish.migration.is_some());
    app.reader_publish.site_title = "预览后改了标题".into();
    app.reader_profile_action(PublishAction::ConfirmMigration, &egui::Context::default());
    assert_eq!(app.reader_publish.selection().schema_version, 2);
    assert!(app
        .reader_publish
        .status
        .as_ref()
        .unwrap()
        .contains("重新预览"));
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn confirmed_migration_keeps_routes_and_remains_an_unsaved_candidate() {
    let mut app = app();
    let mut selection = app.reader_publish.selection();
    selection.schema_version = worldline_core::reader_export::READER_FIELDS_SCHEMA_VERSION;
    selection.required_features = vec![worldline_core::reader_export::READER_FIELDS_FEATURE.into()];
    let profile = app
        .project
        .create_reader_profile("legacy", &selection)
        .unwrap();
    let routes = profile.routes.clone();
    app.reader_publish.load_profile(profile);
    let baseline = app.project.content_baseline();
    app.reader_profile_action(PublishAction::PreviewMigration, &egui::Context::default());
    app.reader_profile_action(PublishAction::ConfirmMigration, &egui::Context::default());
    assert_eq!(app.reader_publish.selection().schema_version, 3);
    assert_eq!(app.reader_publish.profile.as_ref().unwrap().routes, routes);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

fn wait_for_profile(app: &mut crate::app::WorldeditApp) {
    for _ in 0..1000 {
        app.poll_reader_profile_job();
        if app.reader_publish.profile_job.is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("发布配置后台任务超时");
}

#[test]
fn profile_only_apply_preserves_fresh_analysis_and_undo_redo_dirty_state() {
    let mut app = app();
    let source = app.project.sources();
    let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
    let version = app.version;
    let generation = app.map_revision.content_generation;
    app.reader_profile_action(PublishAction::SaveProfile, &egui::Context::default());
    wait_for_profile(&mut app);
    assert_eq!(app.version, version + 1);
    assert_eq!(app.map_revision.content_generation, generation + 1);
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
    assert!(app.project.is_dirty());
    app.undo(false);
    assert!(app.project.reader_profiles().unwrap().is_empty());
    assert!(app.reader_publish.profiles.is_empty());
    assert!(!app.project.is_dirty());
    app.undo(true);
    assert_eq!(app.project.reader_profiles().unwrap().len(), 1);
    assert_eq!(app.reader_publish.profiles.len(), 1);
    assert!(app.project.is_dirty());
    assert_eq!(app.project.sources(), source);
    assert_eq!(app.project.compile().analysis.fingerprint, fingerprint);
}

fn pending_plan(app: &mut crate::app::WorldeditApp) {
    let input = app.reader_profile_input();
    let mut profile = app
        .project
        .create_reader_profile(&input.id, &input.selection)
        .unwrap();
    profile.title = input.title.clone();
    let plan = app.project.preview_save_reader_profile(&profile).unwrap();
    app.reader_publish.profile_plan = Some((input, plan));
}

#[test]
fn stale_analysis_or_form_cannot_be_marked_fresh_by_profile_only_apply() {
    let mut app = app();
    pending_plan(&mut app);
    let baseline = app.project.content_baseline();
    let source = app.snapshot.as_ref().unwrap().result.sources.clone();
    app.snapshot.as_mut().unwrap().result.sources.clear();
    assert!(!app.apply_reader_profile_plan());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    assert!(app.reader_publish.profile_plan.is_some());
    assert!(app.snapshot.as_ref().unwrap().result.sources.is_empty());
    app.snapshot.as_mut().unwrap().result.sources = source;
    app.stale_form = true;
    assert!(!app.apply_reader_profile_plan());
    assert_eq!(app.project.content_baseline(), baseline);
    app.stale_form = false;
    assert!(app.apply_reader_profile_plan());
}

#[test]
fn profile_planning_does_not_apply_before_main_poll_and_scope_change_rejects_done() {
    let mut app = app();
    let baseline = app.project.content_baseline();
    let ctx = egui::Context::default();
    app.start_reader_profile_save(&ctx);
    assert!(app.reader_publish.profile_job.is_some());
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    app.reader_publish.profile_title = "核对时改了名称".into();
    wait_for_profile(&mut app);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    assert!(app.reader_publish.status.as_ref().unwrap().contains("改变"));
}

#[test]
fn global_close_waits_for_profile_planning_and_discards_a_queued_valid_plan() {
    use super::super::profile_job::{ProfileJob, ProfileMessage};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    };
    let mut app = app();
    pending_plan(&mut app);
    let (input, plan) = app.reader_publish.profile_plan.take().unwrap();
    let baseline = app.project.content_baseline();
    let (sender, receiver) = mpsc::channel();
    sender
        .send(ProfileMessage::Done(Box::new(Ok(plan))))
        .unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    app.reader_publish.profile_job = Some(ProfileJob {
        input,
        cancel: cancel.clone(),
        receiver: Some(receiver),
    });
    let ctx = egui::Context::default();
    app.perform_action(crate::app::Pending::Close, &ctx);
    assert!(app.reader_app_close_pending());
    assert!(!app.allow_close);
    assert!(cancel.load(Ordering::Acquire));
    assert!(!app.poll_reader_app_close(&ctx), "规划线程仍存活时不可退出");
    drop(sender);
    let mut ready = false;
    for _ in 0..1000 {
        if app.poll_reader_app_close(&ctx) {
            ready = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(ready);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    assert!(app.project.reader_profiles().unwrap().is_empty());
}

#[test]
fn cancelling_profile_task_and_reopening_preserves_inputs_but_not_review_or_plan() {
    let mut app = app();
    pending_plan(&mut app);
    let profile = app
        .reader_publish
        .profile_plan
        .as_ref()
        .unwrap()
        .1
        .profile
        .clone();
    app.reader_publish.load_profile(profile);
    app.reader_publish.profile_title = "尚未应用的配置名称".into();
    app.reader_publish.site_title = "尚未应用的站点标题".into();
    app.reader_publish.query = "保留搜索".into();
    let selection = app.reader_publish.selection();
    let profile = app.reader_publish.current_profile();
    let id = app.reader_publish.profile_id.clone();
    let baseline = app.project.content_baseline();
    let ctx = egui::Context::default();
    app.start_reader_profile_save(&ctx);
    assert!(app.reader_publish.profile_job.is_some());
    app.reader_publish.confirmed = true;
    assert!(app.cancel_reader_publish());
    assert!(!app.reader_publish.open);
    assert!(!app.reader_publish.confirmed);
    assert!(app.reader_publish.profile_job.is_none());
    assert!(app.reader_publish.reviewed.is_none());
    assert!(app.reader_publish.profile_plan.is_none());
    app.open_reader_publish();
    assert!(app.reader_publish.open);
    assert_eq!(app.reader_publish.selection(), selection);
    assert_eq!(app.reader_publish.current_profile(), profile);
    assert_eq!(app.reader_publish.profile_id, id);
    assert_eq!(app.reader_publish.query, "保留搜索");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    app.reader_profile_action(PublishAction::NewProfile, &ctx);
    assert!(app.reader_publish.profile.is_none());
    assert!(app.reader_publish.profile_id.is_empty());
    assert!(app.reader_publish.profile_title.is_empty());
    assert!(!app.reader_publish.has_selection());
}

#[path = "tests/performance.rs"]
mod performance;
