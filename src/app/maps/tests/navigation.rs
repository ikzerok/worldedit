use super::tests_support::*;
use super::*;
#[test]
fn map_navigation_entry_and_back_keep_path_and_camera() {
    let root = navigation_workspace("submap-entry-back");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    let sources_before = app.project.sources();
    let revision_before = app.map_revision;
    let documents_before = app
        .project
        .authoring_documents
        .iter()
        .map(|(path, document)| (path.clone(), document.bytes().to_vec()))
        .collect::<Vec<_>>();
    assert!(!app.project.is_dirty());
    app.tab = super::super::Tab::Map;
    app.map_selection = Some("harbor".into());
    let screen_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0));
    let frame = || RawInput {
        screen_rect: Some(screen_rect),
        ..Default::default()
    };

    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    assert_eq!(app.map_selection.as_deref(), Some("harbor"));
    app.map_canvas.camera.pan_by(vec2(37.0, -19.0));
    let harbor_camera = *app.map_canvas.camera();
    let target = app.map_canvas.snapshot.layers[0].placements[0]
        .navigation
        .clone()
        .expect("child map navigation");

    assert!(app.enter_submap(target));
    assert_eq!(app.map_selection.as_deref(), Some("city"));
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    assert_eq!(app.map_canvas.map_id(), "city");
    assert_eq!(
        app.map_navigation
            .as_ref()
            .expect("navigation state")
            .breadcrumbs()
            .iter()
            .map(|crumb| crumb.map_id.as_str())
            .collect::<Vec<_>>(),
        vec!["harbor", "city"]
    );

    app.map_canvas.camera.pan_by(vec2(-11.0, 23.0));
    let city_camera = *app.map_canvas.camera();
    let return_target = app.map_canvas.snapshot.layers[0].placements[0]
        .navigation
        .clone()
        .expect("return map navigation");
    assert!(app.enter_submap(return_target));
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    assert_eq!(app.map_canvas.map_id(), "harbor");
    assert_eq!(
        app.map_navigation
            .as_ref()
            .expect("navigation state")
            .breadcrumbs()
            .iter()
            .map(|crumb| crumb.map_id.as_str())
            .collect::<Vec<_>>(),
        vec!["harbor", "city", "harbor"]
    );

    assert!(app.back_from_map_navigation());
    assert_eq!(app.map_selection.as_deref(), Some("city"));
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    assert_eq!(app.map_canvas.map_id(), "city");
    assert_eq!(*app.map_canvas.camera(), city_camera);
    assert!(app.back_from_map_navigation());
    assert_eq!(app.map_selection.as_deref(), Some("harbor"));
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    assert_eq!(app.map_canvas.map_id(), "harbor");
    assert_eq!(*app.map_canvas.camera(), harbor_camera);
    assert_eq!(
        app.map_navigation
            .as_ref()
            .expect("navigation state")
            .breadcrumbs()
            .len(),
        1
    );

    assert_eq!(app.project.sources(), sources_before);
    assert_eq!(app.map_revision, revision_before);
    assert!(!app.project.is_dirty());
    assert!(app.history.is_empty());
    assert_eq!(
        app.project
            .authoring_documents
            .iter()
            .map(|(path, document)| (path.clone(), document.bytes().to_vec()))
            .collect::<Vec<_>>(),
        documents_before
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn navigation_refuses_to_leave_with_unsubmitted_canvas_or_form_state() {
    let root = navigation_workspace("navigation-pending-guard");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.tab = super::super::Tab::Map;
    app.map_selection = Some("harbor".into());
    let frame = || RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
        ..Default::default()
    };
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    let target = app.map_canvas.snapshot.layers[0].placements[0]
        .navigation
        .clone()
        .expect("child map navigation");
    let source_before = app.project.sources();
    let revision_before = app.map_revision;
    let bytes_before = app
        .project
        .authoring_documents
        .iter()
        .map(|(path, document)| (path.clone(), document.bytes().to_vec()))
        .collect::<Vec<_>>();
    let history_before = app.history.len();

    app.map_canvas.draft = Some(MapGeometry::Point(NormalizedPoint::new(0.2, 0.3)));
    app.map_form.annotation = "尚未提交的说明".into();
    assert!(!app.enter_submap(target));
    assert_eq!(app.map_selection.as_deref(), Some("harbor"));
    assert_eq!(app.map_canvas.map_id(), "harbor");
    assert!(app.map_canvas.draft.is_some());
    assert_eq!(app.map_form.annotation, "尚未提交的说明");
    assert!(app
        .message
        .as_deref()
        .is_some_and(|message| message.contains("提交") && message.contains("取消")));
    assert_eq!(app.project.sources(), source_before);
    assert_eq!(app.map_revision, revision_before);
    assert_eq!(app.history.len(), history_before);
    assert_eq!(
        app.project
            .authoring_documents
            .iter()
            .map(|(path, document)| (path.clone(), document.bytes().to_vec()))
            .collect::<Vec<_>>(),
        bytes_before
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn navigation_refuses_return_with_a_failed_preview() {
    let root = navigation_workspace("navigation-failed-preview-guard");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.tab = super::super::Tab::Map;
    app.map_selection = Some("harbor".into());
    let frame = || RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
        ..Default::default()
    };
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    let target = app.map_canvas.snapshot.layers[0].placements[0]
        .navigation
        .clone()
        .expect("child map navigation");
    assert!(app.enter_submap(target));
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    app.map_failed_command = Some(PendingMapCommand {
        map_id: "city".into(),
        intent: EditIntent::Create(MapGeometry::Point(NormalizedPoint::new(0.4, 0.4))),
    });
    assert!(!app.back_from_map_navigation());
    assert_eq!(app.map_selection.as_deref(), Some("city"));
    assert_eq!(
        app.map_navigation
            .as_ref()
            .expect("navigation state")
            .history_len(),
        1
    );
    assert!(app
        .message
        .as_deref()
        .is_some_and(|message| message.contains("提交") && message.contains("取消")));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn manual_map_switch_with_pending_form_keeps_current_map() {
    let root = navigation_workspace("navigation-manual-switch-guard");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.tab = super::super::Tab::Map;
    app.map_selection = Some("harbor".into());
    let frame = || RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
        ..Default::default()
    };
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    app.map_form.target_query = "未提交查询".into();
    app.map_selection = Some("city".into());
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    assert_eq!(app.map_selection.as_deref(), Some("harbor"));
    assert_eq!(app.map_canvas.map_id(), "harbor");
    assert_eq!(app.map_form.target_query, "未提交查询");
    assert!(app
        .message
        .as_deref()
        .is_some_and(|message| message.contains("提交") && message.contains("取消")));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn locating_reference_with_pending_geometry_keeps_current_map() {
    let root = navigation_workspace("navigation-locate-guard");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.tab = super::super::Tab::Map;
    app.map_selection = Some("harbor".into());
    let frame = || RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
        ..Default::default()
    };
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    app.map_canvas.draft = Some(MapGeometry::Point(NormalizedPoint::new(0.5, 0.5)));
    app.locate_reference("city", "harbor-return");
    assert_eq!(app.map_selection.as_deref(), Some("harbor"));
    assert!(app.map_locate_request.is_none());
    assert_eq!(app.tab, super::super::Tab::Map);
    assert!(app
        .message
        .as_deref()
        .is_some_and(|message| message.contains("提交") && message.contains("取消")));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn invalid_selected_map_clears_navigation_and_canvas() {
    let root = navigation_workspace("navigation-invalid-selection");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.tab = super::super::Tab::Map;
    app.map_selection = Some("harbor".into());
    let frame = || RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
        ..Default::default()
    };
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    assert!(app.map_navigation.is_some());
    assert!(!app.map_canvas.map_id().is_empty());

    app.map_selection = Some("deleted-map".into());
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    assert!(app.map_navigation.is_none());
    assert!(app.map_canvas.map_id().is_empty());
    assert!(app.map_canvas.snapshot.layers.is_empty());
    assert!(app.map_canvas.snapshot.raster_layers.is_empty());
    assert!(app
        .message
        .as_deref()
        .is_some_and(|message| message.contains("不可用") || message.contains("失效")));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn invalid_map_keeps_form_until_cancel_then_allows_another_map() {
    let root = navigation_workspace("navigation-invalid-form");
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
    app.tab = super::super::Tab::Map;
    app.map_selection = Some("harbor".into());
    let frame = || RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
        ..Default::default()
    };
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    app.map_form.annotation = "外部失效时仍需保留".into();
    app.map_selection = Some("deleted-map".into());
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    assert!(app.map_navigation.is_none());
    assert_eq!(app.map_selection, None);
    assert_eq!(app.map_canvas.map_id(), "harbor");
    assert_eq!(app.map_form.annotation, "外部失效时仍需保留");

    app.cancel_map_form();
    app.map_selection = Some("city".into());
    let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
    assert_eq!(app.map_selection.as_deref(), Some("city"));
    assert_eq!(app.map_canvas.map_id(), "city");
    assert!(!app.map_form.has_uncommitted_work());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn snapshot_switch_rejects_unsubmitted_canvas_state() {
    let mut canvas = point_canvas();
    canvas.draft = Some(MapGeometry::Point(NormalizedPoint::new(0.6, 0.7)));
    let snapshot = MapRenderSnapshot {
        map_id: "other".into(),
        title: "另一张地图".into(),
        extent: vec2(500.0, 300.0),
        raster_layers: Vec::new(),
        layers: Vec::new(),
    };
    canvas.set_snapshot(1, snapshot);
    assert_eq!(canvas.map_id(), "map");
    assert!(canvas.draft.is_some());
}

#[test]
fn navigation_reset_rejects_unsubmitted_canvas_state() {
    let mut canvas = point_canvas();
    canvas
        .edit_intents
        .push(EditIntent::Create(MapGeometry::Point(
            NormalizedPoint::new(0.6, 0.7),
        )));
    canvas.reset_for_navigation();
    assert_eq!(canvas.edit_intents().len(), 1);
}

#[test]
fn pointer_zoom_keeps_anchor_without_dirtying_project() {
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = super::super::WorldeditApp::new(&creation, None);
    app.tab = super::super::Tab::Map;
    let source_before = app.project.sources();
    let dirty_before = app.project.is_dirty();
    let pointer = pos2(430.0, 300.0);

    let first = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))),
        events: vec![Event::PointerMoved(pointer)],
        ..Default::default()
    };
    let _ = ctx.run(first, |ctx| {
        app.map_tab(ctx);
    });
    let zoom_before = app.map_canvas.camera().zoom();
    let map_point_before = app
        .map_canvas
        .camera()
        .screen_to_normalized(pointer, app.map_canvas.viewport());

    let second = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))),
        events: vec![
            Event::PointerMoved(pointer),
            Event::MouseWheel {
                unit: MouseWheelUnit::Point,
                delta: vec2(0.0, 120.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
        ..Default::default()
    };
    let _ = ctx.run(second, |ctx| {
        app.map_tab(ctx);
    });

    assert!(app.map_canvas.camera().zoom() > zoom_before);
    let map_point_after = app
        .map_canvas
        .camera()
        .screen_to_normalized(pointer, app.map_canvas.viewport());
    assert!((map_point_after.x - map_point_before.x).abs() < 1e-4);
    assert!((map_point_after.y - map_point_before.y).abs() < 1e-4);
    assert_eq!(app.map_canvas.edit_intents().len(), 0);
    assert_eq!(app.project.sources(), source_before);
    assert_eq!(app.project.is_dirty(), dirty_before);
}
