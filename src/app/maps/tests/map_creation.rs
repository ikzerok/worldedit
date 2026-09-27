use super::*;
#[test]
fn legacy_content_project_creates_map_from_form_and_undo_restores_it_once() {
    let root = std::env::temp_dir().join(format!(
        "worldedit-create-map-ui-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut project = worldline_core::project::Project::new(&root);
    let entry = project.entry.clone();
    project
        .set_text(&entry, "world empty as \"空白资料集\"\n".into())
        .unwrap();
    project.save().unwrap();
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx);
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.clone()));
    let sources = app.project.sources();
    let content_generation = app.map_revision.content_generation;
    assert!(!app.create_map_from_form());
    assert!(!app.project.is_dirty());
    app.map_canvas.set_mode(CanvasMode::Edit);
    app.map_creation.open_with_defaults(
        app.map_revision,
        worldline_core::map_creation::MISSING_DOCUMENT_HASH.into(),
    );
    app.map_creation.id = "harbor".into();
    app.map_creation.title = "港口草图".into();
    app.map_creation.width = "0".into();
    assert!(!app.create_map_from_form());
    assert!(app.map_creation.open);
    assert_eq!(app.map_creation.title, "港口草图");
    assert!(app.history.is_empty());
    app.map_creation.width = "640".into();
    assert!(app.create_map_from_form());
    assert_eq!(app.map_selection.as_deref(), Some("harbor"));
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .map_index
        .maps
        .contains_key("harbor"));
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.project.sources(), sources);
    assert_eq!(app.map_revision.content_generation, content_generation);
    assert!(!root.join(".world/project.json").exists());
    app.undo(false);
    assert!(app.snapshot.as_ref().unwrap().map_index.maps.is_empty());
    assert_eq!(app.project.sources(), sources);
    assert_eq!(app.map_revision.content_generation, content_generation);
    assert!(app.history.is_empty());
    assert_eq!(app.redo.len(), 1);
    let _ = std::fs::remove_dir_all(root);
}
