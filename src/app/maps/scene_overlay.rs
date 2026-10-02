use super::scene_input::SceneGesture;
use super::*;
use worldline_core::vector_scene::{node_state, project_scene, MapScene, SceneGeometry, SceneNode};

impl MapCanvas {
    pub(super) fn draw_scene_overlay(&self, painter: &egui::Painter) {
        let Some(scene) = &self.scene.source else {
            return;
        };
        let painter = painter.with_clip_rect(self.viewport);
        let stroke = egui::Stroke::new(1.5_f32, crate::theme::ACCENT());
        for id in &self.scene.selection {
            if scene.nodes.get(id).is_some_and(|node| {
                !self
                    .snapshot
                    .layers
                    .iter()
                    .any(|layer| layer.id == node.layer_id && layer.visible)
            }) {
                continue;
            }
            if let Some(rect) = self.node_screen_bounds(id) {
                painter.rect_stroke(rect.expand(3.0), 0.0, stroke, egui::StrokeKind::Outside);
            }
        }
        for id in &self.scene.selection {
            if self.tool != CanvasTool::Nodes {
                break;
            }
            let Some(node) = scene.nodes.get(id) else {
                continue;
            };
            let Ok(state) = node_state(scene, id) else {
                continue;
            };
            if (!state.visible && !self.scene.revealed.contains(id))
                || !self
                    .snapshot
                    .layers
                    .iter()
                    .any(|layer| layer.id == node.layer_id && layer.visible)
            {
                continue;
            }
            if self.tool != CanvasTool::Nodes {
                continue;
            }
            self.paint_control_arms(&painter, &node.geometry, state.transform);
            for handle in super::scene_handles::handles(&node.geometry) {
                if let Some(screen) = self.scene_to_screen(state.transform.point(handle.point)) {
                    let color = if handle.control {
                        crate::theme::GOLD()
                    } else {
                        crate::theme::ACCENT()
                    };
                    painter.circle_filled(
                        screen,
                        if handle.control { 4.0 } else { 5.0 },
                        crate::theme::PANEL(),
                    );
                    painter.circle_stroke(screen, 5.0, egui::Stroke::new(1.5_f32, color));
                }
            }
        }
        if !self.scene.path.is_empty() {
            self.paint_draft_geometry(
                &painter,
                SceneGeometry::Path {
                    segments: self.scene.path.clone(),
                },
            );
        }
        if let Some(gesture) = &self.scene.gesture {
            match gesture {
                SceneGesture::Shape {
                    tool,
                    start,
                    current,
                } => {
                    if let (Some(a), Some(b)) =
                        (self.scene_to_screen(*start), self.scene_to_screen(*current))
                    {
                        let rect = Rect::from_two_pos(a, b);
                        if *tool == CanvasTool::Ellipse {
                            painter.add(egui::Shape::ellipse_stroke(
                                rect.center(),
                                rect.size() * 0.5,
                                stroke,
                            ));
                        } else {
                            painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
                        }
                    }
                }
                SceneGesture::Marquee { start, current } => {
                    let rect = Rect::from_two_pos(*start, *current);
                    painter.rect_filled(rect, 0.0, crate::theme::ACCENT().gamma_multiply(0.12));
                    painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);
                }
                SceneGesture::Move {
                    start,
                    current,
                    nodes,
                } => {
                    if let (Some(a), Some(b)) =
                        (self.scene_to_screen(*start), self.scene_to_screen(*current))
                    {
                        let delta = b - a;
                        for (node, _) in nodes {
                            if let Some(rect) = self.node_screen_bounds(&node.id) {
                                painter.rect_stroke(
                                    rect.translate(delta),
                                    0.0,
                                    stroke,
                                    egui::StrokeKind::Outside,
                                );
                            }
                        }
                    }
                }
                SceneGesture::Handle {
                    node,
                    kind,
                    inverse,
                    current,
                } => {
                    let mut geometry = node.geometry.clone();
                    if super::scene_handles::move_handle(
                        &mut geometry,
                        *kind,
                        inverse.point(*current),
                    ) {
                        let mut draft = (**node).clone();
                        draft.geometry = geometry;
                        if let Ok(state) = node_state(scene, &node.id) {
                            draft.transform = state.transform;
                        }
                        self.paint_draft_node(&painter, draft);
                    }
                }
                SceneGesture::Pen { anchor, .. } => {
                    if let Some(outgoing) = self.scene.outgoing {
                        if let (Some(a), Some(b)) = (
                            self.scene_to_screen(*anchor),
                            self.scene_to_screen(outgoing),
                        ) {
                            painter.line_segment(
                                [a, b],
                                egui::Stroke::new(1.0_f32, crate::theme::GOLD()),
                            );
                            painter.circle_filled(b, 4.0, crate::theme::GOLD());
                        }
                    }
                }
            }
        }
    }

    fn paint_draft_geometry(&self, painter: &egui::Painter, geometry: SceneGeometry) {
        self.paint_draft_node(painter, SceneNode::new("draft", "draft", geometry));
    }

    fn paint_control_arms(
        &self,
        painter: &egui::Painter,
        geometry: &SceneGeometry,
        transform: worldline_core::vector_scene::Affine,
    ) {
        use worldline_core::vector_scene::PathSegment;
        let draw = |a, b| {
            if let (Some(a), Some(b)) = (
                self.scene_to_screen(transform.point(a)),
                self.scene_to_screen(transform.point(b)),
            ) {
                painter.line_segment([a, b], egui::Stroke::new(1.0_f32, crate::theme::GOLD()));
            }
        };
        if let SceneGeometry::Ellipse { cx, cy, rx, ry } = geometry {
            draw([*cx, *cy], [cx + rx, *cy]);
            draw([*cx, *cy], [*cx, cy + ry]);
        }
        if let SceneGeometry::Path { segments } = geometry {
            let mut previous = [0.0, 0.0];
            let mut start = previous;
            for segment in segments {
                match segment {
                    PathSegment::Move { to } => {
                        previous = *to;
                        start = *to;
                    }
                    PathSegment::Cubic {
                        control1,
                        control2,
                        to,
                    } => {
                        draw(previous, *control1);
                        draw(*to, *control2);
                        previous = *to;
                    }
                    PathSegment::Quadratic { control, to } => {
                        draw(previous, *control);
                        draw(*to, *control);
                        previous = *to;
                    }
                    PathSegment::Line { to } | PathSegment::Arc { to, .. } => previous = *to,
                    PathSegment::Close => previous = start,
                }
            }
        }
    }

    fn paint_draft_node(&self, painter: &egui::Painter, mut node: SceneNode) {
        let Some(source) = &self.scene.source else {
            return;
        };
        let mut scene = MapScene::new(source.view_box[2], source.view_box[3]);
        scene.view_box = source.view_box;
        // 仅临时金色几何导线；声明可解释的样式能力，不修改原稿或开启持久 scene。
        scene.extra.insert(
            "required_features".into(),
            serde_json::json!([worldline_core::vector_scene::SCENE_DASH_FEATURE]),
        );
        node.id = "draft".into();
        node.layer_id = "draft".into();
        node.parent_id = None;
        scene
            .root_order
            .insert("draft".into(), vec!["draft".into()]);
        scene.nodes.insert("draft".into(), node);
        if let Ok(primitives) = project_scene(&scene, 0.1) {
            for primitive in primitives {
                for (index, path) in primitive.paths.iter().enumerate() {
                    let mut points: Vec<_> = path
                        .iter()
                        .filter_map(|point| self.scene_to_screen(*point))
                        .collect();
                    if primitive.closed.get(index) == Some(&true) && !points.is_empty() {
                        points.push(points[0]);
                    }
                    if points.len() > 1 {
                        painter.add(egui::Shape::line(
                            points,
                            egui::Stroke::new(2.0_f32, crate::theme::GOLD()),
                        ));
                    } else if let Some(point) = points.first() {
                        painter.circle_filled(*point, 4.0, crate::theme::GOLD());
                    }
                }
            }
        }
    }
}
