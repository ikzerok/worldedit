use super::*;
use worldline_core::vector_scene::{Affine, ScenePrimitive};

impl MapCanvas {
    pub(super) fn scene_view_transform(&self) -> Option<Affine> {
        self.scene.source.as_ref()?;
        Some(self.scene.canvas_transform)
    }

    pub(super) fn scene_to_screen(&self, point: [f64; 2]) -> Option<Pos2> {
        let point = self.scene_view_transform()?.point(point);
        Some(self.camera.normalized_to_screen(
            Pos2::new(
                (point[0] / self.snapshot.canvas.width as f64) as f32,
                (point[1] / self.snapshot.canvas.height as f64) as f32,
            ),
            self.viewport,
        ))
    }

    pub(super) fn screen_to_scene(&self, screen: Pos2) -> Option<[f64; 2]> {
        let normalized = self.camera.screen_to_normalized(screen, self.viewport);
        Some(self.scene_view_transform()?.inverse()?.point([
            normalized.x as f64 * self.snapshot.canvas.width as f64,
            normalized.y as f64 * self.snapshot.canvas.height as f64,
        ]))
    }

    pub(super) fn scene_hit(&self, screen: Pos2) -> Option<String> {
        let point = self.screen_to_scene(screen)?;
        let scene = self.scene.source.as_ref()?;
        let normalized = self.camera.screen_to_normalized(screen, self.viewport);
        let legacy_layer = self
            .hit_test(NormalizedPoint::new(normalized.x, normalized.y), 10.0)
            .and_then(|(id, _)| self.placement_layer(&id).map(|(layer, _, _)| layer));
        for layer in self
            .snapshot
            .layers
            .iter()
            .rev()
            .filter(|layer| layer.visible)
        {
            for index in self
                .scene
                .primitive_layers
                .get(&layer.id)
                .into_iter()
                .flatten()
                .rev()
            {
                let primitive = &self.scene.primitives[*index];
                if scene
                    .nodes
                    .get(&primitive.node_id)
                    .is_none_or(|node| node.layer_id != layer.id)
                    || !inside_clips(primitive, point)
                {
                    continue;
                }
                let paths: Vec<Vec<Pos2>> = primitive
                    .paths
                    .iter()
                    .map(|path| {
                        path.iter()
                            .filter_map(|point| self.scene_to_screen(*point))
                            .collect()
                    })
                    .collect();
                let mut edge_hit = false;
                for (index, path) in paths.iter().enumerate() {
                    if path.len() == 1 && path[0].distance(screen) <= 10.0 {
                        edge_hit = true;
                    }
                    for pair in path.windows(2) {
                        edge_hit |= segment_distance(screen, pair[0], pair[1]) <= 8.0;
                    }
                    if primitive.closed.get(index) == Some(&true) && path.len() > 1 {
                        edge_hit |= segment_distance(screen, path[0], path[path.len() - 1]) <= 8.0;
                    }
                }
                let winding: i32 = paths.iter().map(|path| winding_number(path, screen)).sum();
                let filled = primitive.style.fill.as_deref() != Some("none")
                    && if primitive.style.fill_rule.as_deref() == Some("evenodd") {
                        winding.abs() % 2 == 1
                    } else {
                        winding != 0
                    };
                let text_hit = !primitive.text.is_empty()
                    && self
                        .primitive_screen_bounds(primitive)
                        .is_some_and(|bounds| bounds.contains(screen));
                if edge_hit || filled || text_hit {
                    return Some(primitive.node_id.clone());
                }
            }
            if legacy_layer.as_ref() == Some(&layer.id) {
                return None;
            }
        }
        None
    }

    pub(super) fn primitive_screen_bounds(&self, primitive: &ScenePrimitive) -> Option<Rect> {
        self.bounds_to_screen(clipped_bounds(primitive)?)
    }

    pub(super) fn node_screen_bounds(&self, id: &str) -> Option<Rect> {
        self.bounds_to_screen(*self.scene.bounds.get(id)?)
    }

    fn bounds_to_screen(&self, [x0, y0, x1, y1]: [f64; 4]) -> Option<Rect> {
        let points = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];
        let mut result = Rect::NOTHING;
        for point in points {
            result.extend_with(self.scene_to_screen(point)?);
        }
        Some(result)
    }

    pub(super) fn select_scene(&mut self, id: &str, extend: bool) -> bool {
        let Some(node) = self
            .scene
            .source
            .as_ref()
            .and_then(|scene| scene.nodes.get(id))
        else {
            return false;
        };
        if self.scene.inspector_dirty && !self.scene.selection.contains(id) {
            self.scene.error = Some("请先应用或取消当前对象检查器输入".into());
            return false;
        }
        if !extend {
            self.scene.selection.clear();
        }
        if extend && self.scene.selection.contains(id) {
            self.scene.selection.remove(id);
        } else {
            self.scene.selection.insert(id.into());
        }
        if !self.scene.inspector_dirty {
            self.scene.inspector = Some(node.clone());
        }
        self.selected = None;
        true
    }
}

pub(super) fn clipped_bounds(primitive: &ScenePrimitive) -> Option<[f64; 4]> {
    let mut bounds = primitive.bounds;
    for clip in &primitive.clips {
        let [x, y, w, h] = clip.rect;
        let corners =
            [[x, y], [x + w, y], [x + w, y + h], [x, y + h]].map(|p| clip.transform.point(p));
        let mut rectangle = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        for [x, y] in corners {
            rectangle[0] = rectangle[0].min(x);
            rectangle[1] = rectangle[1].min(y);
            rectangle[2] = rectangle[2].max(x);
            rectangle[3] = rectangle[3].max(y);
        }
        bounds[0] = bounds[0].max(rectangle[0]);
        bounds[1] = bounds[1].max(rectangle[1]);
        bounds[2] = bounds[2].min(rectangle[2]);
        bounds[3] = bounds[3].min(rectangle[3]);
        if bounds[0] > bounds[2] || bounds[1] > bounds[3] {
            return None;
        }
    }
    Some(bounds)
}

pub(super) fn inside_clips(primitive: &ScenePrimitive, point: [f64; 2]) -> bool {
    primitive.clips.iter().all(|clip| {
        let Some(inverse) = clip.transform.inverse() else {
            return false;
        };
        let [x, y] = inverse.point(point);
        let [left, top, width, height] = clip.rect;
        x >= left && x <= left + width && y >= top && y <= top + height
    })
}

fn segment_distance(point: Pos2, first: Pos2, second: Pos2) -> f32 {
    let span = second - first;
    let length = span.length_sq();
    if length <= f32::EPSILON {
        return point.distance(first);
    }
    let t = ((point - first).dot(span) / length).clamp(0.0, 1.0);
    point.distance(first + span * t)
}

fn winding_number(path: &[Pos2], point: Pos2) -> i32 {
    if path.len() < 3 {
        return 0;
    }
    let mut winding = 0;
    for (a, b) in path
        .iter()
        .zip(path.iter().cycle().skip(1))
        .take(path.len())
    {
        let side = (b.x - a.x) * (point.y - a.y) - (point.x - a.x) * (b.y - a.y);
        if a.y <= point.y && b.y > point.y && side > 0.0 {
            winding += 1;
        }
        if a.y > point.y && b.y <= point.y && side < 0.0 {
            winding -= 1;
        }
    }
    winding
}
