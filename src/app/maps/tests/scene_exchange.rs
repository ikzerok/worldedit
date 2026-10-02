use super::super::tests_support::test_workspace;
use super::*;
use crate::scene_raster::{render_scene, RasterSpec};
use worldline_core::vector_scene::map_to_safe_svg;

fn grouped_map(name: &str) -> (Project, MapDocument, std::path::PathBuf) {
    let root = test_workspace(name);
    let project = Project::open(&root.join("world.wl")).unwrap();
    let mut map = project.map_index().maps["harbor"].clone();
    let mut scene = worldline_core::svg_import::preview_scene(r##"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="200">
<g id="group" transform="translate(20 10)" opacity="0.5">
  <rect id="red" x="0" y="0" width="20" height="20" fill="#ff0000"/>
  <g id="nested" transform="translate(50 0)"><ellipse id="blue" cx="10" cy="10" rx="10" ry="10" fill="#0000ff"/></g>
</g><rect id="unselected" x="300" y="10" width="20" height="20" fill="#00ff00"/>
</svg>"##).unwrap().scene;
    let roots = scene.root_order.remove("svg").unwrap();
    scene.root_order.insert("places".into(), roots);
    for node in scene.nodes.values_mut() {
        node.layer_id = "places".into();
    }
    scene.nodes.get_mut("blue").unwrap().visible = false;
    map.scene = Some(scene);
    map.canvas.width = 400;
    map.canvas.height = 200;
    map.placements.clear();
    map.raster_layers.clear();
    (project, map, root)
}

use worldline_core::project::Project;

#[test]
fn author_group_selection_expands_nested_children_without_widening_reader_whitelist() {
    let (_, map, root) = grouped_map("selected-svg-ids");
    let selected = BTreeSet::from(["group".into()]);
    let exact = map_to_safe_svg(&map, Some(&selected)).unwrap();
    let exact_preview = worldline_core::svg_import::preview_scene(&exact).unwrap();
    assert!(
        worldline_core::vector_scene::project_scene(&exact_preview.scene, 0.1)
            .unwrap()
            .is_empty(),
        "reader's exact authorization remains exact; clipPath rectangles are not visible primitives"
    );
    let expanded = expand_author_selection(&map, &selected).unwrap();
    assert_eq!(
        expanded,
        BTreeSet::from(["group".into(), "nested".into(), "red".into(), "blue".into()])
    );
    assert!(expand_author_selection(&map, &BTreeSet::from(["missing".into()])).is_err());
    let leaf = expand_author_selection(&map, &BTreeSet::from(["red".into()])).unwrap();
    assert_eq!(leaf, BTreeSet::from(["red".into()]));
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn selected_group_background_export_reimport_has_visible_geometry_and_no_unselected_sibling() {
    let (project, map, root) = grouped_map("selected-svg-pixels");
    let selected = expand_author_selection(&map, &BTreeSet::from(["group".into()])).unwrap();
    let baseline = project.content_baseline();
    let ctx = egui::Context::default();
    let mut job = SvgExportJob::render(&project, map, Some(selected), &ctx).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let source = loop {
        if let Some(result) = job.poll() {
            let SvgExportEvent::Source(source) = result.unwrap() else {
                panic!("source expected");
            };
            break source;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "background selection export timeout"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    let preview = worldline_core::svg_import::preview_scene(&source).unwrap();
    assert_eq!(
        preview
            .scene
            .nodes
            .values()
            .filter(|node| matches!(
                node.geometry,
                SceneGeometry::Rect { .. } | SceneGeometry::Ellipse { .. }
            ))
            .count(),
        2
    );
    let pixels = render_scene(
        &preview.scene,
        [400.0, 200.0],
        &RasterSpec {
            width: 400,
            height: 200,
            zoom: 1.0,
            pan: [0.0, 0.0],
            dpi: 1.0,
        },
    )
    .unwrap();
    let pixel = |x: usize, y: usize| &pixels[(y * 400 + x) * 4..(y * 400 + x) * 4 + 4];
    let red = pixel(30, 20);
    let blue = pixel(80, 20);
    assert!(
        red[0] > 100 && (120..=136).contains(&red[3]),
        "selected group must render its red child and group opacity: {red:?}"
    );
    assert!(
        blue[2] > 100 && (120..=136).contains(&blue[3]),
        "exact author selection includes hidden nested child: {blue:?}"
    );
    assert_eq!(
        pixel(310, 20),
        &[0, 0, 0, 0],
        "unselected sibling leaked into exchange"
    );
    assert_eq!(project.content_baseline(), baseline);
    let _ = std::fs::remove_dir_all(root);
}
