use super::*;

pub(super) fn fixture() -> WorldeditApp {
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx);
    let mut app = WorldeditApp::new(&creation, None);
    static NEXT_ROOT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "production-ui-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT_ROOT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).expect("each fixture exclusively creates a fresh root");
    app.project = worldline_core::project::Project::new(&root);
    let path = app.project.entry.clone();
    app.project.set_text(&path, "character a as \"同名\"\ncharacter b as \"同名\"\nevent start\n  say a \"第一句\" direction \"PRIVATE_DIRECTION\"\n  say b \"第二句\"\n  -> END\n".into()).unwrap();
    app.project.create_authoring_document(&app.project.root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.11","required_features":[],"maps":{},"graph_views":{}}"#.to_vec()).unwrap();
    app.project.save().unwrap();
    app.recompile();
    app.manuscript.production.scope = 2;
    app
}
pub(super) fn generate(app: &mut WorldeditApp) {
    let input = app.production_input().unwrap();
    let snapshot = app
        .project
        .production_script_snapshot(&app.production_buffers(), &input.drafts, &input.request)
        .unwrap();
    app.manuscript.production.page = Some(snapshot.page(0, 40).unwrap());
    app.manuscript.production.snapshot = Some(Arc::new(snapshot));
    app.manuscript.production.captured = Some(input);
}
#[test]
fn production_ui_filters_formal_speaker_and_current_identity_before_delivery() {
    let mut app = fixture();
    app.manuscript.production.speaker = Some(worldline_core::TargetRef::new("character", "a"));
    generate(&mut app);
    assert!(app.production_is_current());
    let page = app.manuscript.production.page.as_ref().unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.rows[0].speaker.as_ref().unwrap().target.id, "a");
    assert!(page.rows[0].direction.is_none());
    app.manuscript.production.speaker = Some(worldline_core::TargetRef::new("character", "b"));
    assert!(!app.production_is_current());
    let _ = std::fs::remove_dir_all(app.project.root);
}
#[test]
fn production_ui_uses_exact_core_artifact_and_resets_confirmation_for_option_changes() {
    let mut app = fixture();
    let ctx = egui::Context::default();
    generate(&mut app);
    app.finish_production_export(&ctx, delivery::Action::Preview);
    let artifact = app.manuscript.production.artifact.as_ref().unwrap();
    let bytes = artifact.bytes().to_vec();
    assert!(!String::from_utf8_lossy(&bytes).contains("PRIVATE_DIRECTION"));
    app.manuscript.production.confirmed = true;
    assert_eq!(
        app.checked_production_artifact(&ctx).unwrap().bytes(),
        bytes
    );
    app.manuscript.production.direction = true;
    assert!(app.checked_production_artifact(&ctx).is_err());
    app.finish_production_export(&ctx, delivery::Action::Preview);
    assert!(!app.manuscript.production.confirmed);
    assert!(
        String::from_utf8_lossy(app.manuscript.production.artifact.as_ref().unwrap().bytes())
            .contains("PRIVATE_DIRECTION")
    );
    let _ = std::fs::remove_dir_all(app.project.root);
}
#[test]
fn production_ui_invalid_draft_and_newer_source_never_reuse_current_receipt() {
    let mut app = fixture();
    generate(&mut app);
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.project.entry)
        .unwrap();
    buffer.replace_source("event start\n  if (\n".into());
    app.manuscript
        .writing_buffers
        .insert(buffer.path().into(), buffer);
    assert!(!app.production_is_current());
    assert!(app
        .project
        .production_script_snapshot(
            &app.production_buffers(),
            &app.production_drafts(),
            &app.production_request().unwrap()
        )
        .is_err());
    let _ = std::fs::remove_dir_all(app.project.root);
}

#[test]
fn production_freshness_detects_undo_branch_with_the_same_buffer_generation() {
    let mut app = fixture();
    let target = worldline_core::TargetRef::new("event", "start");
    let original = app.project.open_writing_buffer(&target).unwrap();
    let mut first = original.clone();
    first.replace_source(first.source().replace("第一句", "同代次甲"));
    app.manuscript
        .writing_buffers
        .insert(first.path().into(), first.clone());
    generate(&mut app);
    assert!(app.production_is_current());
    let mut branched = original;
    branched.replace_source(branched.source().replace("第一句", "同代次乙"));
    assert_eq!(first.generation(), branched.generation());
    app.manuscript
        .writing_buffers
        .insert(branched.path().into(), branched);
    assert!(!app.production_is_current());
    let _ = std::fs::remove_dir_all(app.project.root);
}
#[test]
fn production_navigation_receipt_ignores_open_clean_buffers_consistently() {
    let mut app = fixture();
    let clean = app
        .project
        .open_writing_buffer(&worldline_core::TargetRef::new("event", "start"))
        .unwrap();
    app.manuscript
        .writing_buffers
        .insert(clean.path().into(), clean);
    generate(&mut app);
    let snapshot = app.manuscript.production.snapshot.as_ref().unwrap();
    let row = &app.manuscript.production.page.as_ref().unwrap().rows[0];
    assert!(app
        .project
        .production_script_source_hit(
            &app.production_buffers(),
            &app.production_drafts(),
            snapshot,
            &row.row_key
        )
        .is_ok());
    assert!(app.production_is_current());
    let _ = std::fs::remove_dir_all(app.project.root);
}

#[test]
fn production_explicit_character_id_validates_draft_only_characters_in_core() {
    let mut app = fixture();
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.project.entry)
        .unwrap();
    let source = format!(
        "character draft_role as \"稿中新人物\"\n{}",
        buffer.source().replace("say a", "say draft_role")
    );
    buffer.replace_source(source);
    app.manuscript
        .writing_buffers
        .insert(buffer.path().into(), buffer);
    app.manuscript.production.speaker =
        Some(worldline_core::TargetRef::new("character", "draft_role"));
    generate(&mut app);
    let row = &app.manuscript.production.page.as_ref().unwrap().rows[0];
    assert_eq!(row.speaker.as_ref().unwrap().target.id, "draft_role");
    assert_eq!(row.speaker.as_ref().unwrap().display, "稿中新人物");
    assert!(!app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .symbols
        .characters
        .contains_key("draft_role"));
    app.manuscript.production.speaker =
        Some(worldline_core::TargetRef::new("character", "missing_role"));
    assert!(app
        .project
        .production_script_snapshot(
            &app.production_buffers(),
            &app.production_drafts(),
            &app.production_request().unwrap()
        )
        .is_err());
    let _ = std::fs::remove_dir_all(app.project.root);
}

#[test]
fn production_host_observation_failure_then_recovery_never_revives_old_receipt() {
    // 小宿主结果入口注入，不是操作系统权限失败/原生窗口测试。
    let mut app = fixture();
    let buffer = app
        .project
        .open_source_writing_buffer(&app.project.entry)
        .unwrap();
    app.manuscript
        .writing_buffers
        .insert(buffer.path().into(), buffer.clone());
    generate(&mut app);
    let key = app
        .manuscript
        .production
        .snapshot
        .as_ref()
        .unwrap()
        .key()
        .to_owned();
    let history = (
        app.version,
        app.history.len(),
        app.redo.len(),
        app.project.content_baseline(),
    );
    app.manuscript.production.confirmed = true;
    assert!(
        !app.manuscript_refresh_required(false),
        "健康 same stamp 无需刷新"
    );
    assert!(
        app.manuscript_refresh_required(true),
        "健康 diff stamp 需要刷新"
    );
    app.manuscript_observation_result(false);
    let epoch = app.manuscript.production.observation_epoch;
    assert!(
        app.manuscript_refresh_required(false),
        "失败后的 same stamp 也必须刷新"
    );
    assert!(
        app.manuscript_refresh_required(true),
        "失败后的 diff stamp 必须刷新"
    );
    assert!(!app.production_is_current());
    assert!(!app.manuscript.production.confirmed);
    app.begin_production_script(&egui::Context::default());
    assert!(app.manuscript.production.job.is_none());
    assert!(app
        .manuscript
        .production
        .notice
        .as_deref()
        .unwrap()
        .contains("磁盘观察尚未恢复"));
    // 持续 scan Err 或 refresh Err 不先通知成功，不反复增加 epoch。
    for _ in 0..3 {
        app.manuscript_observation_result(false);
        assert!(app.manuscript_refresh_required(false));
        assert_eq!(app.manuscript.production.observation_epoch, epoch);
    }
    // 宿主仅在实际 refresh Ok 后调用成功入口。
    app.manuscript_observation_result(true);
    assert!(!app.manuscript_refresh_required(false));
    assert!(app.manuscript_refresh_required(true));
    assert!(!app.manuscript.production.observation_unavailable);
    assert_eq!(app.manuscript.production.observation_epoch, epoch);
    assert!(
        !app.production_is_current(),
        "相同 stamp 恢复不能复活旧确认"
    );
    assert_eq!(
        app.manuscript.production.snapshot.as_ref().unwrap().key(),
        key
    );
    assert_eq!(
        (
            app.version,
            app.history.len(),
            app.redo.len(),
            app.project.content_baseline()
        ),
        history
    );
    assert_eq!(
        app.manuscript.writing_buffers[buffer.path()].identity(),
        buffer.identity()
    );
    generate(&mut app);
    assert!(app.production_is_current());
    let _ = std::fs::remove_dir_all(app.project.root);
}
