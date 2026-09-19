//! 地图画布原型。

mod camera;
mod geometry;
mod raster;

use camera::Camera2D;
use egui::{Color32, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2};
use geometry::{
    hit_test, triangulate_polygon, GeometryError, GeometryHit, MapGeometry, NormalizedPoint,
};
use raster::RasterTextureCache;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use worldline_core::catalog::TargetRef;
use worldline_core::presentation::{MapDocument, MapRasterLayer};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NormalizedRect {
    pub(super) min: NormalizedPoint,
    pub(super) max: NormalizedPoint,
}

impl NormalizedRect {
    fn to_screen(&self, camera: &Camera2D, viewport: Rect) -> Rect {
        Rect::from_two_pos(
            camera.normalized_to_screen(self.min.as_pos2(), viewport),
            camera.normalized_to_screen(self.max.as_pos2(), viewport),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct MapStyle {
    pub(super) stroke: Color32,
    pub(super) fill: Color32,
    pub(super) width: f32,
}

impl Default for MapStyle {
    fn default() -> Self {
        Self {
            stroke: Color32::from_rgb(101, 180, 255),
            fill: Color32::from_rgba_unmultiplied(53, 126, 190, 72),
            width: 1.5,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct MapPlacement {
    pub(super) id: String,
    pub(super) target_ref: Option<TargetRef>,
    pub(super) annotation: String,
    pub(super) role: String,
    pub(super) label_override: Option<String>,
    pub(super) geometry: MapGeometry,
    pub(super) style: MapStyle,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct MapLayer {
    pub(super) id: String,
    pub(super) visible: bool,
    pub(super) placements: Vec<MapPlacement>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RasterPlacement {
    pub(super) asset_key: String,
    pub(super) asset_path: Option<PathBuf>,
    pub(super) asset_available: bool,
    pub(super) rect: NormalizedRect,
}

impl RasterPlacement {
    fn cache_key(&self) -> String {
        self.asset_path.as_ref().map_or_else(
            || self.asset_key.clone(),
            |path| format!("{}|{}", self.asset_key, path.display()),
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct MapRenderSnapshot {
    pub(super) map_id: String,
    pub(super) title: String,
    pub(super) extent: Vec2,
    pub(super) raster_layers: Vec<RasterPlacement>,
    pub(super) layers: Vec<MapLayer>,
}

impl MapRenderSnapshot {
    pub(super) fn empty(extent: Vec2) -> Self {
        Self {
            map_id: String::new(),
            title: String::new(),
            extent,
            raster_layers: Vec::new(),
            layers: Vec::new(),
        }
    }
}

/// 将 core 已校验的地图 DTO 投影成画布所需的渲染数据。
///
/// 这里不读取或解析地图 JSON；所有结构、坐标和引用都来自 `MapDocument`。
pub(super) fn render_snapshot(document: &MapDocument) -> MapRenderSnapshot {
    let mut layers = Vec::with_capacity(document.layer_order.len());
    for id in &document.layer_order {
        let Some(layer) = document.layers.get(id) else {
            continue;
        };
        layers.push(MapLayer {
            id: layer.id.clone(),
            visible: layer.visible_default,
            placements: document
                .placements
                .values()
                .filter(|placement| placement.layer_id == layer.id)
                .map(|placement| MapPlacement {
                    id: placement.id.clone(),
                    target_ref: placement.target_ref.clone(),
                    annotation: placement.annotation.clone(),
                    role: placement.role.clone(),
                    label_override: placement.label_override.clone(),
                    geometry: geometry_from_core(&placement.geometry),
                    style: MapStyle::default(),
                })
                .collect(),
        });
    }

    MapRenderSnapshot {
        map_id: document.id.clone(),
        title: document.title.clone(),
        extent: Vec2::new(document.canvas.width as f32, document.canvas.height as f32),
        raster_layers: document
            .raster_layers
            .iter()
            .map(raster_from_core)
            .collect(),
        layers,
    }
}

fn geometry_from_core(geometry: &worldline_core::presentation::MapGeometry) -> MapGeometry {
    match geometry {
        worldline_core::presentation::MapGeometry::Point { position } => {
            MapGeometry::Point(NormalizedPoint::new(position[0] as f32, position[1] as f32))
        }
        worldline_core::presentation::MapGeometry::Polyline { points } => MapGeometry::Polyline(
            points
                .iter()
                .map(|point| NormalizedPoint::new(point[0] as f32, point[1] as f32))
                .collect(),
        ),
        worldline_core::presentation::MapGeometry::Polygon { points } => MapGeometry::Polygon(
            points
                .iter()
                .map(|point| NormalizedPoint::new(point[0] as f32, point[1] as f32))
                .collect(),
        ),
    }
}

fn raster_from_core(raster: &MapRasterLayer) -> RasterPlacement {
    RasterPlacement {
        asset_key: raster.asset.id.clone(),
        asset_path: raster
            .asset_info
            .as_ref()
            .map(|asset| PathBuf::from(&asset.resolved_path)),
        asset_available: raster
            .asset_info
            .as_ref()
            .is_some_and(|asset| asset.available),
        rect: NormalizedRect {
            min: NormalizedPoint::new(raster.rect[0] as f32, raster.rect[1] as f32),
            max: NormalizedPoint::new(raster.rect[2] as f32, raster.rect[3] as f32),
        },
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CanvasMode {
    Browse,
    Edit,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CanvasTool {
    Select,
    Point,
    Polyline,
    Polygon,
}

pub(super) struct MapCanvas {
    snapshot: MapRenderSnapshot,
    core_snapshot: MapRenderSnapshot,
    source_version: u64,
    camera: Camera2D,
    fit_pending: bool,
    viewport: Rect,
    mode: CanvasMode,
    tool: CanvasTool,
    draft: Option<MapGeometry>,
    selected: Option<(String, GeometryHit)>,
    drag: Option<DragState>,
    edit_intents: Vec<EditIntent>,
    next_preview_id: u64,
    last_error: Option<String>,
    textures: RasterTextureCache,
    raster_errors: HashMap<String, String>,
    raster_attempts: HashSet<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum EditIntent {
    Create(MapGeometry),
    Move {
        placement: String,
        geometry: MapGeometry,
    },
}

#[derive(Clone, Debug)]
struct DragState {
    placement: String,
    vertex: usize,
}

impl MapCanvas {
    pub(super) fn new(snapshot: MapRenderSnapshot) -> Self {
        Self {
            camera: Camera2D::new(snapshot.extent),
            fit_pending: true,
            core_snapshot: snapshot.clone(),
            snapshot,
            source_version: 0,
            viewport: Rect::NOTHING,
            mode: CanvasMode::Browse,
            tool: CanvasTool::Select,
            draft: None,
            selected: None,
            drag: None,
            edit_intents: Vec::new(),
            next_preview_id: 1,
            last_error: None,
            textures: RasterTextureCache::with_budget(
                raster::RasterDecodeConfig::platform().texture_budget_bytes,
            ),
            raster_errors: HashMap::new(),
            raster_attempts: HashSet::new(),
        }
    }

    pub(super) fn clear(&mut self) {
        let empty = MapRenderSnapshot::empty(Vec2::new(1.0, 1.0));
        self.core_snapshot = empty.clone();
        self.snapshot = empty;
        self.source_version = 0;
        self.camera = Camera2D::new(self.snapshot.extent);
        self.fit_pending = true;
        self.viewport = Rect::NOTHING;
        self.selected = None;
        self.drag = None;
        self.draft = None;
        self.edit_intents.clear();
        self.next_preview_id = 1;
        self.last_error = None;
        self.textures.clear();
        self.raster_errors.clear();
        self.raster_attempts.clear();
    }

    pub(super) fn invalidate_rasters(&mut self) {
        self.textures.clear();
        self.raster_errors.clear();
        self.raster_attempts.clear();
    }

    pub(super) fn needs_snapshot(&self, source_version: u64, map_id: &str) -> bool {
        self.source_version != source_version || self.snapshot.map_id != map_id
    }

    pub(super) fn set_snapshot(&mut self, source_version: u64, mut snapshot: MapRenderSnapshot) {
        let same_map = self.snapshot.map_id == snapshot.map_id
            && self.snapshot.extent == snapshot.extent
            && !snapshot.map_id.is_empty();
        let same_source = self.source_version == source_version;
        if !same_map {
            self.camera = Camera2D::new(snapshot.extent);
            self.fit_pending = true;
            self.selected = None;
            self.drag = None;
            self.draft = None;
            self.edit_intents.clear();
            self.last_error = None;
            self.raster_errors.clear();
            self.raster_attempts.clear();
        } else {
            for layer in &mut snapshot.layers {
                if let Some(previous) = self
                    .snapshot
                    .layers
                    .iter()
                    .find(|previous| previous.id == layer.id)
                {
                    layer.visible = previous.visible;
                }
            }
            if !same_source {
                self.selected = None;
                self.drag = None;
                self.draft = None;
                self.edit_intents.clear();
                self.last_error = None;
            }
        }
        self.source_version = source_version;
        self.core_snapshot = snapshot.clone();
        self.snapshot = snapshot;
    }

    pub(super) fn layer_states(&self) -> Vec<(String, bool, usize)> {
        self.snapshot
            .layers
            .iter()
            .map(|layer| (layer.id.clone(), layer.visible, layer.placements.len()))
            .collect()
    }

    pub(super) fn set_layer_visible(&mut self, id: &str, visible: bool) {
        if let Some(layer) = self.snapshot.layers.iter_mut().find(|layer| layer.id == id) {
            layer.visible = visible;
            if !visible {
                self.selected = None;
            }
        }
    }

    pub(super) fn selected_placement(&self) -> Option<MapPlacement> {
        let (id, _) = self.selected.as_ref()?;
        self.snapshot
            .layers
            .iter()
            .filter(|layer| layer.visible)
            .flat_map(|layer| layer.placements.iter())
            .find(|placement| placement.id == *id)
            .cloned()
    }

    pub(super) fn map_title(&self) -> &str {
        &self.snapshot.title
    }

    pub(super) fn map_id(&self) -> &str {
        &self.snapshot.map_id
    }

    #[cfg(test)]
    pub(super) fn raster_bytes(&self) -> usize {
        self.textures.used_bytes()
    }

    #[cfg(test)]
    pub(super) fn set_raster_budget(&mut self, budget: usize) {
        self.textures = RasterTextureCache::with_budget(budget);
        self.raster_errors.clear();
        self.raster_attempts.clear();
    }

    #[cfg(test)]
    pub(super) fn raster_attempt_count(&self) -> usize {
        self.raster_attempts.len()
    }

    pub(super) fn raster_states(&self) -> Vec<(String, bool, Option<String>)> {
        self.snapshot
            .raster_layers
            .iter()
            .map(|raster| {
                let key = raster.cache_key();
                (
                    raster.asset_key.clone(),
                    raster.asset_available,
                    self.raster_errors.get(&key).cloned(),
                )
            })
            .collect()
    }

    pub(super) fn prepare_rasters(&mut self, ctx: &egui::Context, root: &Path) {
        let config = raster::RasterDecodeConfig::platform();
        let rasters = self.snapshot.raster_layers.clone();
        for raster in rasters {
            let key = raster.cache_key();
            if self.textures.contains(&key)
                || self.raster_errors.contains_key(&key)
                || self.raster_attempts.contains(&key)
            {
                continue;
            }
            self.raster_attempts.insert(key.clone());
            if !raster.asset_available {
                self.raster_errors
                    .insert(key, "素材缺失、不可读或不适合作为栅格图层".into());
                continue;
            }
            let Some(path) = raster.asset_path else {
                self.raster_errors.insert(key, "素材路径不可用".into());
                continue;
            };
            match read_raster(root, &path, config) {
                Ok(decoded) => {
                    if !self.textures.insert(ctx, &key, &decoded) {
                        self.raster_errors
                            .insert(key, "素材解码结果超过纹理缓存预算".into());
                    }
                }
                Err(error) => {
                    self.raster_errors.insert(key, error);
                }
            }
        }
    }

    #[cfg(test)]
    pub fn camera(&self) -> &Camera2D {
        &self.camera
    }

    #[cfg(test)]
    pub fn viewport(&self) -> Rect {
        self.viewport
    }

    #[cfg(test)]
    pub fn edit_intents(&self) -> &[EditIntent] {
        &self.edit_intents
    }

    pub(super) fn take_edit_intents(&mut self) -> Vec<EditIntent> {
        std::mem::take(&mut self.edit_intents)
    }

    pub(super) fn apply_local_intents(&mut self, intents: Vec<EditIntent>) {
        for intent in intents {
            match intent {
                EditIntent::Create(geometry) => {
                    let id = self.next_preview_placement_id();
                    let placement = MapPlacement {
                        id,
                        target_ref: None,
                        annotation: "临时绘图预览".into(),
                        role: "本地草稿".into(),
                        label_override: None,
                        geometry,
                        style: MapStyle::default(),
                    };
                    if let Some(layer) = self.snapshot.layers.iter_mut().find(|layer| layer.visible)
                    {
                        layer.placements.push(placement);
                    } else {
                        self.snapshot.layers.push(MapLayer {
                            id: "__preview__".into(),
                            visible: true,
                            placements: vec![placement],
                        });
                    }
                }
                EditIntent::Move {
                    placement,
                    geometry,
                } => {
                    if let Err(error) = geometry::validate_geometry(&geometry) {
                        self.last_error = Some(geometry_error_message(error));
                        continue;
                    }
                    if let Some(existing) = self
                        .snapshot
                        .layers
                        .iter_mut()
                        .flat_map(|layer| layer.placements.iter_mut())
                        .find(|existing| existing.id == placement)
                    {
                        existing.geometry = geometry;
                    }
                }
            }
        }
    }

    fn next_preview_placement_id(&mut self) -> String {
        loop {
            let id = format!("__preview_{}", self.next_preview_id);
            self.next_preview_id = self.next_preview_id.saturating_add(1);
            if !self
                .snapshot
                .layers
                .iter()
                .flat_map(|layer| layer.placements.iter())
                .any(|placement| placement.id == id)
            {
                return id;
            }
        }
    }

    pub(super) fn reset_local_preview(&mut self) {
        let visibility = self
            .snapshot
            .layers
            .iter()
            .map(|layer| (layer.id.clone(), layer.visible))
            .collect::<HashMap<_, _>>();
        self.snapshot = self.core_snapshot.clone();
        for layer in &mut self.snapshot.layers {
            if let Some(visible) = visibility.get(&layer.id) {
                layer.visible = *visible;
            }
        }
        self.selected = None;
        self.drag = None;
        self.draft = None;
        self.edit_intents.clear();
        self.last_error = None;
    }

    pub(super) fn validation_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    #[allow(dead_code)]
    fn set_mode(&mut self, mode: CanvasMode) {
        self.mode = mode;
        self.last_error = None;
        if mode == CanvasMode::Browse {
            self.draft = None;
            self.drag = None;
        }
    }

    #[allow(dead_code)]
    fn set_tool(&mut self, tool: CanvasTool) {
        self.tool = tool;
        self.draft = None;
        self.drag = None;
        self.last_error = None;
    }

    pub(super) fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("浏览地图");
            if ui
                .selectable_label(self.mode == CanvasMode::Browse, "浏览")
                .clicked()
            {
                self.set_mode(CanvasMode::Browse);
            }
            if ui
                .selectable_label(self.mode == CanvasMode::Edit, "临时绘图预览")
                .clicked()
            {
                self.set_mode(CanvasMode::Edit);
            }
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
                ui.label(crate::theme::muted("临时绘图预览，不保存到工程"));
                let previous_tool = self.tool;
                ui.selectable_value(&mut self.tool, CanvasTool::Select, "选择")
                    .on_hover_text("选择标记并拖动控制点，释放后提交本地预览");
                ui.selectable_value(&mut self.tool, CanvasTool::Point, "点")
                    .on_hover_text("在地图范围内放置一个点预览");
                ui.selectable_value(&mut self.tool, CanvasTool::Polyline, "线")
                    .on_hover_text("连续点按添加线段，双击完成，Esc 取消");
                ui.selectable_value(&mut self.tool, CanvasTool::Polygon, "面")
                    .on_hover_text("连续点按添加面边界，双击完成，Esc 取消");
                ui.label(crate::theme::muted("线/面双击完成，Esc 取消"));
                if self.tool != previous_tool {
                    self.draft = None;
                    self.drag = None;
                    self.last_error = None;
                }
                if ui.small_button("放弃预览修改").clicked() {
                    self.reset_local_preview();
                }
            });
        }
        if let Some(error) = self.validation_error() {
            ui.colored_label(crate::theme::ERROR, error);
        }
    }

    fn hit_test(&self, point: NormalizedPoint, tolerance: f32) -> Option<(String, GeometryHit)> {
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

    fn show(&mut self, ui: &mut egui::Ui) {
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

    fn handle_input(&mut self, response: &egui::Response, ui: &egui::Ui) {
        if self.mode == CanvasMode::Edit && ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.draft = None;
            self.drag = None;
            self.selected = None;
            self.last_error = None;
            return;
        }
        if !response.hovered() {
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

    fn handle_edit_input(&mut self, response: &egui::Response, ui: &egui::Ui) {
        let Some(pointer) = response.interact_pointer_pos() else {
            return;
        };
        let map_point = self.camera.screen_to_normalized(pointer, response.rect);
        let normalized = NormalizedPoint::new(map_point.x, map_point.y);
        let primary_down = ui.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
        if primary_down && self.drag.is_none() && self.tool == CanvasTool::Select {
            self.selected = self.hit_test(normalized, 10.0);
            if let Some((placement, GeometryHit::Vertex(vertex))) = self.selected.clone() {
                self.drag = Some(DragState { placement, vertex });
            }
        }
        if primary_down {
            if let Some(drag) = self.drag.clone() {
                if let Some(layer_placement) = self.visible_placement(&drag.placement) {
                    let mut geometry = layer_placement.geometry.clone();
                    if let Some(vertex) = geometry_vertex_mut(&mut geometry, drag.vertex) {
                        *vertex = normalized;
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
            if let Some(drag) = self.drag.take() {
                if let Some(geometry) = self.draft.take() {
                    self.submit_intent(
                        EditIntent::Move {
                            placement: drag.placement,
                            geometry: geometry.clone(),
                        },
                        &geometry,
                    );
                }
            }
        }
    }

    fn visible_placement(&self, id: &str) -> Option<&MapPlacement> {
        self.snapshot
            .layers
            .iter()
            .filter(|layer| layer.visible)
            .flat_map(|layer| layer.placements.iter())
            .find(|placement| placement.id == id)
    }

    fn append_draft_point(&mut self, point: NormalizedPoint) {
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

    fn finish_draft(&mut self) {
        let Some(geometry) = self.draft.take() else {
            return;
        };
        if !self.submit_intent(EditIntent::Create(geometry.clone()), &geometry) {
            self.draft = Some(geometry);
        }
    }

    fn submit_intent(&mut self, intent: EditIntent, geometry: &MapGeometry) -> bool {
        match geometry::validate_geometry(geometry) {
            Ok(()) => {
                self.last_error = None;
                self.edit_intents.push(intent);
                true
            }
            Err(error) => {
                self.last_error = Some(geometry_error_message(error));
                false
            }
        }
    }

    fn draw_rasters(&mut self, painter: &egui::Painter, _ctx: &egui::Context, viewport: Rect) {
        for raster in &self.snapshot.raster_layers {
            let rect = raster.rect.to_screen(&self.camera, viewport);
            let key = raster.cache_key();
            if let Some(texture) = self.textures.get(&key) {
                painter.image(
                    texture.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                painter.rect_filled(rect, 0.0, Color32::from_gray(38));
                painter.rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(1.0_f32, Color32::from_gray(85)),
                    StrokeKind::Inside,
                );
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    self.raster_errors
                        .get(&key)
                        .map(String::as_str)
                        .unwrap_or("栅格资源不可用"),
                    egui::FontId::proportional(12.0),
                    Color32::from_gray(150),
                );
            }
        }
    }

    fn draw_vectors(&self, painter: &egui::Painter, viewport: Rect) {
        for layer in &self.snapshot.layers {
            if !layer.visible {
                continue;
            }
            for placement in &layer.placements {
                draw_geometry(
                    painter,
                    &placement.geometry,
                    &placement.style,
                    &self.camera,
                    viewport,
                );
                if self
                    .selected
                    .as_ref()
                    .is_some_and(|(id, _)| id == &placement.id)
                {
                    draw_control_points(painter, &placement.geometry, &self.camera, viewport);
                }
            }
        }
    }
}

fn read_raster(
    root: &Path,
    path: &Path,
    config: raster::RasterDecodeConfig,
) -> Result<raster::DecodedRaster, String> {
    let path = worldline_core::file_access::within(root, path)?;
    let max_encoded_bytes = max_encoded_bytes();
    preflight_raster_size(&path, max_encoded_bytes)?;
    let bytes = worldline_core::file_access::read(&path).map_err(|error| error.to_string())?;
    if bytes.len() > max_encoded_bytes {
        return Err(format!(
            "素材文件超过读取上限（{} MiB）",
            max_encoded_bytes / (1024 * 1024)
        ));
    }
    raster::decode_rgba(&bytes, config).map_err(raster_error_message)
}

fn preflight_raster_size(path: &Path, max_encoded_bytes: usize) -> Result<(), String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let size = std::fs::metadata(path)
            .map_err(|error| format!("无法读取素材大小：{error}"))?
            .len();
        if size > max_encoded_bytes as u64 {
            return Err(format!(
                "素材文件超过读取上限（{} MiB）",
                max_encoded_bytes / (1024 * 1024)
            ));
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let size = crate::web::imported_size(path)
            .ok_or_else(|| "素材尚未导入当前浏览器工作区".to_string())?;
        if size > max_encoded_bytes {
            return Err(format!(
                "素材文件超过读取上限（{} MiB）",
                max_encoded_bytes / (1024 * 1024)
            ));
        }
    }
    Ok(())
}

fn max_encoded_bytes() -> usize {
    #[cfg(target_arch = "wasm32")]
    {
        64 * 1024 * 1024
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        128 * 1024 * 1024
    }
}

fn raster_error_message(error: raster::RasterError) -> String {
    match error {
        raster::RasterError::UnsupportedFormat => "仅支持 PNG 或 JPEG 素材".into(),
        raster::RasterError::InvalidImage(message) => format!("图片无法解码：{message}"),
        raster::RasterError::DimensionsExceeded { width, height } => {
            format!("图片尺寸 {width}×{height} 超出显示上限")
        }
        raster::RasterError::PixelsExceeded { width, height } => {
            format!("图片像素数 {width}×{height} 超出显示上限")
        }
        raster::RasterError::AllocationExceeded { bytes } => {
            format!("图片解码内存 {} MiB 超出显示上限", bytes / (1024 * 1024))
        }
    }
}

fn geometry_error_message(error: GeometryError) -> String {
    match error {
        GeometryError::NonFinite => "点坐标必须是有限数字".into(),
        GeometryError::OutOfRange => "点必须位于地图范围内".into(),
        GeometryError::TooFewPoints => "线至少需要两个点，面至少需要三个点".into(),
        GeometryError::RepeatedClosingPoint => "面不应重复首个点作为末点".into(),
        GeometryError::SelfIntersection => "面边界不能自相交".into(),
        GeometryError::Degenerate => "几何形状不能退化".into(),
    }
}

fn geometry_vertex_mut(geometry: &mut MapGeometry, index: usize) -> Option<&mut NormalizedPoint> {
    match geometry {
        MapGeometry::Point(point) if index == 0 => Some(point),
        MapGeometry::Polyline(points) | MapGeometry::Polygon(points) => points.get_mut(index),
        _ => None,
    }
}

fn draw_geometry(
    painter: &egui::Painter,
    geometry: &MapGeometry,
    style: &MapStyle,
    camera: &Camera2D,
    viewport: Rect,
) {
    match geometry {
        MapGeometry::Point(point) => {
            let screen = camera.normalized_to_screen(point.as_pos2(), viewport);
            painter.circle_filled(screen, 5.0, style.stroke);
            painter.circle_stroke(screen, 7.0, Stroke::new(style.width, style.stroke));
        }
        MapGeometry::Polyline(points) => {
            let screen_points = points
                .iter()
                .map(|point| camera.normalized_to_screen(point.as_pos2(), viewport))
                .collect::<Vec<_>>();
            if screen_points.len() >= 2 {
                painter.add(Shape::line(
                    screen_points,
                    Stroke::new(style.width, style.stroke),
                ));
            }
        }
        MapGeometry::Polygon(points) => {
            let screen_points = points
                .iter()
                .map(|point| camera.normalized_to_screen(point.as_pos2(), viewport))
                .collect::<Vec<_>>();
            if let Some(triangles) = triangulate_polygon(points) {
                for triangle in triangles {
                    painter.add(Shape::convex_polygon(
                        triangle
                            .into_iter()
                            .map(|index| screen_points[index])
                            .collect(),
                        style.fill,
                        Stroke::NONE,
                    ));
                }
            }
            if screen_points.len() >= 3 {
                painter.add(Shape::closed_line(
                    screen_points,
                    Stroke::new(style.width, style.stroke),
                ));
            }
        }
    }
}

fn draw_control_points(
    painter: &egui::Painter,
    geometry: &MapGeometry,
    camera: &Camera2D,
    viewport: Rect,
) {
    let draw = |painter: &egui::Painter, point: &NormalizedPoint| {
        painter.circle_filled(
            camera.normalized_to_screen(point.as_pos2(), viewport),
            4.0,
            Color32::from_rgb(255, 210, 90),
        );
    };
    match geometry {
        MapGeometry::Point(point) => draw(painter, point),
        MapGeometry::Polyline(points) | MapGeometry::Polygon(points) => {
            for point in points {
                draw(painter, point);
            }
        }
    }
}

impl super::WorldeditApp {
    pub(super) fn map_tab(&mut self, ctx: &egui::Context) {
        let (map_summaries, map_ids, map_diagnostics) = self
            .snapshot
            .as_ref()
            .map(|snapshot| {
                (
                    snapshot
                        .map_index
                        .maps
                        .values()
                        .map(|map| (map.id.clone(), map.title.clone()))
                        .collect::<Vec<_>>(),
                    snapshot.map_index.maps.keys().cloned().collect::<Vec<_>>(),
                    snapshot.map_index.diagnostics.clone(),
                )
            })
            .unwrap_or_default();
        if self
            .map_selection
            .as_ref()
            .is_none_or(|id| !map_ids.iter().any(|map_id| map_id == id))
        {
            self.map_selection = map_ids.first().cloned();
        }
        let selected_map_id = self.map_selection.clone();
        let has_document = selected_map_id
            .as_ref()
            .is_some_and(|id| map_ids.iter().any(|map_id| map_id == id));
        if let Some(map_id) = selected_map_id.as_deref().filter(|_| has_document) {
            if self.map_canvas.needs_snapshot(self.version, map_id) {
                let document = self
                    .snapshot
                    .as_ref()
                    .and_then(|snapshot| snapshot.map_index.maps.get(map_id));
                if let Some(document) = document {
                    self.map_canvas
                        .set_snapshot(self.version, render_snapshot(document));
                } else {
                    self.map_canvas.clear();
                }
            }
            self.map_canvas.prepare_rasters(ctx, &self.project.root);
        } else if !self.map_canvas.map_id().is_empty()
            || !self.map_canvas.snapshot.layers.is_empty()
            || !self.map_canvas.snapshot.raster_layers.is_empty()
        {
            self.map_canvas.clear();
        }

        let selected_placement = self.map_canvas.selected_placement();
        let selected_label = selected_placement.as_ref().and_then(|placement| {
            placement.label_override.clone().or_else(|| {
                placement.target_ref.as_ref().and_then(|target| {
                    self.snapshot
                        .as_ref()?
                        .result
                        .analysis
                        .catalog
                        .object(target)
                        .map(|object| object.display.clone())
                })
            })
        });
        egui::SidePanel::right("map-inspector")
            .resizable(true)
            .default_width(310.0)
            .width_range(260.0..=420.0)
            .frame(crate::theme::panel())
            .show(ctx, |ui| {
                ui.heading("地图浏览");
                ui.label(crate::theme::muted("地图选择、图层和标记信息"));
                ui.separator();
                ui.label(egui::RichText::new("已注册地图").strong());
                if map_summaries.is_empty() {
                    ui.label(crate::theme::muted("当前工程没有注册地图。"));
                } else {
                    egui::ScrollArea::vertical()
                        .id_salt("map-list")
                        .max_height(150.0)
                        .show(ui, |ui| {
                            for (id, title) in &map_summaries {
                                let label = format!("{}  ·  {}", title, id);
                                if ui
                                    .add(egui::Button::selectable(
                                        self.map_selection.as_deref() == Some(id.as_str()),
                                        label,
                                    ))
                                    .clicked()
                                {
                                    self.map_selection = Some(id.clone());
                                }
                            }
                        });
                }

                if has_document {
                    ui.separator();
                    ui.label(egui::RichText::new("图层").strong());
                    ui.label(crate::theme::muted("显隐仅作用于当前浏览显示。"));
                    for (id, visible, count) in self.map_canvas.layer_states() {
                        let mut next = visible;
                        if ui
                            .checkbox(&mut next, format!("{}  ·  {} 个标记", id, count))
                            .changed()
                        {
                            self.map_canvas.set_layer_visible(&id, next);
                        }
                    }

                    if !self.map_canvas.raster_states().is_empty() {
                        ui.separator();
                        ui.label(egui::RichText::new("栅格图层").strong());
                        for (asset, available, error) in self.map_canvas.raster_states() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(asset);
                                if available && error.is_none() {
                                    ui.colored_label(crate::theme::ACCENT, "已加载");
                                } else {
                                    ui.colored_label(crate::theme::GOLD, "不可用");
                                }
                            });
                            if let Some(error) = error {
                                ui.label(crate::theme::muted(error));
                            }
                        }
                    }

                    ui.separator();
                    ui.label(egui::RichText::new("标记信息").strong());
                    if let Some(placement) = selected_placement.as_ref() {
                        let display = selected_label
                            .clone()
                            .unwrap_or_else(|| placement.id.clone());
                        ui.label(egui::RichText::new(display).strong());
                        ui.label(crate::theme::muted(format!(
                            "标记 ID：{} · {}",
                            placement.id, placement.role
                        )));
                        if !placement.annotation.is_empty() {
                            ui.label(&placement.annotation);
                        }
                        if let Some(target) = placement.target_ref.clone() {
                            ui.label(crate::theme::muted(format!(
                                "对象：{} · {}",
                                target.kind, target.id
                            )));
                            if ui.small_button("打开资料").clicked() {
                                self.open_reading(target);
                            }
                        }
                    } else {
                        ui.label(crate::theme::muted("点击画布上的标记查看信息。"));
                    }
                }

                if !map_diagnostics.is_empty() {
                    ui.separator();
                    ui.label(egui::RichText::new("地图诊断").strong());
                    egui::ScrollArea::vertical()
                        .id_salt("map-diagnostics")
                        .show(ui, |ui| {
                            for diagnostic in &map_diagnostics {
                                let color = match diagnostic.severity {
                                    worldline_core::Severity::Error => crate::theme::ERROR,
                                    worldline_core::Severity::Warning => crate::theme::GOLD,
                                    worldline_core::Severity::Hint => crate::theme::ACCENT,
                                };
                                ui.colored_label(
                                    color,
                                    format!("[{}] {}", diagnostic.code, diagnostic.message),
                                );
                                ui.label(crate::theme::muted(diagnostic.file.clone()));
                            }
                        });
                }
            });

        egui::CentralPanel::default()
            .frame(crate::theme::panel().fill(crate::theme::BG))
            .show(ctx, |ui| {
                self.page_heading(ui, "地图画布", "浏览注册地图、图层和标记，不会修改工程。");
                if !self.map_canvas.map_title().is_empty() {
                    ui.label(egui::RichText::new(self.map_canvas.map_title()).strong());
                }
                self.map_canvas.toolbar(ui);
                ui.separator();
                self.map_canvas.show(ui);
            });

        // 临时绘图预览只更新画布草稿，不进入 Project 历史。
        let intents = self.map_canvas.take_edit_intents();
        self.map_canvas.apply_local_intents(intents);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2, Event, MouseWheelUnit, RawInput, Rect};

    const TEST_PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240,
        31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];

    #[test]
    fn app_renders_a_registered_map_from_the_core_snapshot_without_dirtying_project() {
        let root = test_workspace("registered-map");
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
        app.tab = super::super::Tab::Map;
        let source_before = app.project.sources();
        let dirty_before = app.project.is_dirty();

        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
                ..Default::default()
            },
            |ctx| app.map_tab(ctx),
        );

        assert_eq!(app.map_canvas.snapshot.layers.len(), 1);
        assert_eq!(app.map_canvas.snapshot.layers[0].placements.len(), 1);
        assert_eq!(app.map_canvas.map_id(), "harbor");
        assert_eq!(app.map_canvas.raster_bytes(), 4);
        app.map_canvas.camera.pan_by(vec2(27.0, -11.0));
        let camera_before_repaint = *app.map_canvas.camera();
        app.map_canvas.set_layer_visible("places", false);
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
                ..Default::default()
            },
            |ctx| app.map_tab(ctx),
        );
        assert_eq!(*app.map_canvas.camera(), camera_before_repaint);
        assert!(!app.map_canvas.layer_states()[0].1);
        assert_eq!(app.project.sources(), source_before);
        assert_eq!(app.project.is_dirty(), dirty_before);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn app_clears_the_previous_map_when_registration_disappears() {
        let root = test_workspace("map-removed");
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
        app.tab = super::super::Tab::Map;
        let frame = || RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
            ..Default::default()
        };
        let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
        assert_eq!(app.map_canvas.map_id(), "harbor");

        std::fs::write(
            root.join(".world/project.json"),
            r#"{
  "schema_version": 1,
  "language_version": "1.9",
  "entry": "world.wl",
  "required_features": ["presentation.maps.v1"],
  "maps": {},
  "graph_views": {},
  "extensions": {}
}"#,
        )
        .expect("updated manifest");
        app.project.refresh().expect("refresh test workspace");
        app.recompile();
        let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));

        assert!(app.map_canvas.map_id().is_empty());
        assert!(app.map_canvas.snapshot.layers.is_empty());
        assert!(app.map_canvas.snapshot.raster_layers.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn app_surfaces_a_missing_raster_asset_without_writing_the_project() {
        let root = test_workspace("missing-raster");
        std::fs::remove_file(root.join("assets/harbor.png")).expect("remove test raster");
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
        app.tab = super::super::Tab::Map;
        let source_before = app.project.sources();
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
                ..Default::default()
            },
            |ctx| app.map_tab(ctx),
        );

        let states = app.map_canvas.raster_states();
        assert_eq!(states.len(), 1);
        assert!(!states[0].1);
        assert!(states[0]
            .2
            .as_deref()
            .is_some_and(|message| message.contains("素材")));
        assert_eq!(app.project.sources(), source_before);
        assert!(!app.project.is_dirty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn raster_budget_eviction_does_not_retry_reads_on_each_repaint() {
        let root = test_workspace("raster-budget");
        let second = root.join("assets/second.png");
        std::fs::write(&second, TEST_PNG).expect("second test raster");
        let mut canvas = MapCanvas::new(MapRenderSnapshot {
            map_id: "budget".into(),
            title: "预算测试".into(),
            extent: vec2(100.0, 100.0),
            raster_layers: vec![
                RasterPlacement {
                    asset_key: "first".into(),
                    asset_path: Some(root.join("assets/harbor.png")),
                    asset_available: true,
                    rect: NormalizedRect {
                        min: NormalizedPoint::new(0.0, 0.0),
                        max: NormalizedPoint::new(0.5, 1.0),
                    },
                },
                RasterPlacement {
                    asset_key: "second".into(),
                    asset_path: Some(second),
                    asset_available: true,
                    rect: NormalizedRect {
                        min: NormalizedPoint::new(0.5, 0.0),
                        max: NormalizedPoint::new(1.0, 1.0),
                    },
                },
            ],
            layers: Vec::new(),
        });
        canvas.set_raster_budget(4);
        let ctx = egui::Context::default();
        canvas.prepare_rasters(&ctx, &root);
        assert_eq!(canvas.raster_attempt_count(), 2);
        assert_eq!(canvas.raster_bytes(), 4);
        canvas.prepare_rasters(&ctx, &root);
        assert_eq!(canvas.raster_attempt_count(), 2);
        assert_eq!(canvas.raster_bytes(), 4);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn opening_a_map_fits_its_extent_and_redraw_preserves_the_camera() {
        let root = test_workspace("initial-fit");
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = super::super::WorldeditApp::new(&creation, Some(root.clone()));
        let frame = || RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
            ..Default::default()
        };
        let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
        let viewport = app.map_canvas.viewport();
        for corner in [pos2(0.0, 0.0), pos2(1.0, 1.0)] {
            let screen = app
                .map_canvas
                .camera()
                .normalized_to_screen(corner, viewport);
            assert!(
                viewport.expand(0.01).contains(screen),
                "地图边界应在画布内: {screen:?} {viewport:?}"
            );
        }
        app.map_canvas.camera.pan_by(vec2(17.0, 9.0));
        let camera = *app.map_canvas.camera();
        let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
        assert_eq!(*app.map_canvas.camera(), camera);
        assert!(!app.project.is_dirty());
        let _ = std::fs::remove_dir_all(root);
    }

    fn test_workspace(name: &str) -> std::path::PathBuf {
        let root =
            std::env::temp_dir().join(format!("worldedit-map-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".world")).expect("test workspace directory");
        std::fs::write(
            root.join("world.wl"),
            "world harbor as \"雾港\"\n  description \"地图阅读测试\"\nasset harbor_image image \"assets/harbor.png\" as \"港口图\"\n",
        )
        .expect("test source");
        std::fs::create_dir_all(root.join("assets")).expect("asset directory");
        std::fs::write(root.join("assets/harbor.png"), TEST_PNG).expect("test raster");
        std::fs::write(
            root.join(".world/project.json"),
            r#"{
  "schema_version": 1,
  "language_version": "1.9",
  "entry": "world.wl",
  "required_features": ["presentation.maps.v1"],
  "maps": {"harbor": ".world/maps/harbor.json"},
  "graph_views": {},
  "extensions": {}
}"#,
        )
        .expect("test manifest");
        std::fs::create_dir_all(root.join(".world/maps")).expect("map directory");
        std::fs::write(
            root.join(".world/maps/harbor.json"),
            r#"{
  "schema_version": 1,
  "id": "harbor",
  "title": "雾港地图",
  "raster_layers": [{"id": "base", "asset": {"kind": "asset", "id": "harbor_image"}, "rect": [0, 0, 1, 1]}],
  "canvas": {"width": 1000, "height": 600, "unit": "normalized"},
  "layer_order": ["places"],
  "layers": {"places": {"title": "地点", "visible_default": true, "locked": false}},
  "placements": {
    "lighthouse": {
      "layer_id": "places",
      "target_ref": {"kind": "world", "id": "harbor"},
      "geometry": {"kind": "point", "position": [0.4, 0.3]},
      "annotation": "查看灯塔",
      "role": "地点入口",
      "label_override": null,
      "navigation": null,
      "scope_refs": []
    }
  },
  "extensions": {}
}"#,
        )
        .expect("test map");
        root
    }

    #[test]
    fn pointer_zoom_keeps_anchor_without_dirtying_project() {
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = super::super::WorldeditApp::new(&creation, None);
        app.tab = super::super::Tab::Map;
        let source_before = app.project.sources();
        let dirty_before = app.project.is_dirty();
        let pointer = pos2(430.0, 300.0);

        let first = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))),
            events: vec![Event::PointerMoved(pointer)],
            ..Default::default()
        };
        let _ = ctx.run(first, |ctx| {
            app.map_tab(ctx);
        });
        let zoom_before = app.map_canvas.camera().zoom();
        let map_point_before = app
            .map_canvas
            .camera()
            .screen_to_normalized(pointer, app.map_canvas.viewport());

        let second = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0))),
            events: vec![
                Event::PointerMoved(pointer),
                Event::MouseWheel {
                    unit: MouseWheelUnit::Point,
                    delta: vec2(0.0, 120.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            ..Default::default()
        };
        let _ = ctx.run(second, |ctx| {
            app.map_tab(ctx);
        });

        assert!(app.map_canvas.camera().zoom() > zoom_before);
        let map_point_after = app
            .map_canvas
            .camera()
            .screen_to_normalized(pointer, app.map_canvas.viewport());
        assert!((map_point_after.x - map_point_before.x).abs() < 1e-4);
        assert!((map_point_after.y - map_point_before.y).abs() < 1e-4);
        assert_eq!(app.map_canvas.edit_intents().len(), 0);
        assert_eq!(app.project.sources(), source_before);
        assert_eq!(app.project.is_dirty(), dirty_before);
    }

    #[test]
    fn edit_canvas_hit_test_uses_screen_pixel_tolerance_for_non_square_extent() {
        let ctx = egui::Context::default();
        let mut canvas = MapCanvas::new(MapRenderSnapshot {
            map_id: String::new(),
            title: String::new(),
            extent: vec2(1000.0, 100.0),
            raster_layers: Vec::new(),
            layers: vec![MapLayer {
                id: "places".into(),
                visible: true,
                placements: vec![MapPlacement {
                    id: "point".into(),
                    target_ref: None,
                    annotation: String::new(),
                    role: String::new(),
                    label_override: None,
                    geometry: MapGeometry::Point(NormalizedPoint::new(0.5, 0.5)),
                    style: MapStyle::default(),
                }],
            }],
        });
        canvas.set_mode(CanvasMode::Edit);
        canvas.set_tool(CanvasTool::Select);

        let screen_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
        let exact = canvas
            .camera()
            .normalized_to_screen(pos2(0.5, 0.5), canvas.viewport());
        let near = exact + vec2(0.0, 8.0);
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                events: vec![
                    Event::PointerMoved(near),
                    Event::PointerButton {
                        pos: near,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                events: vec![
                    Event::PointerMoved(near),
                    Event::PointerButton {
                        pos: near,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
        assert_eq!(
            canvas.selected,
            Some(("point".into(), GeometryHit::Vertex(0)))
        );
    }

    #[test]
    fn edit_drag_keeps_preview_until_release_and_emits_one_intent() {
        let ctx = egui::Context::default();
        let mut canvas = MapCanvas::new(MapRenderSnapshot {
            map_id: String::new(),
            title: String::new(),
            extent: vec2(400.0, 400.0),
            raster_layers: Vec::new(),
            layers: vec![MapLayer {
                id: "places".into(),
                visible: true,
                placements: vec![MapPlacement {
                    id: "point".into(),
                    target_ref: None,
                    annotation: String::new(),
                    role: String::new(),
                    label_override: None,
                    geometry: MapGeometry::Point(NormalizedPoint::new(0.25, 0.25)),
                    style: MapStyle::default(),
                }],
            }],
        });
        canvas.set_mode(CanvasMode::Edit);
        canvas.set_tool(CanvasTool::Select);
        let screen_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
        let start = canvas
            .camera()
            .normalized_to_screen(pos2(0.25, 0.25), canvas.viewport());
        let first_move = start + vec2(40.0, 20.0);
        let second_move = start + vec2(80.0, 40.0);

        for (events, expected_intents) in [
            (
                vec![
                    Event::PointerMoved(start),
                    Event::PointerButton {
                        pos: start,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                0,
            ),
            (vec![Event::PointerMoved(first_move)], 0),
            (vec![Event::PointerMoved(second_move)], 0),
        ] {
            let _ = ctx.run(
                RawInput {
                    screen_rect: Some(screen_rect),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
                },
            );
            assert_eq!(canvas.edit_intents().len(), expected_intents);
        }
        assert!(canvas.draft.is_some());

        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                events: vec![
                    Event::PointerMoved(second_move),
                    Event::PointerButton {
                        pos: second_move,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
        assert_eq!(canvas.edit_intents().len(), 1);
        assert!(canvas.draft.is_none());
        assert!(matches!(
            canvas.edit_intents().first(),
            Some(EditIntent::Move {
                placement,
                geometry: MapGeometry::Point(point)
            }) if placement == "point" && point.x > 0.25 && point.y > 0.25
        ));
    }

    #[test]
    fn point_tool_rejects_clicks_in_the_margin_outside_the_map() {
        let ctx = egui::Context::default();
        let mut canvas = MapCanvas::new(MapRenderSnapshot::empty(vec2(400.0, 200.0)));
        canvas.set_mode(CanvasMode::Edit);
        canvas.set_tool(CanvasTool::Point);
        let screen_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
        let margin = canvas
            .camera()
            .normalized_to_screen(pos2(0.5, -0.1), canvas.viewport());
        assert!(canvas.viewport().contains(margin));
        click_canvas(&ctx, &mut canvas, screen_rect, margin, 1.0);
        assert!(canvas.edit_intents().is_empty(), "地图留白不能生成越界点");
        assert_eq!(canvas.validation_error(), Some("点必须位于地图范围内"));
    }

    #[test]
    fn dragging_a_control_point_outside_the_map_is_rejected() {
        let ctx = egui::Context::default();
        let mut canvas = MapCanvas::new(MapRenderSnapshot {
            map_id: "map".into(),
            title: "测试地图".into(),
            extent: vec2(400.0, 400.0),
            raster_layers: Vec::new(),
            layers: vec![MapLayer {
                id: "places".into(),
                visible: true,
                placements: vec![MapPlacement {
                    id: "point".into(),
                    target_ref: None,
                    annotation: String::new(),
                    role: String::new(),
                    label_override: None,
                    geometry: MapGeometry::Point(NormalizedPoint::new(0.25, 0.25)),
                    style: MapStyle::default(),
                }],
            }],
        });
        canvas.set_mode(CanvasMode::Edit);
        canvas.set_tool(CanvasTool::Select);
        let screen_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
        let start = canvas
            .camera()
            .normalized_to_screen(pos2(0.25, 0.25), canvas.viewport());
        let outside = canvas
            .camera()
            .normalized_to_screen(pos2(-0.1, 0.25), canvas.viewport());
        assert!(canvas.viewport().contains(outside));

        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                events: vec![
                    Event::PointerMoved(start),
                    Event::PointerButton {
                        pos: start,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                events: vec![Event::PointerMoved(outside)],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                events: vec![
                    Event::PointerMoved(outside),
                    Event::PointerButton {
                        pos: outside,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );

        assert!(canvas.edit_intents().is_empty());
        assert!(canvas.draft.is_none());
        assert_eq!(canvas.validation_error(), Some("点必须位于地图范围内"));
    }

    #[test]
    fn refreshing_a_map_clears_selection_and_draft_without_resetting_camera_or_layers() {
        let mut canvas = MapCanvas::new(MapRenderSnapshot {
            map_id: "map".into(),
            title: "测试地图".into(),
            extent: vec2(400.0, 400.0),
            raster_layers: Vec::new(),
            layers: vec![MapLayer {
                id: "places".into(),
                visible: true,
                placements: vec![MapPlacement {
                    id: "point".into(),
                    target_ref: None,
                    annotation: String::new(),
                    role: String::new(),
                    label_override: None,
                    geometry: MapGeometry::Point(NormalizedPoint::new(0.25, 0.25)),
                    style: MapStyle::default(),
                }],
            }],
        });
        canvas.camera.pan_by(vec2(12.0, -8.0));
        canvas.set_layer_visible("places", false);
        canvas.selected = Some(("point".into(), GeometryHit::Vertex(0)));
        canvas.drag = Some(DragState {
            placement: "point".into(),
            vertex: 0,
        });
        canvas.draft = Some(MapGeometry::Point(NormalizedPoint::new(0.3, 0.3)));
        let camera = *canvas.camera();
        canvas.set_snapshot(
            1,
            MapRenderSnapshot {
                map_id: "map".into(),
                title: "更新地图".into(),
                extent: vec2(400.0, 400.0),
                raster_layers: Vec::new(),
                layers: vec![MapLayer {
                    id: "places".into(),
                    visible: true,
                    placements: vec![MapPlacement {
                        id: "new-point".into(),
                        target_ref: None,
                        annotation: String::new(),
                        role: String::new(),
                        label_override: None,
                        geometry: MapGeometry::Point(NormalizedPoint::new(0.75, 0.75)),
                        style: MapStyle::default(),
                    }],
                }],
            },
        );

        assert!(canvas.selected.is_none());
        assert!(canvas.drag.is_none());
        assert!(canvas.draft.is_none());
        assert!(canvas.edit_intents().is_empty());
        assert_eq!(*canvas.camera(), camera);
        assert!(!canvas.layer_states()[0].1);
        assert_eq!(canvas.selected_placement(), None);
    }

    #[test]
    fn selection_uses_stable_placement_id_when_layer_order_changes() {
        let placement = |id: &str, x: f32| MapPlacement {
            id: id.into(),
            target_ref: None,
            annotation: String::new(),
            role: String::new(),
            label_override: None,
            geometry: MapGeometry::Point(NormalizedPoint::new(x, 0.5)),
            style: MapStyle::default(),
        };
        let mut canvas = MapCanvas::new(MapRenderSnapshot {
            map_id: "map".into(),
            title: "测试地图".into(),
            extent: vec2(400.0, 400.0),
            raster_layers: Vec::new(),
            layers: vec![MapLayer {
                id: "places".into(),
                visible: true,
                placements: vec![placement("first", 0.2), placement("second", 0.4)],
            }],
        });
        canvas.selected = Some(("second".into(), GeometryHit::Vertex(0)));
        canvas.set_snapshot(
            0,
            MapRenderSnapshot {
                map_id: "map".into(),
                title: "测试地图".into(),
                extent: vec2(400.0, 400.0),
                raster_layers: Vec::new(),
                layers: vec![MapLayer {
                    id: "places".into(),
                    visible: true,
                    placements: vec![
                        placement("inserted", 0.1),
                        placement("first", 0.2),
                        placement("second", 0.4),
                    ],
                }],
            },
        );

        assert_eq!(
            canvas.selected_placement().map(|placement| placement.id),
            Some("second".into())
        );
    }

    #[test]
    fn polygon_tool_draws_a_concave_shape_from_canvas_clicks() {
        let ctx = egui::Context::default();
        let mut canvas = MapCanvas::new(MapRenderSnapshot::empty(vec2(400.0, 400.0)));
        canvas.set_mode(CanvasMode::Edit);
        canvas.set_tool(CanvasTool::Polygon);
        let screen_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
        let points = [
            pos2(0.1, 0.1),
            pos2(0.9, 0.1),
            pos2(0.9, 0.4),
            pos2(0.5, 0.4),
            pos2(0.5, 0.9),
            pos2(0.1, 0.9),
        ];
        for (index, point) in points.iter().copied().enumerate() {
            let screen_point = canvas
                .camera()
                .normalized_to_screen(point, canvas.viewport());
            click_canvas(
                &ctx,
                &mut canvas,
                screen_rect,
                screen_point,
                index as f64 + 1.0,
            );
        }
        assert!(
            matches!(canvas.draft, Some(MapGeometry::Polygon(ref points)) if points.len() == 6)
        );

        let last = canvas
            .camera()
            .normalized_to_screen(points[5], canvas.viewport());
        click_canvas(&ctx, &mut canvas, screen_rect, last, 6.02);
        assert!(canvas.draft.is_none());
        assert!(matches!(
            canvas.edit_intents().first(),
            Some(EditIntent::Create(MapGeometry::Polygon(points))) if points.len() == 6
        ));
    }

    #[test]
    fn escape_cancels_a_local_geometry_draft() {
        let ctx = egui::Context::default();
        let mut canvas = MapCanvas::new(MapRenderSnapshot::empty(vec2(400.0, 400.0)));
        canvas.set_mode(CanvasMode::Edit);
        canvas.set_tool(CanvasTool::Polyline);
        let screen_rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(800.0, 600.0));
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
        let point = canvas
            .camera()
            .normalized_to_screen(pos2(0.2, 0.2), canvas.viewport());
        click_canvas(&ctx, &mut canvas, screen_rect, point, 1.0);
        assert!(canvas.draft.is_some());

        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                events: vec![Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );

        assert!(canvas.draft.is_none());
        assert!(canvas.edit_intents().is_empty());
    }

    fn click_canvas(
        ctx: &egui::Context,
        canvas: &mut MapCanvas,
        screen_rect: Rect,
        point: Pos2,
        time: f64,
    ) {
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                time: Some(time),
                events: vec![
                    Event::PointerMoved(point),
                    Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                time: Some(time + 0.01),
                events: vec![
                    Event::PointerMoved(point),
                    Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
    }
}
