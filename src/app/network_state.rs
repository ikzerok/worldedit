//! 局部网络的个人浏览状态；只消费 core 查询，不修改 Project 或作者文件。
use std::collections::{BTreeMap, BTreeSet};
use worldline_core::catalog::{Catalog, TargetRef};
use worldline_core::graph_views::{position_key, GraphViewDraft, GraphViewFilters};
use worldline_core::RelationQueryResult;

#[derive(Clone, Debug, PartialEq)]
pub struct GraphCamera {
    pub zoom: f64,
    pub pan: [f64; 2],
}
impl Default for GraphCamera {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: [0.0, 0.0],
        }
    }
}
impl GraphCamera {
    pub fn canvas_to_world(&self, point: [f64; 2], size: [f64; 2]) -> [f64; 2] {
        [
            (point[0] - size[0] / 2.0 - self.pan[0]) / self.zoom,
            (point[1] - size[1] / 2.0 - self.pan[1]) / self.zoom,
        ]
    }
    pub fn world_to_canvas(&self, point: [f64; 2], size: [f64; 2]) -> [f64; 2] {
        [
            size[0] / 2.0 + self.pan[0] + point[0] * self.zoom,
            size[1] / 2.0 + self.pan[1] + point[1] * self.zoom,
        ]
    }
    pub fn zoom_at(&mut self, factor: f64, cursor: [f64; 2], size: [f64; 2]) {
        if !factor.is_finite()
            || factor <= 0.0
            || !cursor.iter().chain(size.iter()).all(|x| x.is_finite())
        {
            return;
        }
        let anchor = self.canvas_to_world(cursor, size);
        let zoom = (self.zoom * factor).clamp(0.05, 6.0);
        let pan = [
            cursor[0] - size[0] / 2.0 - anchor[0] * zoom,
            cursor[1] - size[1] / 2.0 - anchor[1] * zoom,
        ];
        if pan.iter().all(|x| x.is_finite()) {
            self.zoom = zoom;
            self.pan = pan;
        }
    }
    pub fn pan_by(&mut self, delta: [f64; 2]) {
        let pan = [self.pan[0] + delta[0], self.pan[1] + delta[1]];
        if pan.iter().all(|x| x.is_finite()) {
            self.pan = pan;
        }
    }
    pub fn fit(&mut self, points: &[[f64; 2]], size: [f64; 2]) {
        if points.is_empty() || !size.iter().all(|x| x.is_finite() && *x > 0.0) {
            return;
        }
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for point in points
            .iter()
            .filter(|point| point.iter().all(|x| x.is_finite()))
        {
            for axis in 0..2 {
                min[axis] = min[axis].min(point[axis]);
                max[axis] = max[axis].max(point[axis]);
            }
        }
        let extent = [max[0] - min[0], max[1] - min[1]];
        if !extent.iter().all(|x| x.is_finite()) {
            return;
        }
        let zoom = ((size[0] - 160.0).max(20.0) / extent[0].max(1.0))
            .min((size[1] - 120.0).max(20.0) / extent[1].max(1.0))
            .clamp(0.05, 2.0);
        let pan = [
            -(min[0] / 2.0 + max[0] / 2.0) * zoom,
            -(min[1] / 2.0 + max[1] / 2.0) * zoom,
        ];
        if pan.iter().all(|x| x.is_finite()) {
            self.zoom = zoom;
            self.pan = pan;
        }
    }
}

#[derive(Clone)]
struct LocalView {
    focus: Option<TargetRef>,
    filters: GraphViewFilters,
    positions: BTreeMap<String, [f64; 2]>,
    hidden: BTreeSet<String>,
    camera: GraphCamera,
    offset: usize,
    pages: Vec<usize>,
}
#[derive(Default)]
pub struct NetworkState {
    pub focus: Option<TargetRef>,
    pub filters: GraphViewFilters,
    pub result: Option<RelationQueryResult>,
    pub positions: BTreeMap<String, [f64; 2]>,
    pub hidden: BTreeSet<String>,
    pub camera: GraphCamera,
    pub offset: usize,
    pages: Vec<usize>,
    cache: Option<(u64, TargetRef, GraphViewFilters, usize)>,
    drag: Option<(String, [f64; 2])>,
    history: Vec<LocalView>,
}
impl NetworkState {
    pub fn set_focus(&mut self, target: TargetRef) {
        self.focus = Some(target);
        self.filters = GraphViewFilters::default();
        self.result = None;
        self.positions.clear();
        self.hidden.clear();
        self.camera = GraphCamera::default();
        self.offset = 0;
        self.pages.clear();
        self.cache = None;
        self.drag = None;
    }
    pub fn enter(&mut self, target: TargetRef) {
        if self.focus.as_ref() == Some(&target) {
            return;
        }
        if self.focus.is_some() {
            if self.history.len() == 64 {
                self.history.remove(0);
            }
            self.history.push(LocalView {
                focus: self.focus.clone(),
                filters: self.filters.clone(),
                positions: self.positions.clone(),
                hidden: self.hidden.clone(),
                camera: self.camera.clone(),
                offset: self.offset,
                pages: self.pages.clone(),
            });
        }
        self.set_focus(target);
    }
    pub fn can_back(&self) -> bool {
        !self.history.is_empty()
    }
    pub fn back(&mut self) -> bool {
        let Some(view) = self.history.pop() else {
            return false;
        };
        self.focus = view.focus;
        self.filters = view.filters;
        self.positions = view.positions;
        self.hidden = view.hidden;
        self.camera = view.camera;
        self.offset = view.offset;
        self.pages = view.pages;
        self.cache = None;
        self.result = None;
        self.drag = None;
        true
    }
    pub fn refresh(&mut self, catalog: &Catalog, generation: u64) -> bool {
        let Some(target) = self.focus.clone() else {
            return false;
        };
        if self.cache.as_ref().is_some_and(|(old, focus, filters, _)| {
            *old != generation || focus != &target || filters != &self.filters
        }) {
            self.offset = 0;
            self.pages.clear();
            self.cancel_drag();
        }
        let key = (
            generation,
            target.clone(),
            self.filters.clone(),
            self.offset,
        );
        if self.cache.as_ref() == Some(&key) {
            return false;
        }
        self.result = catalog
            .object(&target)
            .map(|_| catalog.query_relations(&target, self.filters.query_options(self.offset)));
        self.cache = Some(key);
        self.populate_positions();
        true
    }
    fn populate_positions(&mut self) {
        let Some(result) = &self.result else {
            return;
        };
        self.positions
            .entry(position_key(&result.target))
            .or_insert([0.0, 0.0]);
        for depth in 1..=2 {
            let nodes: Vec<_> = result
                .nodes
                .iter()
                .filter(|node| node.depth == depth)
                .collect();
            for (index, node) in nodes.iter().enumerate() {
                let angle = std::f64::consts::TAU * index as f64 / nodes.len().max(1) as f64;
                let radius = if depth == 1 { 240.0 } else { 450.0 };
                self.positions
                    .entry(position_key(&node.target))
                    .or_insert([radius * angle.cos(), radius * angle.sin()]);
            }
        }
    }
    pub fn next_page(&mut self) -> bool {
        let next = self
            .result
            .as_ref()
            .and_then(|result| result.continuation.as_ref())
            .map(|item| item.offset);
        let Some(next) = next.filter(|next| *next > self.offset) else {
            return false;
        };
        self.pages.push(self.offset);
        self.offset = next;
        true
    }
    pub fn previous_page(&mut self) -> bool {
        let Some(previous) = self.pages.pop() else {
            return false;
        };
        self.offset = previous;
        true
    }
    pub fn can_previous(&self) -> bool {
        !self.pages.is_empty()
    }
    pub fn hide(&mut self, relation: &str) {
        self.hidden.insert(relation.into());
    }
    pub fn show_all(&mut self) {
        self.hidden.clear();
    }
    pub fn reset_layout(&mut self) {
        self.positions.clear();
        self.drag = None;
        self.camera = GraphCamera::default();
        self.populate_positions();
    }
    pub fn begin_drag(&mut self, key: &str) {
        if let Some(position) = self.positions.get(key) {
            self.drag = Some((key.into(), *position));
        }
    }
    pub fn drag_to(&mut self, position: [f64; 2]) {
        if position.iter().all(|x| x.is_finite()) {
            if let Some((key, _)) = &self.drag {
                self.positions.insert(key.clone(), position);
            }
        }
    }
    pub fn finish_drag(&mut self) {
        self.drag = None;
    }
    pub fn cancel_drag(&mut self) {
        if let Some((key, before)) = self.drag.take() {
            self.positions.insert(key, before);
        }
    }
    pub fn dragging(&self) -> Option<&str> {
        self.drag.as_ref().map(|(key, _)| key.as_str())
    }
    pub fn draft(&self, id: String, title: String) -> Option<GraphViewDraft> {
        Some(GraphViewDraft {
            id,
            title,
            focus: self.focus.clone()?,
            filters: self.filters.clone(),
            positions: self.positions.clone(),
            hidden_relation_ids: self.hidden.iter().cloned().collect(),
        })
    }
    pub fn load(&mut self, draft: &GraphViewDraft) {
        self.set_focus(draft.focus.clone());
        self.filters = draft.filters.clone();
        self.positions = draft.positions.clone();
        self.hidden = draft.hidden_relation_ids.iter().cloned().collect();
        self.history.clear();
    }
}
