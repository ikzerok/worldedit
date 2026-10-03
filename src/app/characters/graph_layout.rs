//! 画布几何不解释关系语义；自动视角与作者调整分开。
use super::state::CharacterFocus;
use egui::{Pos2, Rect, Vec2};
use std::collections::BTreeMap;
pub(super) const SELECTED_SIZE: Vec2 = Vec2::new(200.0, 76.0);
pub(super) const NODE_SIZE: Vec2 = Vec2::new(168.0, 62.0);

impl CharacterFocus {
    pub fn update_graph_layout(&mut self, keys: &[String], selected: Option<&str>, size: Vec2) {
        let resized = self
            .layout_size
            .is_none_or(|old| (old - size).length() > 1.0);
        let changed = self.layout_members != keys;
        if self.positions.is_empty()
            || (!self.layout_manual && (changed || (resized && self.auto_fit)))
        {
            self.positions = default_positions(keys, selected, size);
            self.fit = true;
        } else if changed {
            for (key, point) in default_positions(keys, selected, size) {
                self.positions.entry(key).or_insert(point);
            }
        }
        if resized && self.auto_fit {
            self.fit = true;
        }
        self.layout_size = Some(size);
        self.layout_members = keys.to_vec();
    }
    pub fn fit_graph(&mut self, points: &[([f64; 2], bool)], size: [f64; 2]) {
        if points.is_empty() {
            return;
        }
        let bounds = |zoom: f64| {
            let mut min = [f64::INFINITY; 2];
            let mut max = [f64::NEG_INFINITY; 2];
            for (point, selected) in points {
                let half = if *selected {
                    [100.0, 38.0]
                } else {
                    [84.0 * zoom, 31.0 * zoom]
                };
                for axis in 0..2 {
                    min[axis] = min[axis].min(point[axis] * zoom - half[axis]);
                    max[axis] = max[axis].max(point[axis] * zoom + half[axis]);
                }
            }
            (min, max)
        };
        let (mut low, mut high) = (0.001, 1.0);
        for _ in 0..36 {
            let middle = (low + high) * 0.5;
            let (min, max) = bounds(middle);
            if max[0] - min[0] <= size[0] - 24.0 && max[1] - min[1] <= size[1] - 24.0 {
                low = middle;
            } else {
                high = middle;
            }
        }
        self.camera.zoom = low;
        let (min, max) = bounds(low);
        self.camera.pan = [-(min[0] + max[0]) * 0.5, -(min[1] + max[1]) * 0.5];
    }
}
fn default_positions(
    keys: &[String],
    selected: Option<&str>,
    size: Vec2,
) -> BTreeMap<String, [f64; 2]> {
    let count = keys
        .iter()
        .filter(|key| Some(key.as_str()) != selected)
        .count()
        .max(1);
    let radius = (count as f64 * 32.0).max(245.0);
    let aspect = (size.x as f64 / size.y.max(1.0) as f64).clamp(0.45, 6.0);
    let mut index = 0;
    keys.iter()
        .map(|key| {
            let point = if Some(key.as_str()) == selected {
                [0.0, 0.0]
            } else {
                let angle = index as f64 / count as f64 * std::f64::consts::TAU;
                index += 1;
                [angle.cos() * radius * aspect, angle.sin() * radius]
            };
            (key.clone(), point)
        })
        .collect()
}
/// 标签保留并就近绕开卡片；固定候选上限避免在密图中无限搜索。
pub(super) fn edge_label_rect(
    ideal: Pos2,
    size: Vec2,
    canvas: Rect,
    nodes: &BTreeMap<String, Rect>,
    labels: &[Rect],
) -> Rect {
    let inset = size * 0.5 + Vec2::splat(3.0);
    let clamp = |p: Pos2| {
        egui::pos2(
            p.x.clamp(canvas.left() + inset.x, canvas.right() - inset.x),
            p.y.clamp(canvas.top() + inset.y, canvas.bottom() - inset.y),
        )
    };
    let clear = |rect: Rect| {
        nodes
            .values()
            .chain(labels)
            .all(|node| !node.expand(3.0).intersects(rect))
    };
    let initial = Rect::from_center_size(clamp(ideal), size);
    if clear(initial) {
        return initial;
    }
    for radius in 1..=8 {
        for (x, y) in [
            (0, -1),
            (0, 1),
            (-1, 0),
            (1, 0),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let point = clamp(ideal + Vec2::new(x as f32, y as f32) * (radius as f32 * 20.0));
            let candidate = Rect::from_center_size(point, size);
            if clear(candidate) {
                return candidate;
            }
        }
    }
    initial
}
