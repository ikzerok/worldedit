#[path = "../src/app/network_state.rs"]
mod network_state;
use network_state::{GraphCamera, NetworkState};
use worldline_core::{
    compile_source_with_options, CompileOptions, RelationQueryDirection, TargetRef,
};

fn content() -> worldline_core::CompileResult {
    compile_source_with_options("network.wl", "entity place kind place as \"同名\"\nentity group kind organization as \"同名\"\ncharacter person as \"人物\"\nrelation_type knows as \"认识\"\nrelation_def one type knows from entity place to entity group\nrelation_def two type knows from entity group to character person\nrelation_def parallel type knows from entity place to entity group\n", CompileOptions::v1_10())
}
#[test]
fn local_navigation_layout_hiding_and_query_cache_do_not_mutate_core() {
    let content = content();
    assert!(!content.has_errors(), "{:?}", content.diagnostics);
    let catalog = &content.analysis.catalog;
    let fingerprint = content.analysis.fingerprint;
    let sources = content.sources.clone();
    let mut state = NetworkState::default();
    state.set_focus(TargetRef::new("entity", "place"));
    assert!(state.refresh(catalog, 1));
    assert_eq!(state.result.as_ref().unwrap().edges.len(), 2);
    assert_eq!(state.result.as_ref().unwrap().nodes.len(), 2);
    for _ in 0..100 {
        assert!(!state.refresh(catalog, 1));
    }
    state.filters.depth = 2;
    assert!(state.refresh(catalog, 1));
    assert_eq!(state.result.as_ref().unwrap().nodes.len(), 3);
    assert_eq!(state.result.as_ref().unwrap().edges.len(), 3);
    state.hide("parallel");
    assert!(state.hidden.contains("parallel"));
    assert_eq!(catalog.relations.len(), 3);
    let before = state.positions["entity:place"];
    state.begin_drag("entity:place");
    state.drag_to([120.0, -35.0]);
    state.cancel_drag();
    assert_eq!(state.positions["entity:place"], before);
    state.begin_drag("entity:place");
    state.drag_to([12.0, 34.0]);
    state.finish_drag();
    assert_eq!(state.positions["entity:place"], [12.0, 34.0]);
    let draft = state.draft("view".into(), "测试".into()).unwrap();
    assert_eq!(draft.hidden_relation_ids, ["parallel"]);
    assert_eq!(draft.positions["entity:place"], [12.0, 34.0]);
    state.show_all();
    assert!(state.hidden.is_empty());
    state.reset_layout();
    assert_eq!(state.positions["entity:place"], [0.0, 0.0]);
    assert_eq!(content.sources, sources);
    assert_eq!(content.analysis.fingerprint, fingerprint);
}
#[test]
fn pagination_replaces_bounded_pages_and_filter_change_resets_cursor() {
    let mut source = String::from("entity hub kind place\nrelation_type links as \"连接\"\n");
    for i in 0..600 {
        source.push_str(&format!(
            "entity n{i} kind place\nrelation_def r{i} type links from entity hub to entity n{i}\n"
        ));
    }
    let content = compile_source_with_options("network.wl", &source, CompileOptions::v1_10());
    assert!(!content.has_errors());
    let mut state = NetworkState::default();
    state.set_focus(TargetRef::new("entity", "hub"));
    let mut ids = std::collections::BTreeSet::new();
    loop {
        state.refresh(&content.analysis.catalog, 4);
        let query = state.result.as_ref().unwrap();
        assert!(query.nodes.len() <= 250 && query.edges.len() <= 500);
        for edge in &query.edges {
            assert!(ids.insert(edge.id.clone()));
        }
        if !state.next_page() {
            break;
        }
    }
    assert_eq!(ids.len(), 600);
    assert!(state.previous_page());
    state.refresh(&content.analysis.catalog, 4);
    state.filters.direction = RelationQueryDirection::Incoming;
    state.refresh(&content.analysis.catalog, 4);
    assert_eq!(state.offset, 0);
    assert!(state.result.as_ref().unwrap().edges.is_empty());
    state.filters.direction = RelationQueryDirection::Both;
    state.refresh(&content.analysis.catalog, 5);
    assert_eq!(state.offset, 0);
}
#[test]
fn graph_camera_keeps_zoom_anchor_and_ignores_nonfinite_drag() {
    let mut camera = GraphCamera::default();
    let size = [900.0, 700.0];
    let cursor = [120.0, 260.0];
    let world = camera.canvas_to_world(cursor, size);
    camera.zoom_at(1.7, cursor, size);
    let after = camera.world_to_canvas(world, size);
    assert!((after[0] - cursor[0]).abs() < 1e-8 && (after[1] - cursor[1]).abs() < 1e-8);
    camera.fit(&[[0.0, 0.0], [500.0, 300.0]], size);
    assert!(camera.zoom > 0.0 && camera.zoom.is_finite());
    let content = content();
    let mut state = NetworkState::default();
    state.set_focus(TargetRef::new("entity", "place"));
    state.refresh(&content.analysis.catalog, 0);
    state.begin_drag("entity:place");
    let before = state.positions["entity:place"];
    state.drag_to([f64::NAN, 0.0]);
    assert_eq!(state.positions["entity:place"], before);
}

#[test]
fn navigation_restores_personal_state_and_loading_keeps_saved_data() {
    let content = content();
    let mut state = NetworkState::default();
    state.enter(TargetRef::new("entity", "place"));
    state.refresh(&content.analysis.catalog, 1);
    state.camera.pan_by([30.0, -20.0]);
    state.hide("one");
    let draft = state.draft("saved".into(), "布局".into()).unwrap();
    state.enter(TargetRef::new("entity", "group"));
    assert!(state.can_back());
    assert!(state.back());
    assert_eq!(state.camera.pan, [30.0, -20.0]);
    assert!(state.hidden.contains("one"));
    state.load(&draft);
    assert!(!state.can_back());
    assert!(!state.back());
    assert!(!state.can_previous());
    state.refresh(&content.analysis.catalog, 1);
    state.begin_drag("entity:place");
    assert_eq!(state.dragging(), Some("entity:place"));
    state.drag_to([80.0, 50.0]);
    state.refresh(&content.analysis.catalog, 2);
    assert!(state.dragging().is_none());
    assert_eq!(state.positions["entity:place"], [0.0, 0.0]);
}
