use super::*;

impl MapCanvas {
    pub(super) fn focus_scene(&mut self, id: &str) {
        let Some(scene) = &self.scene.source else {
            return;
        };
        let mut screen = Rect::NOTHING;
        for primitive in &self.scene.primitives {
            let mut node = Some(primitive.node_id.as_str());
            let mut belongs = false;
            while let Some(current) = node {
                if current == id {
                    belongs = true;
                    break;
                }
                node = scene
                    .nodes
                    .get(current)
                    .and_then(|node| node.parent_id.as_deref());
            }
            if belongs {
                if let Some(bounds) = self.primitive_screen_bounds(primitive) {
                    screen = screen.union(bounds);
                }
            }
        }
        if screen.is_finite() && self.viewport.is_positive() {
            self.camera.pan_by(self.viewport.center() - screen.center());
            self.fit_pending = false;
        }
    }

    pub(super) fn reveal_scene_node_for_session(&mut self, id: &str) {
        if let Some(scene) = &self.scene.source {
            let mut current = Some(id);
            while let Some(id) = current {
                self.scene.revealed.insert(id.to_owned());
                current = scene
                    .nodes
                    .get(id)
                    .and_then(|node| node.parent_id.as_deref());
            }
        }
        let source = self.scene.source.clone();
        self.scene.sync(source.as_ref());
    }

    pub(super) fn focus_legacy(&mut self, geometry: &MapGeometry) {
        let points: Vec<_> = match geometry {
            MapGeometry::Point(point)
            | MapGeometry::Text {
                position: point, ..
            } => vec![point.as_pos2()],
            MapGeometry::Polyline(points) | MapGeometry::Polygon(points) => {
                points.iter().map(|point| point.as_pos2()).collect()
            }
        };
        let mut bounds = Rect::NOTHING;
        for point in points {
            bounds.extend_with(self.camera.normalized_to_screen(point, self.viewport));
        }
        if bounds.is_finite() && self.viewport.is_positive() {
            self.camera.pan_by(self.viewport.center() - bounds.center());
            self.fit_pending = false;
        }
    }
}
