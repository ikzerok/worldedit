#[path = "../src/app/maps/navigation.rs"]
mod navigation;

use navigation::{
    project_navigation, project_placement_navigation, Breadcrumb, CameraState,
    MapNavigationController, MapNavigationDto, PlacementNavigationDto,
};
use worldline_core::presentation::{MapNavigation, MapPlacement};

fn target(map_id: &str, available: bool) -> MapNavigationDto {
    MapNavigationDto {
        map_id: map_id.into(),
        available,
    }
}

fn camera(zoom: f32, pan: [f32; 2]) -> CameraState {
    CameraState { zoom, pan }
}

#[test]
fn core_navigation_projects_to_a_clickable_map_target() {
    let navigation = MapNavigation {
        map_id: "city".into(),
        available: true,
        extra: Default::default(),
    };

    assert_eq!(
        project_navigation(Some(&navigation)),
        Some(MapNavigationDto {
            map_id: "city".into(),
            available: true,
        })
    );
    assert_eq!(project_navigation(None), None);
}

#[test]
fn placement_projection_preserves_navigation_without_copying_map_content() {
    let placement = MapPlacement {
        id: "city-entry".into(),
        layer_id: "places".into(),
        target_ref: None,
        geometry: worldline_core::presentation::MapGeometry::point([0.2, 0.3]),
        annotation: "进入城市".into(),
        role: "地点入口".into(),
        label_override: None,
        navigation: Some(MapNavigation {
            map_id: "city".into(),
            available: true,
            extra: Default::default(),
        }),
        scope_refs: Vec::new(),
        style: None,
        extensions: Default::default(),
        extra: Default::default(),
    };

    assert_eq!(
        project_placement_navigation(&placement),
        PlacementNavigationDto {
            placement_id: "city-entry".into(),
            navigation: Some(MapNavigationDto {
                map_id: "city".into(),
                available: true,
            }),
        }
    );
}

#[test]
fn entering_maps_keeps_actual_path_and_restores_each_camera_on_back() {
    let mut controller =
        MapNavigationController::new("overview", "总览", camera(1.0, [12.0, -4.0]));

    assert!(controller.enter(&target("city", true), "城市", camera(1.5, [20.0, 8.0]),));
    controller.update_current_camera(camera(1.75, [33.0, 44.0]));
    assert!(controller.enter(
        &target("lighthouse", true),
        "灯塔",
        camera(2.5, [-3.0, 18.0]),
    ));
    assert_eq!(
        controller.breadcrumbs(),
        vec![
            Breadcrumb::new("overview", "总览"),
            Breadcrumb::new("city", "城市"),
            Breadcrumb::new("lighthouse", "灯塔"),
        ]
    );

    assert_eq!(
        controller.back().map(|view| view.map_id),
        Some("city".into())
    );
    assert_eq!(controller.current().camera, camera(1.75, [33.0, 44.0]));
    assert_eq!(controller.breadcrumbs().len(), 2);

    assert_eq!(
        controller.back().map(|view| view.map_id),
        Some("overview".into())
    );
    assert_eq!(controller.current().camera, camera(1.0, [12.0, -4.0]));
    assert!(controller.back().is_none());
}

#[test]
fn cycles_and_multiple_entries_are_kept_as_distinct_path_steps() {
    let mut controller = MapNavigationController::new("overview", "总览", camera(1.0, [0.0, 0.0]));

    assert!(controller.enter(&target("city", true), "城市", camera(2.0, [1.0, 2.0]),));
    assert!(controller.enter(&target("overview", true), "总览", camera(3.0, [3.0, 4.0]),));

    assert_eq!(
        controller
            .breadcrumbs()
            .into_iter()
            .map(|item| item.map_id)
            .collect::<Vec<_>>(),
        vec!["overview", "city", "overview"]
    );
    assert_eq!(
        controller.back().map(|view| view.map_id),
        Some("city".into())
    );
    assert_eq!(controller.current().camera, camera(2.0, [1.0, 2.0]));
    assert_eq!(
        controller.back().map(|view| view.map_id),
        Some("overview".into())
    );
    assert_eq!(controller.current().camera, camera(1.0, [0.0, 0.0]));
}

#[test]
fn unavailable_targets_do_not_change_path() {
    let mut controller = MapNavigationController::new("overview", "总览", CameraState::default());

    assert!(!controller.enter(
        &target("missing", false),
        "缺失地图",
        CameraState::default(),
    ));
    assert_eq!(controller.history_len(), 0);
    assert_eq!(controller.current().map_id, "overview");
}

#[test]
fn history_is_capped_without_deduplicating_recent_path() {
    let mut controller = MapNavigationController::new("root", "根", CameraState::default());

    for index in 0..100 {
        let map_id = format!("map-{index}");
        assert!(controller.enter(
            &target(&map_id, true),
            map_id.clone(),
            CameraState::default(),
        ));
    }

    assert_eq!(controller.history_len(), 64);
    let breadcrumbs = controller.breadcrumbs();
    assert_eq!(breadcrumbs.len(), 65);
    assert_eq!(
        breadcrumbs.first().map(|item| item.map_id.as_str()),
        Some("map-35")
    );
    assert_eq!(
        breadcrumbs.last().map(|item| item.map_id.as_str()),
        Some("map-99")
    );
}
