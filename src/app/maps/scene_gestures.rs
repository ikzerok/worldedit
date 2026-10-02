use super::*;
use worldline_core::vector_scene::{PathSegment, SceneGeometry, SceneNode, SceneOp, TextRun};

impl MapCanvas {
    pub(super) fn scene_create(&mut self, geometry: SceneGeometry, area: bool) {
        let layer_id = self.scene.active_layer.clone().or_else(|| {
            self.snapshot
                .layers
                .iter()
                .find(|layer| layer.visible && !layer.locked)
                .map(|layer| layer.id.clone())
        });
        let Some(layer_id) = layer_id else {
            self.scene.error = Some("请先选择未锁定的可见图层".into());
            return;
        };
        let mut node = SceneNode::new(self.scene_next_id("shape"), layer_id, geometry);
        node.style = self.scene.style.clone();
        if !area {
            node.style.fill = Some("none".into());
        }
        if matches!(node.geometry, SceneGeometry::Text { .. }) {
            node.style.fill = Some(format!(
                "#{:02x}{:02x}{:02x}",
                crate::theme::TEXT().r(),
                crate::theme::TEXT().g(),
                crate::theme::TEXT().b()
            ));
            node.style.stroke = Some("none".into());
            node.style.font_size = Some(24.0);
            self.scene.selection.clear();
            self.scene.selection.insert(node.id.clone());
            self.scene.inspector = Some(node);
            self.scene.inspector_dirty = true;
            self.scene.intent_baseline = self.command_baseline.clone();
        } else {
            self.scene_queue(vec![SceneOp::Insert { node, index: None }]);
        }
    }

    pub(super) fn scene_shape(&mut self, tool: CanvasTool, start: [f64; 2], end: [f64; 2]) {
        let x = start[0].min(end[0]);
        let y = start[1].min(end[1]);
        let width = (end[0] - start[0]).abs();
        let height = (end[1] - start[1]).abs();
        let geometry = if tool == CanvasTool::Ellipse {
            SceneGeometry::Ellipse {
                cx: x + width / 2.0,
                cy: y + height / 2.0,
                rx: width / 2.0,
                ry: height / 2.0,
            }
        } else {
            SceneGeometry::Rect {
                x,
                y,
                width,
                height,
                rx: 0.0,
                ry: 0.0,
            }
        };
        self.scene_create(geometry, true);
    }

    pub(super) fn scene_text_at(&mut self, position: [f64; 2]) {
        self.scene_create(
            SceneGeometry::Text {
                x: position[0],
                y: position[1],
                runs: vec![TextRun {
                    text: "文字".into(),
                    ..Default::default()
                }],
            },
            true,
        );
    }

    pub(super) fn scene_add_anchor(&mut self, to: [f64; 2], separate: bool) -> usize {
        if self.scene.path.is_empty() || separate || self.scene.new_subpath {
            if self.scene.close_path
                && !self.scene.path.is_empty()
                && !matches!(self.scene.path.last(), Some(PathSegment::Close))
            {
                self.scene.path.push(PathSegment::Close);
            }
            self.scene.path.push(PathSegment::Move { to });
            self.scene.new_subpath = false;
        } else if self.tool == CanvasTool::Bezier {
            let previous = self
                .scene
                .path
                .iter()
                .rev()
                .find_map(|segment| match segment {
                    PathSegment::Move { to }
                    | PathSegment::Line { to }
                    | PathSegment::Cubic { to, .. }
                    | PathSegment::Quadratic { to, .. }
                    | PathSegment::Arc { to, .. } => Some(*to),
                    PathSegment::Close => None,
                })
                .unwrap_or(to);
            self.scene.path.push(PathSegment::Cubic {
                control1: self.scene.outgoing.unwrap_or(previous),
                control2: to,
                to,
            });
        } else {
            self.scene.path.push(PathSegment::Line { to });
        }
        self.scene.outgoing = Some(to);
        if self.scene.intent_baseline.is_none() {
            self.scene.intent_baseline = self.command_baseline.clone();
        }
        self.scene.path.len() - 1
    }

    pub(super) fn scene_finish_path(&mut self) {
        if self.scene.path.len() < 2 {
            self.scene.error = Some("至少需要两个路径节点".into());
            return;
        }
        let mut segments = std::mem::take(&mut self.scene.path);
        let area = self.tool == CanvasTool::Polygon || self.scene.close_path;
        let geometry = match self.tool {
            CanvasTool::Polyline | CanvasTool::Polygon => {
                let points = segments
                    .iter()
                    .filter_map(|segment| match segment {
                        PathSegment::Move { to } | PathSegment::Line { to } => Some(*to),
                        _ => None,
                    })
                    .collect();
                if area {
                    SceneGeometry::Polygon { points }
                } else {
                    SceneGeometry::Polyline { points }
                }
            }
            _ => {
                if area && !matches!(segments.last(), Some(PathSegment::Close)) {
                    segments.push(PathSegment::Close);
                }
                SceneGeometry::Path { segments }
            }
        };
        self.scene.outgoing = None;
        self.scene.new_subpath = false;
        self.scene_create(geometry, area);
    }
}
