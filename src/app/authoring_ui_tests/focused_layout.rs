//! 来自真实native 730×220短宽画布及1188→1040缩窗缺陷的回归。
use super::focused_workbench::{focus_app, work_frame};
use super::*;
fn node_rects(app: &WorldeditApp) -> Vec<Rect> {
    let canvas = app.character_focus.canvas.unwrap();
    let zoom = app.character_focus.camera.zoom as f32;
    let selected = app
        .character_editor
        .as_ref()
        .and_then(|e| e.original.as_deref());
    app.character_focus
        .result
        .as_ref()
        .unwrap()
        .nodes
        .iter()
        .filter_map(|node| {
            let point = app
                .character_focus
                .positions
                .get(&format!("{}:{}", node.target.kind, node.target.id))?;
            let point = app
                .character_focus
                .camera
                .world_to_canvas(*point, [canvas.width() as f64, canvas.height() as f64]);
            let size =
                if node.target.kind == "character" && Some(node.target.id.as_str()) == selected {
                    vec2(200.0, 76.0) * zoom.max(1.0)
                } else {
                    vec2(168.0, 62.0) * zoom
                };
            Some(Rect::from_center_size(
                canvas.min + vec2(point[0] as f32, point[1] as f32),
                size,
            ))
        })
        .collect()
}
fn text_shapes<'a>(shape: &'a egui::Shape, out: &mut Vec<&'a egui::epaint::TextShape>) {
    match shape {
        egui::Shape::Text(text) => out.push(text),
        egui::Shape::Vec(shapes) => {
            for s in shapes {
                text_shapes(s, out);
            }
        }
        _ => {}
    }
}
#[test]
fn short_wide_native_canvas_uses_width_and_keeps_labels_clear_of_cards() {
    let (ctx, mut app) = focus_app();
    work_frame(&ctx, &mut app, vec2(1188.0, 848.0), vec![]);
    app.character_focus.show_results = false;
    let out = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1188.0, 848.0))),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.allocate_ui(vec2(730.0, 220.0), |ui| app.character_context_graph(ui));
            });
        },
    );
    let canvas = app.character_focus.canvas.unwrap();
    let cards = node_rects(&app);
    assert!(
        (canvas.width() - 730.0).abs() < 1.0 && (canvas.height() - 220.0).abs() < 1.0,
        "{canvas:?}"
    );
    assert!(
        cards.iter().all(|rect| canvas.contains_rect(*rect)),
        "{cards:?} in {canvas:?}"
    );
    let left = cards
        .iter()
        .map(|r| r.center().x)
        .fold(f32::INFINITY, f32::min);
    let right = cards
        .iter()
        .map(|r| r.center().x)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        right - left > canvas.width() * 0.6,
        "横向中心跨度 {left}..{right}"
    );
    let mut texts = Vec::new();
    for clipped in &out.shapes {
        text_shapes(&clipped.shape, &mut texts);
    }
    let selected = texts
        .iter()
        .filter(|text| text.galley.job.text == "林栖")
        .collect::<Vec<_>>();
    assert!(!selected.is_empty());
    assert!(selected
        .iter()
        .all(|text| text
            .galley
            .job
            .sections
            .iter()
            .all(|s| s.format.font_id.size >= 14.0)));
    let labels = texts
        .iter()
        .filter(|text| {
            text.galley.job.text.starts_with("属性引用·")
                || text.galley.job.text.starts_with("事件参与·")
        })
        .collect::<Vec<_>>();
    assert_eq!(labels.len(), 2, "所有边仍有标签");
    for label in labels {
        let rect = label.galley.rect.translate(label.pos.to_vec2());
        assert!(
            cards.iter().all(|card| !card.intersects(rect)),
            "label {:?} overlaps {cards:?}",
            label.galley.job.text
        );
    }
}
#[test]
fn default_auto_fit_tracks_native_resize_but_manual_camera_and_nodes_remain() {
    let (ctx, mut app) = focus_app();
    for size in [
        vec2(1188.0, 848.0),
        vec2(1040.0, 660.0),
        vec2(1400.0, 900.0),
    ] {
        for _ in 0..3 {
            work_frame(&ctx, &mut app, size, vec![]);
        }
        let canvas = app.character_focus.canvas.unwrap();
        assert!(
            node_rects(&app)
                .iter()
                .all(|rect| canvas.contains_rect(*rect)),
            "size {size:?}"
        );
        assert!(app.character_focus.auto_fit);
    }
    app.character_focus.auto_fit = false;
    app.character_focus.layout_manual = true;
    app.character_focus
        .positions
        .insert("character:linqi".into(), [123.0, -17.0]);
    app.character_focus.camera.zoom = 0.82;
    app.character_focus.camera.pan = [31.0, 19.0];
    let positions = app.character_focus.positions.clone();
    let camera = app.character_focus.camera.clone();
    for _ in 0..3 {
        work_frame(&ctx, &mut app, vec2(1040.0, 660.0), vec![]);
    }
    assert_eq!(app.character_focus.positions, positions);
    assert_eq!(app.character_focus.camera, camera);
}
