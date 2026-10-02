use super::*;

fn profile() -> ReaderPublicationProfile {
    ReaderPublicationProfile {
        schema_version: 1,
        required_features: vec![READER_PROFILES_FEATURE.into()],
        id: "saved".into(),
        title: "配置".into(),
        routes: vec![],
        selection: ReaderExportSelection {
            schema_version: 3,
            required_features: vec![
                READER_SITE_FEATURE.into(),
                READER_FIELDS_FEATURE.into(),
                "future.must_refuse".into(),
            ],
            site_title: "阅读".into(),
            objects: vec![TargetRef::new("entity", "z"), TargetRef::new("entity", "a")],
            fields: vec![ReaderFieldSelection {
                target: TargetRef::new("entity", "z"),
                keys: vec!["z".into(), "a".into()],
            }],
            manuscripts: vec![ReaderManuscriptSelection {
                id: "book".into(),
                chapters: vec!["chapter_z".into(), "chapter_a".into()],
            }],
            maps: vec![ReaderMapSelection {
                id: "gone".into(),
                placements: vec!["lost".into()],
                raster_layers: vec!["raster".into()],
            }],
            attachments: vec!["z".into(), "a".into()],
        },
    }
}

#[test]
fn missing_profile_choices_features_and_original_order_survive_refresh() {
    let profile = profile();
    let mut state = ReaderPublishState::default();
    state.load_profile(profile.clone());
    let project = worldline_core::project::Project::new(
        &std::env::temp_dir().join("reader-profile-refresh-test"),
    );
    state.refresh_choices(&project, None);
    assert_eq!(state.selection(), profile.selection);
    assert_eq!(state.current_profile(), Some(profile));
}

#[test]
fn v2_empty_fields_keep_mandatory_feature_and_future_features_are_not_downgraded() {
    let mut profile = profile();
    profile.selection.schema_version = 2;
    profile.selection.fields.clear();
    profile.selection.required_features =
        vec![READER_FIELDS_FEATURE.into(), "future.must_refuse".into()];
    let mut state = ReaderPublishState::default();
    state.load_profile(profile.clone());
    assert_eq!(state.selection(), profile.selection);
}

#[test]
fn newly_selected_chapters_append_in_candidate_order_without_reordering_saved_ones() {
    let mut state = ReaderPublishState::default();
    state.load_profile(profile());
    state.manuscript_choices = vec![ManuscriptChoice {
        id: "book".into(),
        title: "书".into(),
        unavailable: None,
        chapters: vec![("new_z".into(), "Z".into()), ("new_a".into(), "A".into())],
    }];
    state
        .chapters
        .get_mut("book")
        .unwrap()
        .extend(["new_a".into(), "new_z".into()]);
    assert_eq!(
        state.selection().manuscripts[0].chapters,
        ["chapter_z", "chapter_a", "new_z", "new_a"]
    );
}

#[test]
fn opening_an_existing_wizard_preserves_its_selection_and_profile_input() {
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx);
    let mut app = crate::app::WorldeditApp::new(&creation, None);
    app.open_reader_publish();
    app.reader_publish.profile_id = "unsaved-profile".into();
    app.reader_publish.site_title = "未应用的标题".into();
    app.reader_publish
        .objects
        .insert(TargetRef::new("entity", "selected"));
    let selection = app.reader_publish.selection();
    app.open_reader_publish();
    assert_eq!(app.reader_publish.selection(), selection);
    assert_eq!(app.reader_publish.profile_id, "unsaved-profile");
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn reopening_a_busy_wizard_keeps_the_job_result_channel() {
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx);
    let mut app = crate::app::WorldeditApp::new(&creation, None);
    let (_sender, receiver) = std::sync::mpsc::channel();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    app.reader_publish.job = Some(ReaderPublishJob {
        cancel: cancel.clone(),
        receiver: Some(receiver),
        generation: 7,
    });
    app.reader_publish.open = false;
    app.open_reader_publish();
    assert!(app.reader_publish.open);
    assert_eq!(app.reader_publish.job.as_ref().unwrap().generation, 7);
    assert!(!cancel.load(std::sync::atomic::Ordering::Acquire));
}

#[test]
fn indexed_ordering_preserves_original_order_and_invalid_duplicate_authorizations() {
    let selected = ["z".to_owned(), "a".to_owned(), "new".to_owned()]
        .into_iter()
        .collect();
    let old = ["z".into(), "z".into(), "a".into(), "removed".into()];
    let candidates = ["new".into(), "a".into()];
    assert_eq!(
        ordered(&selected, &old, &candidates),
        vec!["z", "z", "a", "new"]
    );
}
