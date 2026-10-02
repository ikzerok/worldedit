use super::*;
use worldline_core::vector_scene::{MapScene, SceneGeometry, SceneOp};

impl MapCanvas {
    pub(super) fn scene_tree_panel(&mut self, ui: &mut egui::Ui) {
        let Some(scene) = &self.scene.source else {
            return;
        };
        if self.scene.tree_dirty {
            self.scene.tree_rows.clear();
            for layer in &self.snapshot.layers {
                if let Some(roots) = scene.root_order.get(&layer.id) {
                    flatten(
                        scene,
                        roots,
                        0,
                        &self.scene.collapsed,
                        &mut self.scene.tree_rows,
                    );
                }
            }
            self.scene.tree_dirty = false;
        }
        ui.separator();
        ui.strong(format!("矢量对象 · {} 项", scene.nodes.len()));
        let mut select = None;
        let mut toggle = None;
        let mut updates = Vec::new();
        let rows = &self.scene.tree_rows;
        egui::ScrollArea::vertical()
            .id_salt("scene-object-tree")
            .max_height(440.0)
            .show_rows(ui, 28.0, rows.len(), |ui, range| {
                for (id, depth) in &rows[range] {
                    let Some(node) = scene.nodes.get(id) else {
                        continue;
                    };
                    ui.push_id(id, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_space(*depth as f32 * 12.0);
                            if matches!(node.geometry, SceneGeometry::Group { .. }) {
                                if ui
                                    .small_button(if self.scene.collapsed.contains(id) {
                                        "▸"
                                    } else {
                                        "▾"
                                    })
                                    .clicked()
                                {
                                    toggle = Some(id.clone());
                                }
                            } else {
                                ui.add_space(18.0);
                            }
                            let label = if node.name.is_empty() {
                                &node.id
                            } else {
                                &node.name
                            };
                            let response = ui
                                .selectable_label(self.scene.selection.contains(id), label)
                                .on_hover_text(id);
                            if response.clicked() {
                                select = Some((
                                    id.clone(),
                                    ui.input(|input| input.modifiers.shift),
                                    response.double_clicked(),
                                ));
                            }
                            if self.mode == CanvasMode::Edit {
                                if ui
                                    .small_button(if node.visible { "显" } else { "隐" })
                                    .clicked()
                                {
                                    let mut copy = node.clone();
                                    copy.visible = !copy.visible;
                                    updates.push(SceneOp::Update { node: copy });
                                }
                                if ui
                                    .small_button(if node.locked { "锁" } else { "开" })
                                    .clicked()
                                {
                                    let mut copy = node.clone();
                                    copy.locked = !copy.locked;
                                    updates.push(SceneOp::Update { node: copy });
                                }
                            }
                        })
                    });
                }
            });
        if let Some(id) = toggle {
            if !self.scene.collapsed.remove(&id) {
                self.scene.collapsed.insert(id);
            }
            self.scene.tree_dirty = true;
        }
        if let Some((id, extend, inspect)) = select {
            self.select_scene(&id, extend);
            if inspect {
                self.panel = MapPanel::Inspector;
                self.focus_scene(&id);
            }
        }
        if !updates.is_empty() {
            self.scene_queue(updates);
        }
    }
}

fn flatten(
    scene: &MapScene,
    ids: &[String],
    depth: usize,
    collapsed: &std::collections::BTreeSet<String>,
    rows: &mut Vec<(String, usize)>,
) {
    if depth > 32 {
        return;
    }
    for id in ids {
        rows.push((id.clone(), depth));
        if collapsed.contains(id) {
            continue;
        }
        if let Some(node) = scene.nodes.get(id) {
            if let SceneGeometry::Group { children } = &node.geometry {
                flatten(scene, children, depth + 1, collapsed, rows);
            }
        }
    }
}
