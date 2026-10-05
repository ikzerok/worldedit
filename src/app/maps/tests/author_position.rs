//! 当前 core 身份上的跨视图导航回归；不替代原生输入验证。
use super::tests_support::*;
use super::*;
use crate::app::{Tab, WorldeditApp};

fn setup(name: &str) -> (PathBuf, egui::Context, WorldeditApp) {
    let root = navigation_workspace(name);
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let app = WorldeditApp::new(&creation, Some(root.join("world.wl")));
    (root, ctx, app)
}
fn frame(ctx: &egui::Context, app: &mut WorldeditApp) {
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1188.0, 848.0))),
            ..Default::default()
        },
        |ctx| app.map_tab(ctx),
    );
}
fn documents(app: &WorldeditApp) -> Vec<(PathBuf, Vec<u8>)> {
    app.project
        .authoring_documents
        .iter()
        .map(|(path, doc)| (path.clone(), doc.bytes().to_vec()))
        .collect()
}
fn map(app: &mut WorldeditApp, ctx: &egui::Context, id: &str) {
    app.tab = Tab::Map;
    app.map_selection = Some(id.into());
    frame(ctx, app);
}

#[test]
fn catalog_reading_map_and_back_restores_the_exact_reading_departure_without_writes() {
    let (root, ctx, mut app) = setup("author-reading-back");
    let before = (
        app.project.sources(),
        documents(&app),
        app.project.is_dirty(),
        app.map_revision,
    );
    let target = TargetRef::new("world", "harbor");
    app.tab = Tab::Catalog;
    app.catalog_target = Some(target.clone());
    app.open_reading(TargetRef::new("asset", "harbor_image"));
    app.open_reading(target.clone());
    let history = app.reading_history.clone();
    app.locate_reference("harbor", "lighthouse");
    frame(&ctx, &mut app);
    assert_eq!(app.tab, Tab::Map);
    assert!(app.reading_target.is_none());
    assert_eq!(
        app.map_canvas.selected_placement().unwrap().id,
        "lighthouse"
    );
    app.author_back(&ctx);
    assert_eq!(app.tab, Tab::Catalog);
    assert_eq!(app.catalog_target, Some(target.clone()));
    assert_eq!(app.reading_target, Some(target));
    assert_eq!(app.reading_history, history);
    assert!(app.personal.history.is_empty());
    app.author_back(&ctx);
    assert_eq!(
        (
            app.project.sources(),
            documents(&app),
            app.project.is_dirty(),
            app.map_revision
        ),
        before
    );
    assert!(app.history.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn source_to_map_back_restores_both_unicode_cursor_ends_and_scroll() {
    let (root, ctx, mut app) = setup("author-source-map-back");
    app.tab = Tab::Edit;
    app.personal.source_scroll = [13.0, 77.0];
    let id = egui::Id::new(("source", &app.active_file));
    let mut state = egui::TextEdit::load_state(&ctx, id).unwrap_or_default();
    state.cursor.set_char_range(Some(egui::text::CCursorRange {
        primary: egui::text::CCursor::new(22),
        secondary: egui::text::CCursor::new(17),
        h_pos: None,
    }));
    state.store(&ctx, id);
    let location = app.author_location(Some(&ctx));
    app.locate_reference_from("city", "harbor-return", location);
    frame(&ctx, &mut app);
    app.author_back(&ctx);
    let selection = egui::TextEdit::load_state(&ctx, id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!(
        (selection.primary.index, selection.secondary.index),
        (22, 17)
    );
    assert_eq!(app.personal.source_scroll, [13.0, 77.0]);
    assert_eq!(app.tab, Tab::Edit);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn map_a_source_map_b_returns_source_then_a_selection_camera_and_submap_path() {
    let (root, ctx, mut app) = setup("author-map-a-b");
    map(&mut app, &ctx, "harbor");
    let target = app.map_canvas.snapshot.layers[0].placements[0]
        .navigation
        .clone()
        .unwrap();
    assert!(app.enter_submap(target));
    frame(&ctx, &mut app);
    assert!(app.map_canvas.select_placement_id("harbor-return"));
    app.map_canvas.camera.pan_by(vec2(81.0, -33.0));
    let camera = app.map_canvas.camera_state();
    let before = (app.project.sources(), documents(&app), app.map_revision);
    app.jump_to_file(&root.join("world.wl").to_string_lossy(), 2, 1);
    assert_eq!(app.tab, Tab::Edit);
    app.locate_reference("harbor", "lighthouse");
    frame(&ctx, &mut app);
    app.author_back(&ctx);
    assert_eq!(app.tab, Tab::Edit);
    app.author_back(&ctx);
    assert_eq!(app.tab, Tab::Map);
    frame(&ctx, &mut app);
    assert_eq!(app.map_canvas.map_id(), "city");
    assert_eq!(
        app.map_canvas.selected_placement().unwrap().id,
        "harbor-return"
    );
    assert_eq!(app.map_canvas.camera_state(), camera);
    assert_eq!(app.map_navigation.as_ref().unwrap().history_len(), 1);
    assert!(app.back_from_map_navigation());
    frame(&ctx, &mut app);
    assert_eq!(app.map_canvas.map_id(), "harbor");
    assert_eq!(
        (app.project.sources(), documents(&app), app.map_revision),
        before
    );
    assert!(!app.project.is_dirty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn hidden_placement_is_not_revealed_by_return_and_draft_retains_history_for_retry() {
    let (root, ctx, mut app) = setup("author-hidden-draft");
    map(&mut app, &ctx, "harbor");
    app.map_canvas.select_placement_id("lighthouse");
    let location = app.author_location(None);
    app.map_canvas.set_layer_visible("places", false);
    app.tab = Tab::Edit;
    app.remember_author_location(location);
    app.map_canvas.draft = Some(MapGeometry::Point(NormalizedPoint::new(0.2, 0.3)));
    app.author_back(&ctx);
    assert_eq!(app.personal.history.len(), 1);
    assert_eq!(app.tab, Tab::Edit);
    assert!(app.map_canvas.draft.is_some());
    app.map_canvas.draft = None;
    app.author_back(&ctx);
    assert_eq!(app.tab, Tab::Map);
    assert!(app.map_canvas.selected_placement().is_none());
    assert!(!app.map_canvas.placement_layer("lighthouse").unwrap().1);
    assert!(app.message.as_deref().unwrap().contains("隐藏"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn changed_or_missing_map_identity_never_reapplies_old_camera_or_geometry() {
    let (root, ctx, mut app) = setup("author-stale-map");
    map(&mut app, &ctx, "harbor");
    app.map_canvas.select_placement_id("lighthouse");
    app.map_canvas.restore_camera(navigation::CameraState {
        zoom: 7.0,
        pan: [71.0, 88.0],
    });
    let saved = app.author_location(None);
    let path = root.join(".world/maps/harbor.json");
    let mut current: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&path).unwrap().bytes()).unwrap();
    current["placements"]["lighthouse"]["geometry"]["position"] = serde_json::json!([0.9, 0.8]);
    app.project
        .set_authoring_document(&path, serde_json::to_vec(&current).unwrap())
        .unwrap();
    app.recompile();
    let current_bytes = documents(&app);
    app.tab = Tab::Edit;
    app.restore_author_location(saved.clone(), &ctx);
    assert_eq!(app.tab, Tab::Map);
    assert_ne!(app.map_canvas.camera_state().zoom, 7.0);
    assert!(app.map_canvas.selected_placement().is_none());
    assert_eq!(documents(&app), current_bytes);
    assert!(app.message.as_deref().unwrap().contains("来源版本已变化"));
    app.snapshot
        .as_mut()
        .unwrap()
        .map_index
        .maps
        .remove("harbor");
    app.tab = Tab::Catalog;
    app.restore_author_location(saved, &ctx);
    assert_eq!(app.tab, Tab::Catalog);
    assert!(app.message.as_deref().unwrap().contains("已不存在"));
    assert_eq!(documents(&app), current_bytes);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn external_change_and_missing_marker_refuse_departure_without_consuming_history() {
    let (root, _ctx, mut app) = setup("author-external-map");
    app.tab = Tab::Catalog;
    app.locate_reference("harbor", "missing");
    assert_eq!(app.tab, Tab::Catalog);
    assert!(app.personal.history.is_empty());
    let path = root.join(".world/maps/harbor.json");
    let old = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, old.replace("雾港地图", "外部新地图")).unwrap();
    let before = documents(&app);
    app.locate_reference("harbor", "lighthouse");
    assert_eq!(app.tab, Tab::Catalog);
    assert!(app.personal.history.is_empty());
    assert!(app.message.as_deref().unwrap().contains("尚未确认"));
    assert_eq!(documents(&app), before);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn scene_selection_restores_all_valid_nodes_but_not_hidden_descendants() {
    let (root, ctx, mut app) = setup("author-scene-selection");
    use worldline_core::vector_scene::{MapScene, SceneGeometry, SceneNode};
    let mut scene = MapScene::new(1000.0, 600.0);
    for id in ["one", "two"] {
        scene.nodes.insert(
            id.into(),
            SceneNode::new(
                id,
                "places",
                SceneGeometry::Rect {
                    x: 10.0,
                    y: 10.0,
                    width: 25.0,
                    height: 25.0,
                    rx: 0.0,
                    ry: 0.0,
                },
            ),
        );
    }
    scene
        .root_order
        .insert("places".into(), vec!["one".into(), "two".into()]);
    app.snapshot
        .as_mut()
        .unwrap()
        .map_index
        .maps
        .get_mut("harbor")
        .unwrap()
        .scene = Some(scene);
    map(&mut app, &ctx, "harbor");
    app.map_canvas.select_scene("one", true);
    app.map_canvas.select_scene("two", true);
    let saved = app.author_location(None);
    app.map_selection = Some("city".into());
    frame(&ctx, &mut app);
    app.restore_author_location(saved.clone(), &ctx);
    assert_eq!(app.map_canvas.scene.selection.len(), 2);
    assert!(app.map_canvas.selected_placement().is_none());
    app.map_canvas.set_layer_visible("places", false);
    app.restore_author_location(saved, &ctx);
    assert!(app.map_canvas.scene.selection.is_empty());
    assert!(!app.map_canvas.placement_layer("one").unwrap().1);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn map_reference_names_are_human_first_and_stable_ids_remain_visible() {
    let (root, _ctx, mut app) = setup("author-map-names");
    let map = app
        .snapshot
        .as_mut()
        .unwrap()
        .map_index
        .maps
        .get_mut("harbor")
        .unwrap();
    map.title = "很长的中文地图标题，仍然需要完整可读".repeat(5);
    map.placements.get_mut("lighthouse").unwrap().label_override = Some("灯塔入口（东岸）".into());
    let (caption, identity) = app.map_reference_caption("harbor", "lighthouse").unwrap();
    assert!(caption.starts_with("很长的中文地图标题"));
    assert!(caption.ends_with("灯塔入口（东岸）"));
    assert_eq!(identity, "harbor · lighthouse · 图层 places");
    let _ = std::fs::remove_dir_all(root);
}
