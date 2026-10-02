use super::geometry::hit_test;
use super::render::{draw_geometry, geometry_error_message, geometry_vertex, geometry_vertex_mut};
use super::*;
use egui::Sense;
impl MapCanvas {
    #[allow(dead_code)]
    pub(super) fn set_mode(&mut self, mode: CanvasMode) {
        self.mode = mode;
        self.last_error = None;
        // 尺子与校准切换模式时保留几何预览，避免恢复编辑时丢失草稿。
        // SVG 导入沿用离开编辑展示即取消的既有契约。
        if mode == CanvasMode::Browse {
            self.svg_import = Default::default();
            self.drag = None;
            if let Some(job) = self.scene.job.take() {
                self.scene.retry_operations = job.batch.operations.clone();
                self.scene.retry_review = job.review;
            }
        }
    }

    #[allow(dead_code)]
    pub(super) fn set_tool(&mut self, tool: CanvasTool) {
        self.tool = tool;
        self.draft = None;
        self.drag = None;
        self.last_error = None;
    }

    pub(super) fn hit_test(
        &self,
        point: NormalizedPoint,
        tolerance: f32,
    ) -> Option<(String, GeometryHit)> {
        let screen_scale = self.camera.map_scale();
        let mut found = None;
        for layer in &self.snapshot.layers {
            if layer.visible {
                for placement in &layer.placements {
                    let text_hit = if let MapGeometry::Text { position, .. } = &placement.geometry {
                        self.text_sizes
                            .get(&placement.id)
                            .filter(|(size, zoom)| {
                                Rect::from_min_size(
                                    self.camera
                                        .normalized_to_screen(position.as_pos2(), self.viewport),
                                    *size * (self.camera.zoom() / *zoom),
                                )
                                .expand(4.0)
                                .contains(
                                    self.camera
                                        .normalized_to_screen(point.as_pos2(), self.viewport),
                                )
                            })
                            .map(|_| GeometryHit::Vertex(0))
                    } else {
                        None
                    };
                    if let Some(hit) = text_hit
                        .or_else(|| hit_test(&placement.geometry, point, screen_scale, tolerance))
                    {
                        found = Some((placement.id.clone(), hit));
                    }
                }
            }
        }
        found
    }

    pub(super) fn show(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        self.viewport = response.rect;
        if self.fit_pending && self.viewport.is_positive() {
            self.camera.fit(self.viewport);
            self.fit_pending = false;
        }
        self.text_sizes.clear();
        for layer in &self.snapshot.layers {
            if layer.visible {
                for placement in &layer.placements {
                    if let MapGeometry::Text {
                        text,
                        font_size,
                        color,
                        ..
                    } = &placement.geometry
                    {
                        let galley = text_labels::text_layout(
                            &painter,
                            &self.camera,
                            text,
                            *font_size,
                            color,
                        );
                        self.text_sizes
                            .insert(placement.id.clone(), (galley.size(), self.camera.zoom()));
                    }
                }
            }
        }
        self.handle_input(&response, ui);
        painter.rect_filled(response.rect, 0.0, crate::theme::canvas_background());
        let document = Rect::from_two_pos(
            self.camera.normalized_to_screen(Pos2::ZERO, response.rect),
            self.camera
                .normalized_to_screen(Pos2::new(1.0, 1.0), response.rect),
        );
        painter.rect_filled(document, 0.0, crate::theme::document_background());
        self.draw_rasters(&painter, ui.ctx(), response.rect);
        self.draw_vectors(&painter, response.rect);
        self.draw_scene_overlay(&painter);
        if let Some(draft) = self.draft.as_ref() {
            draw_geometry(
                &painter,
                draft,
                &MapStyle {
                    stroke: Color32::from_rgb(255, 210, 90),
                    fill: Color32::from_rgba_unmultiplied(255, 210, 90, 52),
                    width: 2.0,
                },
                &self.camera,
                response.rect,
            );
        }
        self.draw_measurement(&painter, response.rect);
        if self.snapshot.layers.is_empty()
            && self.snapshot.raster_layers.is_empty()
            && !self.measurement_active()
        {
            painter.text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                "当前地图没有可显示内容",
                egui::FontId::proportional(15.0),
                Color32::from_gray(145),
            );
        }
    }

    pub(super) fn handle_input(&mut self, response: &egui::Response, ui: &egui::Ui) {
        if ui.input(|input| input.key_pressed(egui::Key::Escape)) && self.cancel_measurement() {
            return;
        }
        if self.svg_import.open && self.is_edit_mode() {
            return;
        }
        if self.scene.source.is_some()
            && (self.scene.gesture.is_some()
                || ui.input(|input| {
                    input.key_pressed(egui::Key::Escape) || input.key_pressed(egui::Key::Enter)
                }))
            && self.handle_scene_input(response, ui)
        {
            return;
        }
        if self.mode == CanvasMode::Edit && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.draft = None;
            self.drag = None;
            self.selected = None;
            self.last_error = None;
            return;
        }
        if !response.hovered() {
            if self.mode == CanvasMode::Edit && ui.input(|input| input.pointer.any_released()) {
                self.finish_drag();
            }
            return;
        }
        let pointer = ui.input(|i| i.pointer.hover_pos());
        if let Some(pointer) = pointer.filter(|p| response.rect.contains(*p)) {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > f32::EPSILON {
                self.camera
                    .zoom_at(pointer, (scroll * 0.01).exp(), response.rect);
            }
        }
        if self.measurement_active() {
            self.measurement_input(response, ui);
            return;
        }
        if self.handle_scene_input(response, ui) {
            return;
        }
        if self.tool == CanvasTool::Pan
            || ui.input(|input| {
                input.key_down(egui::Key::Space)
                    || input.pointer.button_down(egui::PointerButton::Middle)
            })
        {
            if response.dragged()
                || ui.input(|input| input.pointer.button_down(egui::PointerButton::Middle))
            {
                self.camera.pan_by(ui.input(|input| input.pointer.delta()));
            }
            return;
        }
        if self.mode == CanvasMode::Browse {
            if response.dragged_by(egui::PointerButton::Primary) {
                self.camera.pan_by(ui.input(|i| i.pointer.delta()));
            }
            if response.clicked() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let map_point = self.camera.screen_to_normalized(pointer, response.rect);
                    self.selected =
                        self.hit_test(NormalizedPoint::new(map_point.x, map_point.y), 10.0);
                    self.scene.selection.clear();
                    self.scene.inspector = None;
                }
            }
            return;
        }
        self.handle_edit_input(response, ui);
    }

    pub(super) fn handle_edit_input(&mut self, response: &egui::Response, ui: &egui::Ui) {
        if self.form_blocked {
            return;
        }
        if !ui.ctx().wants_keyboard_input() && ui.input(|input| input.key_pressed(egui::Key::Enter))
        {
            self.finish_draft();
            return;
        }
        if ui.input(|input| input.key_pressed(egui::Key::Delete))
            && ui.ctx().memory(|memory| memory.focused()).is_none()
        {
            if let Some((placement, _)) = self.selected.clone() {
                self.push_intent(EditIntent::Delete { placement });
                self.draft = None;
                self.drag = None;
                self.last_error = None;
            }
            return;
        }
        let Some(pointer) = response.interact_pointer_pos() else {
            return;
        };
        let map_point = self.camera.screen_to_normalized(pointer, response.rect);
        let normalized = NormalizedPoint::new(map_point.x, map_point.y);
        let primary_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
        if primary_down
            && self.drag.is_none()
            && matches!(self.tool, CanvasTool::Select | CanvasTool::Nodes)
        {
            self.selected = self.hit_test(normalized, 10.0);
            if self.selected.is_some() {
                self.scene.selection.clear();
                self.scene.inspector = None;
            }
            if let Some((placement, GeometryHit::Vertex(vertex))) = self.selected.clone() {
                if self
                    .placement_layer(&placement)
                    .is_some_and(|(_, _, locked)| locked)
                {
                    self.last_error = Some("图层已锁定，只能浏览标记".into());
                } else if let Some(initial_geometry) = self
                    .visible_placement(&placement)
                    .map(|item| item.geometry.clone())
                {
                    let Some(initial_vertex) = geometry_vertex(&initial_geometry, vertex) else {
                        return;
                    };
                    self.drag = Some(DragState {
                        placement,
                        vertex,
                        initial_geometry,
                        start_screen: pointer,
                        grab_offset: [
                            initial_vertex.x - normalized.x,
                            initial_vertex.y - normalized.y,
                        ],
                        baseline: self.command_baseline.clone(),
                        active: false,
                    });
                }
            }
        }
        if primary_down {
            if let Some(drag) = self.drag.clone() {
                if pointer.distance(drag.start_screen) >= DRAG_THRESHOLD_PX {
                    let mut geometry = drag.initial_geometry.clone();
                    let moved = NormalizedPoint::new(
                        normalized.x + drag.grab_offset[0],
                        normalized.y + drag.grab_offset[1],
                    );
                    if let Some(vertex) = geometry_vertex_mut(&mut geometry, drag.vertex) {
                        *vertex = moved;
                        self.draft = Some(geometry.clone());
                    }
                    self.drag = Some(DragState {
                        active: true,
                        ..drag
                    });
                } else if drag.active {
                    let mut geometry = drag.initial_geometry.clone();
                    let moved = NormalizedPoint::new(
                        normalized.x + drag.grab_offset[0],
                        normalized.y + drag.grab_offset[1],
                    );
                    if let Some(vertex) = geometry_vertex_mut(&mut geometry, drag.vertex) {
                        *vertex = moved;
                        self.draft = Some(geometry.clone());
                    }
                }
            }
        }
        if response.double_clicked() {
            self.finish_draft();
        } else if response.clicked() {
            match self.tool {
                CanvasTool::Point => {
                    let geometry = MapGeometry::Point(normalized);
                    self.submit_intent(EditIntent::Create(geometry.clone()), &geometry);
                }
                CanvasTool::Text => {
                    self.push_intent(EditIntent::Create(MapGeometry::Text {
                        position: normalized,
                        text: String::new(),
                        font_size: 24.0,
                        color: "#e8eef8".into(),
                    }));
                }
                CanvasTool::Polyline | CanvasTool::Polygon => self.append_draft_point(normalized),
                CanvasTool::Select | CanvasTool::Nodes => {
                    self.selected = self.hit_test(normalized, 10.0);
                }
                CanvasTool::Pan
                | CanvasTool::Rectangle
                | CanvasTool::Ellipse
                | CanvasTool::Bezier => {}
            }
        }
        if ui.input(|i| i.pointer.any_released()) {
            self.finish_drag();
        }
    }

    pub(super) fn finish_drag(&mut self) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        if !drag.active {
            self.draft = None;
            return;
        }
        let Some(geometry) = self.draft.clone() else {
            return;
        };
        if geometry == drag.initial_geometry {
            self.draft = None;
            return;
        }
        let intent = EditIntent::Move {
            placement: drag.placement,
            geometry: geometry.clone(),
        };
        if !self.submit_intent_with_baseline(intent, &geometry, drag.baseline) {
            // Keep the rejected geometry visible so the user can correct it,
            // retry after an external refresh, or press Esc to cancel.
            self.draft = Some(geometry);
        } else {
            self.draft = None;
        }
    }

    pub(super) fn visible_placement(&self, id: &str) -> Option<&MapPlacement> {
        self.snapshot
            .layers
            .iter()
            .filter(|layer| layer.visible)
            .flat_map(|layer| layer.placements.iter())
            .find(|placement| placement.id == id)
    }

    pub(super) fn append_draft_point(&mut self, point: NormalizedPoint) {
        let is_polygon = self.tool == CanvasTool::Polygon;
        let points = match self.draft.take() {
            Some(MapGeometry::Polyline(points)) if !is_polygon => points,
            Some(MapGeometry::Polygon(points)) if is_polygon => points,
            _ => Vec::new(),
        };
        let mut points = points;
        points.push(point);
        self.draft = Some(if is_polygon {
            MapGeometry::Polygon(points)
        } else {
            MapGeometry::Polyline(points)
        });
    }

    pub(super) fn finish_draft(&mut self) {
        let Some(geometry) = self.draft.take() else {
            return;
        };
        if !self.submit_intent(EditIntent::Create(geometry.clone()), &geometry) {
            self.draft = Some(geometry);
        }
    }

    pub(super) fn submit_intent(&mut self, intent: EditIntent, geometry: &MapGeometry) -> bool {
        match geometry::validate_geometry(geometry) {
            Ok(()) => {
                self.last_error = None;
                self.push_intent(intent);
                true
            }
            Err(error) => {
                self.last_error = Some(geometry_error_message(error));
                false
            }
        }
    }

    pub(super) fn submit_intent_with_baseline(
        &mut self,
        intent: EditIntent,
        geometry: &MapGeometry,
        baseline: Option<MapCommandBaseline>,
    ) -> bool {
        match geometry::validate_geometry(geometry) {
            Ok(()) => {
                self.last_error = None;
                self.edit_intents.push(intent);
                self.intent_baselines
                    .push(baseline.or_else(|| self.command_baseline.clone()));
                true
            }
            Err(error) => {
                self.last_error = Some(geometry_error_message(error));
                false
            }
        }
    }

    pub(super) fn push_intent(&mut self, intent: EditIntent) {
        self.edit_intents.push(intent);
        self.intent_baselines.push(self.command_baseline.clone());
    }
}
