use super::*;
use worldline_core::vector_scene::{PathSegment, SceneGeometry, SceneNode};

fn spec(size: u32, zoom: f32) -> RasterSpec {
    RasterSpec {
        width: size,
        height: size,
        zoom,
        pan: [0.0, 0.0],
        dpi: 1.0,
    }
}

fn alpha(bytes: &[u8], width: usize, x: usize, y: usize) -> u8 {
    bytes[(y * width + x) * 4 + 3]
}

fn scene_with(mut node: SceneNode) -> MapScene {
    node.style.fill = Some("#ff0000".into());
    let mut scene = MapScene::new(100.0, 100.0);
    scene
        .root_order
        .insert("layer".into(), vec![node.id.clone()]);
    scene.nodes.insert(node.id.clone(), node);
    scene
}

fn source_pixels(source: &str, spec: &RasterSpec) -> Result<Vec<u8>, String> {
    raster_safe_source(source, spec, spec.transform()?)
}

#[test]
fn actual_four_hundred_percent_pixels_are_regenerated() {
    let node = SceneNode::new(
        "rect",
        "layer",
        SceneGeometry::Rect {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
            rx: 0.0,
            ry: 0.0,
        },
    );
    let scene = scene_with(node);
    let normal = render_scene(&scene, [100.0, 100.0], &spec(100, 1.0)).unwrap();
    let large = render_scene(&scene, [100.0, 100.0], &spec(400, 4.0)).unwrap();
    assert_eq!(alpha(&normal, 100, 20, 20), 255);
    assert_eq!(alpha(&large, 400, 80, 80), 255);
    assert_eq!(alpha(&large, 400, 20, 20), 0);
    assert_eq!(large.len(), 400 * 400 * 4);
}

#[test]
fn compound_evenodd_hole_is_not_a_filled_polygon_approximation() {
    let square = |lo, hi| {
        vec![
            PathSegment::Move { to: [lo, lo] },
            PathSegment::Line { to: [hi, lo] },
            PathSegment::Line { to: [hi, hi] },
            PathSegment::Line { to: [lo, hi] },
            PathSegment::Close,
        ]
    };
    let mut segments = square(10.0, 90.0);
    segments.extend(square(30.0, 70.0));
    let mut node = SceneNode::new("hole", "layer", SceneGeometry::Path { segments });
    node.style.fill_rule = Some("evenodd".into());
    let bytes = render_scene(&scene_with(node), [100.0, 100.0], &spec(100, 1.0)).unwrap();
    assert_eq!(alpha(&bytes, 100, 20, 50), 255);
    assert_eq!(alpha(&bytes, 100, 50, 50), 0);
}

#[test]
fn group_opacity_is_applied_once_to_the_composited_group() {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
      <g opacity="0.5" fill="#ff0000"><rect x="10" y="10" width="50" height="50"/>
      <rect x="30" y="10" width="50" height="50"/></g></svg>"##;
    let bytes = source_pixels(source, &spec(100, 1.0)).unwrap();
    let non_overlap = alpha(&bytes, 100, 20, 30);
    let overlap = alpha(&bytes, 100, 40, 30);
    assert!((127..=128).contains(&overlap));
    assert_eq!(overlap, non_overlap);
}

#[test]
fn typed_root_viewport_clips_the_actual_paint() {
    let mut root = SceneNode::new(
        "root",
        "layer",
        SceneGeometry::Group {
            children: vec!["rect".into()],
        },
    );
    root.clip_rect = Some([20.0, 20.0, 60.0, 60.0]);
    root.extra.insert("svg_root".into(), true.into());
    let mut scene = scene_with(root);
    let mut rect = SceneNode::new(
        "rect",
        "layer",
        SceneGeometry::Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            rx: 0.0,
            ry: 0.0,
        },
    );
    rect.parent_id = Some("root".into());
    scene.nodes.insert("rect".into(), rect);
    let bytes = render_scene(&scene, [100.0, 100.0], &spec(100, 1.0)).unwrap();
    assert_eq!(alpha(&bytes, 100, 10, 50), 0);
    assert_eq!(alpha(&bytes, 100, 50, 50), 255);
    assert_eq!(alpha(&bytes, 100, 90, 50), 0);
}

#[test]
fn viewport_dpi_and_pan_have_one_pixel_conversion() {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect x="0" y="0" width="10" height="10" fill="#000"/></svg>"##;
    let mut request = spec(100, 2.0);
    request.dpi = 2.0;
    request.pan = [5.0, 10.0];
    let bytes = source_pixels(source, &request).unwrap();
    assert_eq!(alpha(&bytes, 100, 15, 25), 255);
    assert_eq!(alpha(&bytes, 100, 5, 25), 0);
    assert_eq!(alpha(&bytes, 100, 55, 25), 0);
}

#[test]
fn invalid_or_huge_raster_requests_are_rejected_before_allocation() {
    for request in [
        spec(0, 1.0),
        spec(u32::MAX, 1.0),
        spec(20, f32::NAN),
        spec(20, 0.0),
    ] {
        assert!(request.transform().is_err());
    }
    let mut overflow = spec(10, f32::MAX);
    overflow.dpi = 2.0;
    assert!(overflow.transform().is_err());
}

fn peak(source: &str, limit: usize) -> Result<usize, String> {
    let tree = usvg::Tree::from_str(source, &usvg::Options::default()).unwrap();
    surfaces::peak_with_limit(&tree, tiny_skia::Transform::identity(), [100, 100], limit)
}

#[test]
fn rectangle_clip_counts_rgba_and_alpha_surfaces_before_paint() {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
      <defs><clipPath id="c"><rect x="0" y="0" width="100" height="100"/></clipPath></defs>
      <g clip-path="url(#c)"><rect width="100" height="100"/></g></svg>"##;
    let bytes = peak(source, 1_000_000).unwrap();
    assert!(bytes >= 100 * 100 * 9);
    assert!(peak(source, bytes - 1).is_err());
}

#[test]
fn nested_surfaces_add_and_sequential_siblings_only_need_the_peak() {
    let a = r##"<g opacity="0.5"><rect width="30" height="30"/><rect x="10" width="20" height="20"/></g>"##;
    let sibling =
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">{a}{a}</svg>"#);
    let nested = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><g opacity="0.5">{a}<rect x="1" y="1" width="2" height="2"/></g></svg>"#
    );
    let siblings_peak = peak(&sibling, 1_000_000).unwrap();
    let nested_peak = peak(&nested, 1_000_000).unwrap();
    assert!(nested_peak > siblings_peak);
    assert!(peak(&nested, siblings_peak).is_err());
}

#[test]
fn actual_raster_entry_always_runs_internal_surface_guard() {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
      <g opacity="0.5"><rect x="-100000" y="-100000" width="200000" height="200000"/></g></svg>"##;
    let error = source_pixels(source, &spec(1000, 10.0)).unwrap_err();
    assert!(error.contains("内部合成表面"));
}

#[test]
fn image_resolvers_never_load_local_or_data_sources() {
    let options = options();
    assert!(
        (options.image_href_resolver.resolve_string)("not-a-user-resource.png", &options).is_none()
    );
    assert!((options.image_href_resolver.resolve_data)(
        "image/png",
        Arc::new(vec![1, 2, 3]),
        &options
    )
    .is_none());
    assert!(options.fontdb.faces().next().is_some());
}
