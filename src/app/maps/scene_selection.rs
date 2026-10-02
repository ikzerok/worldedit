use super::*;
use worldline_core::vector_scene::{SceneGeometry, SceneOp};

impl MapCanvas {
    pub(super) fn scene_queue(&mut self, operations: Vec<SceneOp>) {
        if self.scene.job.is_some()
            || self.scene.review_plan.is_some()
            || !self.scene.retry_operations.is_empty()
        {
            self.scene.error = Some("请先完成或取消当前矢量预检/待确认修改".into());
            return;
        }
        if self.scene.intent_baseline.is_none() {
            self.scene.intent_baseline = self.command_baseline.clone();
        }
        self.scene.operations.extend(operations);
    }

    pub(super) fn scene_next_id(&self, prefix: &str) -> String {
        for index in 1.. {
            let id = format!("{prefix}_{index}");
            if self
                .scene
                .source
                .as_ref()
                .is_some_and(|scene| scene.nodes.contains_key(&id))
            {
                continue;
            }
            if self
                .snapshot
                .layers
                .iter()
                .flat_map(|layer| &layer.placements)
                .any(|item| item.id == id)
            {
                continue;
            }
            return id;
        }
        unreachable!()
    }

    pub(super) fn scene_selected_roots(&self) -> Vec<String> {
        let Some(scene) = &self.scene.source else {
            return Vec::new();
        };
        self.scene
            .selection
            .iter()
            .filter(|id| {
                let mut parent = scene
                    .nodes
                    .get(*id)
                    .and_then(|node| node.parent_id.as_ref());
                while let Some(id) = parent {
                    if self.scene.selection.contains(id) {
                        return false;
                    }
                    parent = scene.nodes.get(id).and_then(|node| node.parent_id.as_ref());
                }
                true
            })
            .cloned()
            .collect()
    }

    pub(super) fn scene_selection_tools(&mut self, ui: &mut egui::Ui) {
        if self.scene.selection.is_empty() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("已选 {} 项", self.scene.selection.len()));
            if ui.button("复制").clicked() {
                self.scene_queue(vec![SceneOp::Duplicate {
                    node_ids: self.scene_selected_roots(),
                    id_prefix: self.scene_next_id("copy"),
                    offset: [12.0, 12.0],
                }]);
            }
            if ui.button("删除").clicked() {
                self.scene_queue(vec![SceneOp::Delete {
                    node_ids: self.scene_selected_roots(),
                }]);
            }
            if ui.button("成组").clicked() {
                self.scene_queue(vec![SceneOp::Group {
                    group_id: self.scene_next_id("group"),
                    node_ids: self.scene_selected_roots(),
                    name: "新组".into(),
                }]);
            }
            if ui.button("拆组").clicked() {
                let operations = self
                    .scene_selected_roots()
                    .into_iter()
                    .map(|node_id| SceneOp::Ungroup { node_id })
                    .collect();
                self.scene_queue(operations);
            }
            if ui.button("置前").clicked() {
                self.scene_reorder(true);
            }
            if ui.button("置后").clicked() {
                self.scene_reorder(false);
            }
        });
    }

    fn scene_reorder(&mut self, front: bool) {
        let Some(scene) = &self.scene.source else {
            return;
        };
        let mut groups: BTreeMap<(Option<String>, String), Vec<String>> = BTreeMap::new();
        for id in self.scene_selected_roots() {
            if let Some(node) = scene.nodes.get(&id) {
                groups
                    .entry((node.parent_id.clone(), node.layer_id.clone()))
                    .or_default()
                    .push(id);
            }
        }
        let mut operations = Vec::new();
        for ((parent_id, layer_id), selected) in groups {
            let siblings = parent_id
                .as_ref()
                .and_then(|id| scene.nodes.get(id))
                .and_then(|node| {
                    if let SceneGeometry::Group { children } = &node.geometry {
                        Some(children)
                    } else {
                        None
                    }
                })
                .or_else(|| scene.root_order.get(&layer_id));
            let Some(siblings) = siblings else {
                continue;
            };
            let (chosen, others): (Vec<_>, Vec<_>) = siblings
                .iter()
                .cloned()
                .partition(|id| selected.contains(id));
            let node_ids = if front {
                [others, chosen].concat()
            } else {
                [chosen, others].concat()
            };
            operations.push(SceneOp::Reorder {
                parent_id,
                layer_id,
                node_ids,
            });
        }
        self.scene_queue(operations);
    }
}
