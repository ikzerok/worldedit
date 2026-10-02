use super::scene_handles::{handles, move_handle, HandleKind};
use super::*;
use worldline_core::vector_scene::{
    node_state, Affine, PathSegment, SceneGeometry, SceneNode, SceneOp,
};

#[derive(Clone)]
pub(super) enum SceneGesture {
    Shape {
        tool: CanvasTool,
        start: [f64; 2],
        current: [f64; 2],
    },
    Move {
        start: [f64; 2],
        current: [f64; 2],
        nodes: Vec<(SceneNode, Affine)>,
    },
    Handle {
        node: Box<SceneNode>,
        kind: HandleKind,
        inverse: Affine,
        current: [f64; 2],
    },
    Pen {
        anchor: [f64; 2],
        index: usize,
    },
    Marquee {
        start: Pos2,
        current: Pos2,
    },
}

impl MapCanvas {
    pub(super) fn handle_scene_input(&mut self, response: &egui::Response, ui: &egui::Ui) -> bool {
        if self.scene.source.is_none() {
            return false;
        }
        if self.legacy_place_tool && self.tool == CanvasTool::Point {
            return false;
        }
        let editable = self.mode == CanvasMode::Edit;
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) && editable {
            self.scene.gesture = None;
            self.scene.path.clear();
            self.scene.outgoing = None;
            self.scene.new_subpath = false;
            self.scene.error = None;
            return true;
        }
        if self.scene.job.is_some()
            || self.scene.review_plan.is_some()
            || self.scene.place.is_some()
        {
            return true;
        }
        if !ui.ctx().wants_keyboard_input() && editable {
            if ui.input(|input| input.key_pressed(egui::Key::Enter)) && !self.scene.path.is_empty()
            {
                self.scene_finish_path();
                return true;
            }
            if ui.input(|input| input.key_pressed(egui::Key::Delete))
                && !self.scene.selection.is_empty()
            {
                self.scene_queue(vec![SceneOp::Delete {
                    node_ids: self.scene_selected_roots(),
                }]);
                return true;
            }
        }
        let Some(pointer) = ui.input(|input| input.pointer.interact_pos()) else {
            return false;
        };
        let Some(point) = self.screen_to_scene(pointer) else {
            return false;
        };
        let pan = self.tool == CanvasTool::Pan
            || ui.input(|input| {
                input.key_down(egui::Key::Space)
                    || input.pointer.button_down(egui::PointerButton::Middle)
            });
        if pan && self.scene.gesture.is_none() {
            if response.dragged()
                || ui.input(|input| input.pointer.button_down(egui::PointerButton::Middle))
            {
                self.camera.pan_by(ui.input(|input| input.pointer.delta()));
            }
            return true;
        }
        if !editable {
            if response.clicked() {
                if let Some(id) = self.scene_hit(pointer) {
                    self.select_scene(&id, false);
                    return true;
                }
            }
            return false;
        }
        if self.form_blocked || self.scene.inspector_dirty {
            return true;
        }
        let pressed = response.hovered()
            && ui.input(|input| input.pointer.button_pressed(egui::PointerButton::Primary));
        let released =
            ui.input(|input| input.pointer.button_released(egui::PointerButton::Primary));
        if pressed {
            if self.scene.intent_baseline.is_none() {
                self.scene.intent_baseline = self.command_baseline.clone();
            }
            match self.tool {
                CanvasTool::Rectangle | CanvasTool::Ellipse => {
                    self.scene.gesture = Some(SceneGesture::Shape {
                        tool: self.tool,
                        start: point,
                        current: point,
                    })
                }
                CanvasTool::Bezier => {
                    let index = self.scene_add_anchor(point, ui.input(|input| input.modifiers.alt));
                    self.scene.gesture = Some(SceneGesture::Pen {
                        anchor: point,
                        index,
                    });
                }
                CanvasTool::Select | CanvasTool::Nodes => {
                    if self.tool == CanvasTool::Nodes && self.begin_scene_handle(pointer, point) {
                        return true;
                    }
                    if let Some(mut id) = self.scene_hit(pointer) {
                        if self.tool == CanvasTool::Select
                            && !ui.input(|input| input.modifiers.ctrl)
                            && !self.scene.selection.contains(&id)
                        {
                            if let Some(scene) = &self.scene.source {
                                while let Some(parent) =
                                    scene.nodes.get(&id).and_then(|node| node.parent_id.clone())
                                {
                                    id = parent;
                                }
                            }
                        }
                        if !self.scene.selection.contains(&id)
                            || ui.input(|input| input.modifiers.shift)
                        {
                            self.select_scene(&id, ui.input(|input| input.modifiers.shift));
                        }
                        let scene = self.scene.source.as_ref().expect("scene input");
                        let mut nodes = Vec::new();
                        for id in self.scene_selected_roots() {
                            let Some(node) = scene.nodes.get(&id) else {
                                continue;
                            };
                            if node_state(scene, &id).is_ok_and(|state| !state.locked)
                                && self
                                    .snapshot
                                    .layers
                                    .iter()
                                    .any(|layer| layer.id == node.layer_id && !layer.locked)
                            {
                                let parent = node
                                    .parent_id
                                    .as_ref()
                                    .and_then(|id| node_state(scene, id).ok())
                                    .map_or(Affine::IDENTITY, |state| state.transform);
                                nodes.push((node.clone(), parent));
                            }
                        }
                        self.scene.gesture = Some(SceneGesture::Move {
                            start: point,
                            current: point,
                            nodes,
                        });
                    } else {
                        let normalized = self.camera.screen_to_normalized(pointer, self.viewport);
                        if self
                            .hit_test(NormalizedPoint::new(normalized.x, normalized.y), 10.0)
                            .is_some()
                        {
                            return false;
                        }
                        if !ui.input(|input| input.modifiers.shift) {
                            self.scene.selection.clear();
                            self.scene.inspector = None;
                        }
                        self.scene.gesture = Some(SceneGesture::Marquee {
                            start: pointer,
                            current: pointer,
                        });
                    }
                }
                _ => {}
            }
        }
        if let Some(gesture) = &mut self.scene.gesture {
            match gesture {
                SceneGesture::Shape { current, .. }
                | SceneGesture::Move { current, .. }
                | SceneGesture::Handle { current, .. } => *current = point,
                SceneGesture::Marquee { current, .. } => *current = pointer,
                SceneGesture::Pen { anchor, index } => {
                    self.scene.outgoing = Some(point);
                    if let Some(PathSegment::Cubic { control2, .. }) =
                        self.scene.path.get_mut(*index)
                    {
                        *control2 = [anchor[0] * 2.0 - point[0], anchor[1] * 2.0 - point[1]];
                    }
                }
            }
            if released {
                self.finish_scene_gesture();
            }
            return true;
        }
        if response.double_clicked()
            && matches!(self.tool, CanvasTool::Polyline | CanvasTool::Polygon)
        {
            self.scene_finish_path();
            return true;
        }
        if response.clicked() {
            match self.tool {
                CanvasTool::Point => {
                    self.scene_create(SceneGeometry::Point { position: point }, true)
                }
                CanvasTool::Text => self.scene_text_at(point),
                CanvasTool::Polyline | CanvasTool::Polygon => {
                    self.scene_add_anchor(point, false);
                }
                _ => return false,
            }
            return true;
        }
        !matches!(self.tool, CanvasTool::Select | CanvasTool::Nodes)
    }

    fn begin_scene_handle(&mut self, pointer: Pos2, point: [f64; 2]) -> bool {
        let Some(scene) = &self.scene.source else {
            return false;
        };
        for id in &self.scene.selection {
            let Some(node) = scene.nodes.get(id) else {
                continue;
            };
            let Ok(state) = node_state(scene, id) else {
                continue;
            };
            if state.locked
                || self
                    .snapshot
                    .layers
                    .iter()
                    .any(|layer| layer.id == node.layer_id && layer.locked)
            {
                continue;
            }
            for handle in handles(&node.geometry) {
                if self
                    .scene_to_screen(state.transform.point(handle.point))
                    .is_some_and(|position| position.distance(pointer) <= 8.0)
                {
                    if let Some(inverse) = state.transform.inverse() {
                        self.scene.gesture = Some(SceneGesture::Handle {
                            node: Box::new(node.clone()),
                            kind: handle.kind,
                            inverse,
                            current: point,
                        });
                        return true;
                    }
                }
            }
        }
        false
    }

    fn finish_scene_gesture(&mut self) {
        let Some(gesture) = self.scene.gesture.take() else {
            return;
        };
        match gesture {
            SceneGesture::Shape {
                tool,
                start,
                current,
            } => self.scene_shape(tool, start, current),
            SceneGesture::Handle {
                mut node,
                kind,
                inverse,
                current,
            } => {
                if move_handle(&mut node.geometry, kind, inverse.point(current)) {
                    self.scene_queue(vec![SceneOp::Update { node: *node }]);
                }
            }
            SceneGesture::Move {
                start,
                current,
                nodes,
            } => {
                if self
                    .scene_to_screen(start)
                    .zip(self.scene_to_screen(current))
                    .is_none_or(|(a, b)| a.distance(b) < DRAG_THRESHOLD_PX)
                {
                    return;
                }
                let translation = Affine([
                    1.0,
                    0.0,
                    0.0,
                    1.0,
                    current[0] - start[0],
                    current[1] - start[1],
                ]);
                let ops = nodes
                    .into_iter()
                    .filter_map(|(mut node, parent)| {
                        node.transform = parent
                            .inverse()?
                            .then(translation)
                            .then(parent)
                            .then(node.transform);
                        Some(SceneOp::Update { node })
                    })
                    .collect();
                self.scene_queue(ops);
            }
            SceneGesture::Marquee { start, current } => {
                if start.distance(current) < DRAG_THRESHOLD_PX {
                    return;
                }
                let rectangle = Rect::from_two_pos(start, current);
                let visible: HashSet<_> = self
                    .snapshot
                    .layers
                    .iter()
                    .filter(|layer| layer.visible)
                    .map(|layer| layer.id.as_str())
                    .collect();
                let ids: Vec<_> = self
                    .scene
                    .primitives
                    .iter()
                    .filter(|primitive| {
                        self.scene
                            .source
                            .as_ref()
                            .and_then(|scene| scene.nodes.get(&primitive.node_id))
                            .is_some_and(|node| visible.contains(node.layer_id.as_str()))
                            && self
                                .primitive_screen_bounds(primitive)
                                .is_some_and(|bounds| rectangle.intersects(bounds))
                    })
                    .map(|primitive| primitive.node_id.clone())
                    .collect();
                self.scene.selection.extend(ids);
                self.scene.inspector = self
                    .scene
                    .selection
                    .iter()
                    .next()
                    .and_then(|id| self.scene.source.as_ref()?.nodes.get(id))
                    .cloned();
                self.selected = None;
            }
            SceneGesture::Pen { .. } => {}
        }
    }
}
