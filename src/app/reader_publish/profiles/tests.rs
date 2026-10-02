use super::*;

fn app() -> crate::app::WorldeditApp {
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx);
    let mut app = crate::app::WorldeditApp::new(&creation, None);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    app.project = Project::new(
        &std::env::temp_dir().join(format!("reader-profile-ui-{}-{nonce}", std::process::id())),
    );
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
fn profile_application_uses_normal_history_and_does_not_claim_disk_save() {
    let mut app = app();
    let baseline = app.project.content_baseline();
    let source = app.project.sources();
    let history = app.history.len();
    app.reader_profile_action(PublishAction::SaveProfile);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), history + 1);
    assert_eq!(app.history.last().unwrap().content_baseline(), baseline);
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
    app.reader_profile_action(PublishAction::PreviewMigration);
    assert!(app.reader_publish.migration.is_some());
    app.reader_publish.site_title = "预览后改了标题".into();
    app.reader_profile_action(PublishAction::ConfirmMigration);
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
    app.reader_profile_action(PublishAction::PreviewMigration);
    app.reader_profile_action(PublishAction::ConfirmMigration);
    assert_eq!(app.reader_publish.selection().schema_version, 3);
    assert_eq!(app.reader_publish.profile.as_ref().unwrap().routes, routes);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}
