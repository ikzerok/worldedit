use super::geometry::hit_test;
use super::render::{draw_geometry, geometry_error_message, geometry_vertex, geometry_vertex_mut};
use super::*;
use egui::Sense;
impl MapCanvas {
    #[allow(dead_code)]
    pub(super) fn set_mode(&mut self, mode: CanvasMode) {
        self.mode = mode;
        self.last_error = None;
        if mode == CanvasMode::Browse {
            self.svg_import = Default::default();
            self.draft = None;
            self.drag = None;
        }
    }

    #[allow(dead_code)]
    pub(super) fn set_tool(&mut self, tool: CanvasTool) {
        self.tool = tool;
        self.draft = None;
        self.drag = None;
        self.last_error = None;
    }

    pub(in crate::app) fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label(crate::theme::muted("模式"));
            if ui
                .selectable_label(self.mode == CanvasMode::Browse, "浏览")
                .clicked()
            {
                self.set_mode(CanvasMode::Browse);
            }
            if ui
                .selectable_label(self.mode == CanvasMode::Edit, "编辑展示")
                .clicked()
            {
                self.set_mode(CanvasMode::Edit);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(crate::theme::muted("镜头"));
            if ui.small_button("适配全图").clicked() && self.viewport.is_positive() {
                self.camera.fit(self.viewport);
            }
            if ui.small_button("重置镜头").clicked() {
                self.camera.reset();
                self.draft = None;
                self.drag = None;
            }
            ui.label(format!("{:.0}%", self.camera.zoom() * 100.0));
        });
        if self.mode == CanvasMode::Edit {
            ui.horizontal_wrapped(|ui| {
                ui.label(crate::theme::muted("绘制"));
                let previous_tool = self.tool;
                ui.selectable_value(&mut self.tool, CanvasTool::Select, "选择")
                    .on_hover_text("选择标记并拖动控制点，释放后提交一个展示命令");
                ui.selectable_value(&mut self.tool, CanvasTool::Point, "点")
                    .on_hover_text("在地图范围内放置一个点预览");
                ui.selectable_value(&mut self.tool, CanvasTool::Polyline, "线")
                    .on_hover_text("连续点按添加线段，双击完成，Esc 取消");
                ui.selectable_value(&mut self.tool, CanvasTool::Polygon, "面")
                    .on_hover_text("连续点按添加面边界，双击完成，Esc 取消");

                if self.tool != previous_tool {
                    self.draft = None;
                    self.drag = None;
                    self.last_error = None;
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(crate::theme::muted("线/面双击完成，Esc 取消"));
                if ui
                    .add_enabled(
                        self.has_uncommitted_work(),
                        egui::Button::new("放弃未提交修改").small(),
                    )
                    .clicked()
                {
                    self.reset_local_preview();
                }
            });
        }
        if let Some(error) = self.validation_error() {
            ui.colored_label(crate::theme::ERROR, error);
        }
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
                    if let Some(hit) = hit_test(&placement.geometry, point, screen_scale, tolerance)
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
        self.handle_input(&response, ui);
        painter.rect_filled(response.rect, 0.0, Color32::from_gray(22));
        self.draw_rasters(&painter, ui.ctx(), response.rect);
        self.draw_vectors(&painter, response.rect);
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
        if self.snapshot.layers.is_empty() && self.snapshot.raster_layers.is_empty() {
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
        if self.svg_import.open {
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
        if self.mode == CanvasMode::Browse {
            if response.dragged_by(egui::PointerButton::Primary) {
                self.camera.pan_by(ui.input(|i| i.pointer.delta()));
            }
            if response.clicked() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    let map_point = self.camera.screen_to_normalized(pointer, response.rect);
                    self.selected =
                        self.hit_test(NormalizedPoint::new(map_point.x, map_point.y), 10.0);
                }
            }
            return;
        }
        self.handle_edit_input(response, ui);
    }

    pub(super) fn handle_edit_input(&mut self, response: &egui::Response, ui: &egui::Ui) {
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
        if primary_down && self.drag.is_none() && self.tool == CanvasTool::Select {
            self.selected = self.hit_test(normalized, 10.0);
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
                CanvasTool::Polyline | CanvasTool::Polygon => self.append_draft_point(normalized),
                CanvasTool::Select => {
                    self.selected = self.hit_test(normalized, 10.0);
                }
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
