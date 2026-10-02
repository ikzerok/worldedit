use super::*;
use crate::worker_protocol::WorkTask;
use worldline_core::{svg_import, vector_scene::scene_to_safe_svg};

fn spec(scale: u32) -> RasterSpec {
    RasterSpec {
        width: 100 * scale,
        height: 100 * scale,
        zoom: scale as f32,
        pan: [0.0, 0.0],
        dpi: 1.0,
    }
}
fn alpha(bytes: &[u8], width: usize, x: usize, y: usize) -> u8 {
    bytes[(y * width + x) * 4 + 3]
}
fn preview(body: &str) -> worldline_core::vector_scene::SvgScenePreview {
    svg_import::preview_scene(&format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='100'>{body}</svg>"
    ))
    .unwrap()
}

#[test]
fn dash_native_pixels_have_real_gaps_at_multiple_zoom_and_dpi_scales() {
    let preview = preview("<path fill='none' stroke='blue' stroke-width='4' stroke-dasharray='15 12' d='M0 20L90 20M0 40L90 40'/>");
    for scale in [1, 2, 4] {
        let request = spec(scale);
        let bytes = render_scene(&preview.scene, [100.0, 100.0], &request).unwrap();
        for y in [20, 40] {
            for (x, expected) in [(5, 255), (20, 0), (30, 255), (48, 0), (60, 255)] {
                assert_eq!(
                    alpha(
                        &bytes,
                        request.width as usize,
                        x * scale as usize,
                        y * scale as usize
                    ),
                    expected
                );
            }
        }
        let mut hidpi = spec(1);
        hidpi.width = 100 * scale;
        hidpi.height = 100 * scale;
        hidpi.dpi = scale as f32;
        assert_eq!(
            render_scene(&preview.scene, [100.0, 100.0], &hidpi).unwrap(),
            bytes
        );
    }
}

#[test]
fn dash_source_typed_scene_reimport_and_worker_transport_produce_the_same_pixels() {
    let cases = [
        "<g fill='none' stroke='blue' stroke-width='2' stroke-dasharray='7 4 2' stroke-dashoffset='-3'><path d='M5 10C20 0 25 40 40 20M5 60Q30 30 60 60A15 10 20 0 1 80 80'/></g>",
        "<g stroke='red' fill='none' stroke-dasharray='0 8' stroke-linecap='round' stroke-width='4' transform='matrix(.9 .1 -.1 .8 8 3)' opacity='.5'><path d='M5 15L80 15'/><path d='M5 15L80 15'/><path stroke-dasharray='none' d='M5 40L80 40'/></g>",
        "<g stroke='blue' stroke-width='3' stroke-dasharray='5 3' stroke-dashoffset='5'><rect x='10' y='10' width='30' height='20' rx='3' fill='none'/><ellipse cx='70' cy='25' rx='15' ry='10' fill='none'/><path stroke-dasharray='0 0 0' d='M10 70L80 70'/></g>",
        "<g stroke='blue' fill='none' stroke-dasharray='4 0 0 5' stroke-dashoffset='-11'><path d='M10 10L60 10L60 40Z'/><path stroke='red' d='M20 5L20 50'/></g>",
    ];
    for body in cases {
        let original = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='100'>{body}</svg>"
        );
        let preview = svg_import::preview_scene(&original).unwrap();
        let exported = scene_to_safe_svg(&preview.scene, 100.0, 100.0).unwrap();
        let again = svg_import::preview_scene(&exported).unwrap();
        for scale in [1, 2, 4] {
            let request = spec(scale);
            let expected =
                raster_safe_source(&original, &request, request.transform().unwrap()).unwrap();
            let typed = render_scene(&preview.scene, [100.0, 100.0], &request).unwrap();
            assert_eq!(typed, expected, "source projection scale={scale}: {body}");
            assert_eq!(
                render_scene(&again.scene, [100.0, 100.0], &request).unwrap(),
                typed
            );
            let task = WorkTask::RenderScene {
                scene: preview.scene.clone(),
                extent: [100.0, 100.0],
                spec: request.clone(),
            };
            let WorkTask::RenderScene {
                scene,
                extent,
                spec,
            } = serde_json::from_slice::<WorkTask>(&serde_json::to_vec(&task).unwrap()).unwrap()
            else {
                panic!("render task expected")
            };
            assert_eq!(render_scene(&scene, extent, &spec).unwrap(), typed);
        }
    }
}

#[test]
fn dash_phase_zero_caps_and_subpath_restart_have_expected_pixels() {
    for (offset, painted, gap) in [("5", 5, 15), ("-5", 10, 2)] {
        let scene = preview(&format!("<path fill='none' stroke='blue' stroke-width='4' stroke-dasharray='15 12' stroke-dashoffset='{offset}' d='M0 20L90 20M0 40L90 40'/>")).scene;
        let bytes = render_scene(&scene, [100.0, 100.0], &spec(1)).unwrap();
        for y in [20, 40] {
            assert_eq!(alpha(&bytes, 100, painted, y), 255);
            assert_eq!(alpha(&bytes, 100, gap, y), 0);
        }
    }
    let scene = preview("<line x1='10' y1='30' x2='90' y2='30' stroke='blue' stroke-width='4' stroke-linecap='round' stroke-dasharray='0 8'/>").scene;
    let bytes = render_scene(&scene, [100.0, 100.0], &spec(1)).unwrap();
    assert!(alpha(&bytes, 100, 10, 30) > 200);
    assert_eq!(alpha(&bytes, 100, 14, 30), 0);
    assert!(alpha(&bytes, 100, 18, 30) > 200);
}

#[test]
fn dash_raster_rejects_tiny_period_and_missing_feature_before_paint() {
    let mut scene =
        preview("<line id='route' x2='90' stroke='blue' stroke-dasharray='15 12'/>").scene;
    scene.nodes.get_mut("route").unwrap().style.stroke_dasharray = Some(vec![0.00001, 0.00001]);
    let error = render_scene(&scene, [100.0, 100.0], &spec(1)).unwrap_err();
    assert!(error.contains("SCENE_LIMIT"));
    scene.nodes.get_mut("route").unwrap().style.stroke_dasharray = Some(vec![15.0, 12.0]);
    scene.extra.remove("required_features");
    let error = render_scene(&scene, [100.0, 100.0], &spec(1)).unwrap_err();
    assert!(error.contains("SCENE_FEATURE"));
}

#[test]
fn dash_safe_large_arc_backend_bounds_match_zero_rotation_for_both_huge_signs() {
    let safe = |angle| {
        let source = format!("<svg width='200' height='100'><path fill='none' stroke='blue' stroke-dasharray='10000 10000' d='M0 0A100000000 .0000100001 {angle} 0 1 200000000 0'/></svg>");
        let preview = svg_import::preview_scene(&source).unwrap();
        scene_to_safe_svg(&preview.scene, 200.0, 100.0).unwrap()
    };
    let canonical = safe(0);
    // 只交给后端已通过 core、已规范化的输出；危险原 SVG 从不进入渲染库。
    let expected = usvg::Tree::from_str(&canonical, &options())
        .unwrap()
        .root()
        .abs_bounding_box();
    assert!(expected.width().is_finite() && expected.width() <= 300_000_000.0);
    assert!(expected.height().is_finite() && expected.height() <= 10.0);
    for angle in [999999720, -999999720] {
        let output = safe(angle);
        assert_eq!(output, canonical);
        let tree = usvg::Tree::from_str(&output, &options()).unwrap();
        assert_eq!(tree.root().abs_bounding_box(), expected);
    }
}
