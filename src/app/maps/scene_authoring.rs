use super::scene_renderer::{RenderStatus, SceneRenderer};
use super::*;
use std::collections::BTreeSet;
use worldline_core::vector_scene::{
    MapScene, PathSegment, SceneNode, SceneOp, ScenePlan, ScenePrimitive, SceneStyle,
};

pub(super) struct SceneLayer {
    pub(super) scene: MapScene,
    pub(super) renderer: SceneRenderer,
}

pub(super) struct SceneAuthoring {
    pub(super) source: Option<MapScene>,
    pub(super) layers: BTreeMap<String, SceneLayer>,
    pub(super) generation: u64,
    pub(super) selection: BTreeSet<String>,
    pub(super) inspector: Option<SceneNode>,
    pub(super) inspector_dirty: bool,
    pub(super) operations: Vec<SceneOp>,
    pub(super) active_layer: Option<String>,
    pub(super) style: SceneStyle,
    pub(super) close_path: bool,
    pub(super) error: Option<String>,
    pub(super) primitives: Vec<ScenePrimitive>,
    pub(super) job: Option<super::scene_batch_job::SceneBatchJob>,
    pub(super) retry_operations: Vec<SceneOp>,
    pub(super) retry_review: bool,
    pub(super) review_requested: bool,
    pub(super) review_plan: Option<ScenePlan>,
    pub(super) intent_baseline: Option<MapCommandBaseline>,
    pub(super) gesture: Option<super::scene_input::SceneGesture>,
    pub(super) path: Vec<PathSegment>,
    pub(super) outgoing: Option<[f64; 2]>,
    pub(super) new_subpath: bool,
    pub(super) binding_query: String,
    pub(super) binding_node: String,
    pub(super) binding: binding::BindingGuard,
    pub(super) tree_rows: Vec<(String, usize)>,
    pub(super) collapsed: BTreeSet<String>,
    pub(super) tree_dirty: bool,
    pub(super) place: Option<super::scene_place::ScenePlaceForm>,
    pub(super) revealed: BTreeSet<String>,
    pub(super) export: Option<super::scene_exchange::SvgExportForm>,
    pub(super) canvas_transform: worldline_core::vector_scene::Affine,
    pub(super) primitive_layers: BTreeMap<String, Vec<usize>>,
    pub(super) bounds: BTreeMap<String, [f64; 4]>,
}

impl Default for SceneAuthoring {
    fn default() -> Self {
        Self {
            source: None,
            layers: BTreeMap::new(),
            generation: 0,
            selection: BTreeSet::new(),
            inspector: None,
            inspector_dirty: false,
            operations: Vec::new(),
            active_layer: None,
            style: SceneStyle {
                fill: Some("#357ebe66".into()),
                stroke: Some("#65b4ff".into()),
                stroke_width: Some(2.0),
                ..Default::default()
            },
            close_path: false,
            error: None,
            primitives: Vec::new(),
            job: None,
            retry_operations: Vec::new(),
            retry_review: false,
            review_requested: false,
            review_plan: None,
            intent_baseline: None,
            gesture: None,
            path: Vec::new(),
            outgoing: None,
            new_subpath: false,
            binding_query: String::new(),
            binding_node: String::new(),
            binding: binding::BindingGuard::default(),
            tree_rows: Vec::new(),
            collapsed: BTreeSet::new(),
            tree_dirty: true,
            place: None,
            revealed: BTreeSet::new(),
            export: None,
            canvas_transform: worldline_core::vector_scene::Affine::IDENTITY,
            primitive_layers: BTreeMap::new(),
            bounds: BTreeMap::new(),
        }
    }
}

impl SceneAuthoring {
    pub(super) fn sync(&mut self, scene: Option<&MapScene>) {
        self.source = scene.cloned();
        self.generation = self.generation.wrapping_add(1);
        self.layers.clear();
        self.primitives.clear();
        self.primitive_layers.clear();
        self.bounds.clear();
        self.tree_dirty = true;
        let Some(mut display) = scene.cloned() else {
            self.selection.clear();
            if !self.inspector_dirty {
                self.inspector = None;
            }
            return;
        };
        self.revealed.retain(|id| display.nodes.contains_key(id));
        for id in &self.revealed {
            if let Some(node) = display.nodes.get_mut(id) {
                node.visible = true;
            }
        }
        let scene = &display;
        if let Err(error) = worldline_core::vector_scene::validate_scene(
            scene,
            &worldline_core::vector_scene::SceneLimits::default(),
        ) {
            self.error = Some(error.to_string());
            return;
        }
        // 能力声明必须随临时图层投影传播；去重后只复制有界的已知能力，不放大 root extra。
        let mut projection_extra = serde_json::Map::new();
        if let Some(features) = scene
            .extra
            .get("required_features")
            .and_then(serde_json::Value::as_array)
        {
            let unique: std::collections::BTreeSet<_> = features
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect();
            projection_extra.insert("required_features".into(), serde_json::json!(unique));
        }
        // O(nodes + layers)，不为每层重新 filter 整个 scene，也不复制无关 root extras。
        for (id, roots) in &scene.root_order {
            let mut subset = MapScene::new(scene.view_box[2], scene.view_box[3]);
            subset.view_box = scene.view_box;
            subset.preserve_aspect_ratio = scene.preserve_aspect_ratio.clone();
            subset.extra = projection_extra.clone();
            subset.root_order.insert(id.clone(), roots.clone());
            self.layers.insert(
                id.clone(),
                SceneLayer {
                    scene: subset,
                    renderer: SceneRenderer::default(),
                },
            );
        }
        for node in scene.nodes.values() {
            if let Some(layer) = self.layers.get_mut(&node.layer_id) {
                layer.scene.nodes.insert(node.id.clone(), node.clone());
            }
        }
        match worldline_core::vector_scene::project_scene(scene, 0.1) {
            Ok(primitives) => {
                self.primitives = primitives
                    .into_iter()
                    .filter(|primitive| {
                        worldline_core::vector_scene::node_state(scene, &primitive.node_id)
                            .is_ok_and(|state| state.visible)
                    })
                    .collect()
            }
            Err(error) => self.error = Some(error.to_string()),
        }
        for (index, primitive) in self.primitives.iter().enumerate() {
            if let Some(node) = scene.nodes.get(&primitive.node_id) {
                self.primitive_layers
                    .entry(node.layer_id.clone())
                    .or_default()
                    .push(index);
            }
            if let Some(bounds) = super::scene_canvas::clipped_bounds(primitive) {
                let mut current = Some(primitive.node_id.as_str());
                while let Some(id) = current {
                    self.bounds
                        .entry(id.into())
                        .and_modify(|old| {
                            old[0] = old[0].min(bounds[0]);
                            old[1] = old[1].min(bounds[1]);
                            old[2] = old[2].max(bounds[2]);
                            old[3] = old[3].max(bounds[3]);
                        })
                        .or_insert(bounds);
                    current = scene
                        .nodes
                        .get(id)
                        .and_then(|node| node.parent_id.as_deref());
                }
            }
        }
        self.selection.retain(|id| scene.nodes.contains_key(id));
        if !self.inspector_dirty {
            self.inspector = self
                .selection
                .iter()
                .next()
                .and_then(|id| self.source.as_ref()?.nodes.get(id))
                .cloned();
        }
    }

    pub(super) fn has_uncommitted_work(&self) -> bool {
        self.inspector_dirty
            || !self.operations.is_empty()
            || !self.path.is_empty()
            || self.gesture.is_some()
            || self.job.is_some()
            || self.review_plan.is_some()
            || !self.retry_operations.is_empty()
            || self.place.is_some()
    }

    pub(super) fn render_status_panel(&mut self, ui: &mut egui::Ui) {
        let mut queued = 0;
        let mut working = 0;
        let mut errors = 0;
        for layer in self.layers.values() {
            match layer.renderer.status {
                RenderStatus::Queued(_) => queued += 1,
                RenderStatus::Rendering => working += 1,
                RenderStatus::Blocked(_, _) | RenderStatus::Failed(_) => errors += 1,
                _ => {}
            }
        }
        if self.source.is_none() {
            return;
        }
        let title = format!("矢量显示：{working} 层处理中 · {queued} 层排队 · {errors} 层需处理");
        // 状态本身不能改变画布高度，否则 Ready→隐藏状态栏→viewport变大→重绘会循环。
        ui.add(egui::Label::new(&title).truncate())
            .on_hover_text(title);
        egui::CollapsingHeader::new("查看渲染详情")
            .id_salt("scene-render-status")
            .show(ui, |ui| {
                if crate::theme::add_enabled(
                    ui,
                    errors > 0,
                    egui::Button::new("资源已调整，重试失败层"),
                )
                .clicked()
                {
                    for layer in self.layers.values_mut() {
                        layer.renderer.retry();
                    }
                }
                let rows: Vec<_> = self
                    .layers
                    .iter()
                    .filter(|(_, layer)| {
                        !matches!(
                            layer.renderer.status,
                            RenderStatus::Idle | RenderStatus::Ready
                        )
                    })
                    .collect();
                egui::ScrollArea::vertical()
                    .max_height(120.0)
                    .auto_shrink([false, false])
                    .show_rows(ui, 24.0, rows.len(), |ui, range| {
                        for (id, layer) in &rows[range] {
                            if let Some(message) = layer.renderer.message(id) {
                                ui.label(
                                    egui::RichText::new(message).color(crate::theme::WARNING()),
                                );
                            }
                        }
                    });
            });
    }
}

impl MapCanvas {
    pub(super) fn sync_scene(&mut self, scene: Option<&MapScene>) {
        self.scene.sync(scene);
        if let Some(scene) = scene {
            match worldline_core::vector_scene::view_box_transform(
                scene.view_box,
                self.snapshot.canvas.width as f64,
                self.snapshot.canvas.height as f64,
                &scene.preserve_aspect_ratio,
            ) {
                Ok(transform) => self.scene.canvas_transform = transform,
                Err(error) => self.scene.error = Some(error.to_string()),
            }
        }
        if self
            .scene
            .active_layer
            .as_ref()
            .is_none_or(|id| !self.snapshot.layers.iter().any(|layer| &layer.id == id))
        {
            self.scene.active_layer = self
                .snapshot
                .layers
                .iter()
                .find(|layer| layer.visible && !layer.locked)
                .map(|layer| layer.id.clone());
        }
    }

    pub(super) fn release_hidden_scene_layers(&mut self) {
        let visible: HashSet<_> = self
            .snapshot
            .layers
            .iter()
            .filter(|layer| layer.visible)
            .map(|layer| layer.id.as_str())
            .collect();
        for (id, layer) in &mut self.scene.layers {
            if !visible.contains(id.as_str()) {
                layer.renderer.clear();
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/scene_status.rs"]
mod tests;
