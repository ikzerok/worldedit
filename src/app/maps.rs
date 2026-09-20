//! 地图画布与展示编辑。

mod camera;
mod geometry;
mod raster;

use camera::Camera2D;
use egui::{Color32, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Vec2};
use geometry::{
    hit_test, triangulate_polygon, GeometryError, GeometryHit, MapGeometry, NormalizedPoint,
};
use raster::RasterTextureCache;
use std::collections::{BTreeMap, HashMap, HashSet};
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
    pub(super) title: String,
    pub(super) visible: bool,
    pub(super) locked: bool,
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
            title: layer.title.clone(),
            visible: layer.visible_default,
            locked: layer.locked,
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

fn core_geometry(geometry: &MapGeometry) -> worldline_core::presentation::MapGeometry {
    match geometry {
        MapGeometry::Point(point) => {
            worldline_core::presentation::MapGeometry::point([point.x as f64, point.y as f64])
        }
        MapGeometry::Polyline(points) => worldline_core::presentation::MapGeometry::Polyline {
            points: points
                .iter()
                .map(|point| [point.x as f64, point.y as f64])
                .collect(),
        },
        MapGeometry::Polygon(points) => worldline_core::presentation::MapGeometry::Polygon {
            points: points
                .iter()
                .map(|point| [point.x as f64, point.y as f64])
                .collect(),
        },
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

#[derive(Default)]
pub(super) struct PlacementForm {
    pub(super) target: Option<TargetRef>,
    pub(super) target_query: String,
    pub(super) annotation: String,
    pub(super) role: String,
    pub(super) label_override: String,
    pub(super) editing_placement: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct LocateRequest {
    pub(super) map_id: String,
    pub(super) placement_id: String,
    pub(super) layer_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MapCommandBaseline {
    pub(super) revision: worldline_core::presentation_commands::Revision,
    pub(super) expected_documents: BTreeMap<PathBuf, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PendingMapCommand {
    pub(super) map_id: String,
    pub(super) intent: EditIntent,
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
    intent_baselines: Vec<Option<MapCommandBaseline>>,
    command_baseline: Option<MapCommandBaseline>,
    session_layer_visibility: HashMap<String, bool>,
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
    Delete {
        placement: String,
    },
}

#[derive(Clone, Debug)]
struct DragState {
    placement: String,
    vertex: usize,
    initial_geometry: MapGeometry,
    start_screen: Pos2,
    grab_offset: [f32; 2],
    baseline: Option<MapCommandBaseline>,
    active: bool,
}

const DRAG_THRESHOLD_PX: f32 = 3.0;

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
            intent_baselines: Vec::new(),
            command_baseline: None,
            session_layer_visibility: HashMap::new(),
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
        self.intent_baselines.clear();
        self.command_baseline = None;
        self.session_layer_visibility.clear();
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

    pub(super) fn set_snapshot(&mut self, source_version: u64, snapshot: MapRenderSnapshot) {
        let same_map = self.snapshot.map_id == snapshot.map_id
            && self.snapshot.extent == snapshot.extent
            && !snapshot.map_id.is_empty();
        let same_source = self.source_version == source_version;
        let preserve_local = same_map
            && !same_source
            && (self.drag.is_some() || self.draft.is_some() || !self.edit_intents.is_empty());
        let preserved_geometry = if preserve_local {
            self.selected.as_ref().and_then(|(placement_id, _)| {
                self.snapshot
                    .layers
                    .iter()
                    .flat_map(|layer| layer.placements.iter())
                    .find(|placement| placement.id == *placement_id)
                    .map(|placement| (placement_id.clone(), placement.geometry.clone()))
            })
        } else {
            None
        };
        if !same_map {
            self.camera = Camera2D::new(snapshot.extent);
            self.fit_pending = true;
            self.selected = None;
            self.drag = None;
            self.draft = None;
            self.edit_intents.clear();
            self.intent_baselines.clear();
            self.command_baseline = None;
            self.last_error = None;
            self.raster_errors.clear();
            self.raster_attempts.clear();
            self.session_layer_visibility.clear();
        } else {
            if !same_source && !preserve_local {
                self.selected = None;
                self.drag = None;
                self.draft = None;
                self.edit_intents.clear();
                self.intent_baselines.clear();
                self.last_error = None;
            }
            let layer_ids = snapshot
                .layers
                .iter()
                .map(|layer| layer.id.as_str())
                .collect::<HashSet<_>>();
            self.session_layer_visibility
                .retain(|id, _| layer_ids.contains(id.as_str()));
        }

        // `core_snapshot` represents the persisted defaults and content. The
        // display snapshot may include a local drag preview and temporary
        // browse visibility overrides, so keep that overlay out of the core
        // copy used by Undo/cancel.
        let core_snapshot = snapshot.clone();
        let mut display_snapshot = snapshot;
        if let Some((placement_id, geometry)) = preserved_geometry {
            for layer in &mut display_snapshot.layers {
                if let Some(placement) = layer
                    .placements
                    .iter_mut()
                    .find(|placement| placement.id == placement_id)
                {
                    placement.geometry = geometry.clone();
                }
            }
        }
        for layer in &mut display_snapshot.layers {
            if let Some(visible) = self.session_layer_visibility.get(&layer.id) {
                layer.visible = *visible;
            }
        }
        self.source_version = source_version;
        self.core_snapshot = core_snapshot;
        self.snapshot = display_snapshot;
    }

    #[cfg(test)]
    pub(super) fn layer_states(&self) -> Vec<(String, bool, usize)> {
        self.snapshot
            .layers
            .iter()
            .map(|layer| (layer.id.clone(), layer.visible, layer.placements.len()))
            .collect()
    }

    pub(super) fn layer_details(&self) -> Vec<(String, String, bool, bool, usize)> {
        self.snapshot
            .layers
            .iter()
            .map(|layer| {
                (
                    layer.id.clone(),
                    layer.title.clone(),
                    layer.visible,
                    layer.locked,
                    layer.placements.len(),
                )
            })
            .collect()
    }

    pub(super) fn layer_order(&self) -> Vec<String> {
        self.snapshot
            .layers
            .iter()
            .map(|layer| layer.id.clone())
            .collect()
    }

    pub(super) fn layer_default_visibility(&self, id: &str) -> Option<bool> {
        self.core_snapshot
            .layers
            .iter()
            .find(|layer| layer.id == id)
            .map(|layer| layer.visible)
    }

    pub(super) fn set_layer_visible(&mut self, id: &str, visible: bool) {
        if let Some(layer) = self.snapshot.layers.iter_mut().find(|layer| layer.id == id) {
            self.session_layer_visibility.insert(id.to_owned(), visible);
            layer.visible = visible;
            if !visible {
                self.selected = None;
            }
        }
    }

    pub(super) fn set_layer_default_visible(&mut self, id: &str, visible: bool) {
        self.session_layer_visibility.remove(id);
        if let Some(layer) = self
            .core_snapshot
            .layers
            .iter_mut()
            .find(|layer| layer.id == id)
        {
            layer.visible = visible;
        }
        if let Some(layer) = self.snapshot.layers.iter_mut().find(|layer| layer.id == id) {
            layer.visible = visible;
            if !visible {
                self.selected = None;
            }
        }
    }

    pub(super) fn set_command_baseline(&mut self, baseline: Option<MapCommandBaseline>) {
        self.command_baseline = baseline;
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

    pub(super) fn select_placement_id(&mut self, id: &str) -> bool {
        let Some(placement) = self
            .snapshot
            .layers
            .iter()
            .filter(|layer| layer.visible)
            .flat_map(|layer| layer.placements.iter())
            .find(|placement| placement.id == id)
        else {
            return false;
        };
        self.selected = Some((placement.id.clone(), GeometryHit::Body));
        true
    }

    pub(super) fn placement_layer(&self, id: &str) -> Option<(String, bool, bool)> {
        self.snapshot.layers.iter().find_map(|layer| {
            layer
                .placements
                .iter()
                .find(|placement| placement.id == id)
                .map(|_| (layer.id.clone(), layer.visible, layer.locked))
        })
    }

    pub(super) fn reveal_layer_for_session(&mut self, id: &str) -> bool {
        let Some(layer) = self.snapshot.layers.iter_mut().find(|layer| layer.id == id) else {
            return false;
        };
        self.session_layer_visibility.insert(id.to_owned(), true);
        layer.visible = true;
        true
    }

    pub(super) fn is_edit_mode(&self) -> bool {
        self.mode == CanvasMode::Edit
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

    pub(super) fn take_edit_batch(&mut self) -> (Vec<EditIntent>, Vec<Option<MapCommandBaseline>>) {
        (
            std::mem::take(&mut self.edit_intents),
            std::mem::take(&mut self.intent_baselines),
        )
    }

    pub(super) fn restore_failed_preview(&mut self, intent: &EditIntent) {
        match intent {
            EditIntent::Create(geometry) => {
                self.draft = Some(geometry.clone());
            }
            EditIntent::Move {
                placement,
                geometry,
            } => {
                if let Some(existing) = self
                    .snapshot
                    .layers
                    .iter_mut()
                    .flat_map(|layer| layer.placements.iter_mut())
                    .find(|existing| existing.id == *placement)
                {
                    existing.geometry = geometry.clone();
                    self.selected = Some((placement.clone(), GeometryHit::Body));
                    self.draft = Some(geometry.clone());
                }
            }
            EditIntent::Delete { .. } => {}
        }
    }

    pub(super) fn reset_local_preview(&mut self) {
        self.snapshot = self.core_snapshot.clone();
        for layer in &mut self.snapshot.layers {
            if let Some(visible) = self.session_layer_visibility.get(&layer.id) {
                layer.visible = *visible;
            }
        }
        self.selected = None;
        self.drag = None;
        self.draft = None;
        self.edit_intents.clear();
        self.intent_baselines.clear();
        self.command_baseline = None;
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
            ui.label("地图展示");
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
                ui.label(crate::theme::muted("编辑展示，仅修改地图标记和图层"));
                let previous_tool = self.tool;
                ui.selectable_value(&mut self.tool, CanvasTool::Select, "选择")
                    .on_hover_text("选择标记并拖动控制点，释放后提交一个展示命令");
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
                if ui.small_button("放弃未提交修改").clicked() {
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

    fn handle_edit_input(&mut self, response: &egui::Response, ui: &egui::Ui) {
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

    fn finish_drag(&mut self) {
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
                self.push_intent(intent);
                true
            }
            Err(error) => {
                self.last_error = Some(geometry_error_message(error));
                false
            }
        }
    }

    fn submit_intent_with_baseline(
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

    fn push_intent(&mut self, intent: EditIntent) {
        self.edit_intents.push(intent);
        self.intent_baselines.push(self.command_baseline.clone());
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

fn geometry_vertex(geometry: &MapGeometry, index: usize) -> Option<NormalizedPoint> {
    match geometry {
        MapGeometry::Point(point) if index == 0 => Some(*point),
        MapGeometry::Polyline(points) | MapGeometry::Polygon(points) => points.get(index).copied(),
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
    fn map_command_baseline(&self, map_id: &str) -> Option<MapCommandBaseline> {
        let path =
            worldline_core::presentation_commands::map_document_path(&self.project, map_id).ok()?;
        let document = self.project.authoring_document(&path).ok()?;
        let mut expected_documents = std::collections::BTreeMap::new();
        expected_documents.insert(
            path,
            worldline_core::presentation_commands::document_hash(document.bytes()),
        );
        Some(MapCommandBaseline {
            revision: self.map_revision,
            expected_documents,
        })
    }

    fn apply_map_command(
        &mut self,
        map_id: &str,
        command: worldline_core::presentation_commands::Command,
        label: &str,
    ) -> bool {
        self.apply_map_command_with_baseline(map_id, command, label, None)
    }

    fn apply_map_command_with_baseline(
        &mut self,
        map_id: &str,
        command: worldline_core::presentation_commands::Command,
        label: &str,
        baseline: Option<MapCommandBaseline>,
    ) -> bool {
        if !self.map_canvas.is_edit_mode() {
            self.io_error = Some("请先进入编辑展示模式".into());
            return false;
        }
        let path =
            match worldline_core::presentation_commands::map_document_path(&self.project, map_id) {
                Ok(path) => path,
                Err(error) => {
                    self.io_error = Some(error.to_string());
                    return false;
                }
            };
        let baseline = baseline
            .or_else(|| self.map_canvas.command_baseline.clone())
            .or_else(|| {
                let expected = self
                    .project
                    .authoring_document(&path)
                    .ok()
                    .map(|document| {
                        worldline_core::presentation_commands::document_hash(document.bytes())
                    })?;
                let mut expected_documents = std::collections::BTreeMap::new();
                expected_documents.insert(path.clone(), expected);
                Some(MapCommandBaseline {
                    revision: self.map_revision,
                    expected_documents,
                })
            });
        let Some(baseline) = baseline else {
            self.io_error = Some("无法读取地图文档基线".into());
            return false;
        };
        let before = self.project.clone();
        let envelope = worldline_core::presentation_commands::CommandEnvelope {
            expected_revision: baseline.revision,
            expected_documents: baseline.expected_documents,
            command,
        };
        let content = self.snapshot.as_ref().map(|snapshot| &snapshot.result);
        let result = match content {
            Some(content) => worldline_core::presentation_commands::apply_with_content(
                &mut self.project,
                &mut self.map_revision,
                envelope,
                content,
            ),
            None => worldline_core::presentation_commands::apply(
                &mut self.project,
                &mut self.map_revision,
                envelope,
            ),
        };
        match result {
            Ok(result) => {
                self.remember(before);
                self.map_canvas.reset_local_preview();
                self.refresh_presentation_after_map_command();
                self.io_error = None;
                self.message = Some(label.into());
                self.map_failed_command = None;
                debug_assert_eq!(self.map_revision, result.new_revision);
                true
            }
            Err(error) => {
                self.io_error = Some(error.to_string());
                false
            }
        }
    }

    fn remember_failed_map_command(&mut self, map_id: &str, intent: EditIntent) {
        self.map_failed_command = Some(PendingMapCommand {
            map_id: map_id.to_owned(),
            intent,
        });
    }

    fn retry_failed_map_command(&mut self) {
        let Some(pending) = self.map_failed_command.take() else {
            return;
        };
        if !self.map_canvas.is_edit_mode() {
            self.io_error = Some("请先进入编辑展示模式，再按当前版本重试提交".into());
            self.map_failed_command = Some(pending);
            return;
        }
        let Some(current_map_id) = self.map_selection.as_deref() else {
            self.io_error = Some("当前没有选中的地图，无法重试提交".into());
            self.map_failed_command = Some(pending);
            return;
        };
        if current_map_id != pending.map_id {
            self.io_error = Some(format!(
                "待重试命令属于地图“{}”，请返回该地图后按当前版本重试提交",
                pending.map_id
            ));
            self.map_failed_command = Some(pending);
            return;
        }
        let Some(baseline) = self.map_command_baseline(&pending.map_id) else {
            self.io_error = Some("无法读取当前地图文档基线，暂不能重试提交".into());
            self.map_failed_command = Some(pending);
            return;
        };
        self.message = Some("按当前地图版本重新检查并提交展示预览".into());
        self.apply_map_intents(vec![pending.intent], vec![Some(baseline)]);
    }

    fn apply_map_intents(
        &mut self,
        intents: Vec<EditIntent>,
        baselines: Vec<Option<MapCommandBaseline>>,
    ) {
        let Some(map_id) = self.map_selection.clone() else {
            return;
        };
        for (index, intent) in intents.into_iter().enumerate() {
            let baseline = baselines.get(index).cloned().unwrap_or(None);
            match intent {
                EditIntent::Move {
                    placement,
                    geometry,
                } => {
                    let pending = EditIntent::Move {
                        placement: placement.clone(),
                        geometry: geometry.clone(),
                    };
                    if !self.apply_map_command_with_baseline(
                        &map_id,
                        worldline_core::presentation_commands::Command::UpdatePlacement {
                            map_id: map_id.clone(),
                            placement_id: placement.clone(),
                            geometry: Some(core_geometry(&geometry)),
                            target_ref: None,
                            annotation: None,
                            role: None,
                            label_override: None,
                            layer_id: None,
                        },
                        "已保存标记位置（可撤销）",
                        baseline.clone(),
                    ) {
                        self.map_canvas.restore_failed_preview(&pending);
                        self.remember_failed_map_command(&map_id, pending);
                        break;
                    }
                }
                EditIntent::Create(geometry) => {
                    let pending = EditIntent::Create(geometry.clone());
                    let Some(layer_id) = self
                        .map_canvas
                        .layer_details()
                        .into_iter()
                        .find(|(_, _, visible, locked, _)| *visible && !*locked)
                        .map(|(id, _, _, _, _)| id)
                    else {
                        self.io_error = Some("当前没有可编辑的未锁定图层".into());
                        self.map_canvas.restore_failed_preview(&pending);
                        self.remember_failed_map_command(&map_id, pending);
                        break;
                    };
                    let placement_id = self.next_map_placement_id(&map_id);
                    let annotation = if self.map_form.annotation.trim().is_empty() {
                        "地图标记".into()
                    } else {
                        self.map_form.annotation.trim().into()
                    };
                    let role = if self.map_form.role.trim().is_empty() {
                        "说明".into()
                    } else {
                        self.map_form.role.trim().into()
                    };
                    let label_override = (!self.map_form.label_override.trim().is_empty())
                        .then(|| self.map_form.label_override.trim().to_owned());
                    if !self.apply_map_command_with_baseline(
                        &map_id,
                        worldline_core::presentation_commands::Command::CreatePlacement {
                            map_id: map_id.clone(),
                            placement_id,
                            layer_id,
                            target_ref: self.map_form.target.clone(),
                            geometry: core_geometry(&geometry),
                            annotation,
                            role,
                            label_override,
                        },
                        "已保存地图标记",
                        baseline.clone(),
                    ) {
                        self.map_canvas.restore_failed_preview(&pending);
                        self.remember_failed_map_command(&map_id, pending);
                        break;
                    }
                }
                EditIntent::Delete { placement } => {
                    let pending = EditIntent::Delete {
                        placement: placement.clone(),
                    };
                    if !self.apply_map_command_with_baseline(
                        &map_id,
                        worldline_core::presentation_commands::Command::DeletePlacement {
                            map_id: map_id.clone(),
                            placement_id: placement,
                        },
                        "已删除地图标记（可撤销）",
                        baseline.clone(),
                    ) {
                        self.remember_failed_map_command(&map_id, pending);
                        break;
                    }
                    self.map_form.editing_placement = None;
                }
            }
        }
    }

    fn next_map_placement_id(&self, map_id: &str) -> String {
        let mut index = 1;
        let existing = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.map_index.maps.get(map_id));
        while existing.is_some_and(|map| map.placements.contains_key(&format!("marker_{index}"))) {
            index += 1;
        }
        format!("marker_{index}")
    }

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
            if let Some(request) = self.map_locate_request.clone() {
                if request.map_id == map_id {
                    if let Some((_, visible, _)) =
                        self.map_canvas.placement_layer(&request.placement_id)
                    {
                        if visible {
                            self.map_canvas.select_placement_id(&request.placement_id);
                            self.map_locate_request = None;
                        }
                    }
                }
            }
        } else if !self.map_canvas.map_id().is_empty()
            || !self.map_canvas.snapshot.layers.is_empty()
            || !self.map_canvas.snapshot.raster_layers.is_empty()
        {
            self.map_canvas.clear();
        }

        let command_baseline = if self.map_canvas.is_edit_mode() {
            selected_map_id
                .as_deref()
                .and_then(|map_id| self.map_command_baseline(map_id))
        } else {
            None
        };
        self.map_canvas.set_command_baseline(command_baseline);

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
                    let editing = self.map_canvas.is_edit_mode();
                    ui.label(crate::theme::muted(if editing {
                        "编辑展示：显隐默认会写入展示文档；锁定和顺序也会保存。"
                    } else {
                        "浏览模式：显隐只作用于本次浏览；进入编辑展示后才能保存图层设置。"
                    }));
                    let layer_details = self.map_canvas.layer_details();
                    for (index, (id, title, visible, locked, count)) in
                        layer_details.iter().enumerate()
                    {
                        let mut next = if editing {
                            self.map_canvas
                                .layer_default_visibility(id)
                                .unwrap_or(*visible)
                        } else {
                            *visible
                        };
                        ui.horizontal(|ui| {
                            if ui
                                .checkbox(
                                    &mut next,
                                    if editing {
                                        format!("{}  ·  默认可见 · {} 个标记", title, count)
                                    } else {
                                        format!("{}  ·  临时显示 · {} 个标记", title, count)
                                    },
                                )
                                .changed()
                            {
                                if editing {
                                    let applied = self.apply_map_command(
                                        selected_map_id.as_deref().unwrap_or_default(),
                                        worldline_core::presentation_commands::Command::SetLayer {
                                            map_id: selected_map_id.clone().unwrap_or_default(),
                                            layer_id: id.clone(),
                                            title: None,
                                            visible_default: Some(next),
                                            locked: None,
                                            layer_order: None,
                                        },
                                        "已保存图层默认显隐",
                                    );
                                    if applied {
                                        self.map_canvas.set_layer_default_visible(id, next);
                                    }
                                } else {
                                    self.map_canvas.set_layer_visible(id, next);
                                }
                            }
                            if editing {
                                let lock_label = if *locked { "🔒" } else { "🔓" };
                                if ui
                                    .small_button(lock_label)
                                    .on_hover_text(if *locked { "解锁图层" } else { "锁定图层" })
                                    .clicked()
                                {
                                    let _ = self.apply_map_command(
                                        selected_map_id.as_deref().unwrap_or_default(),
                                        worldline_core::presentation_commands::Command::SetLayer {
                                            map_id: selected_map_id.clone().unwrap_or_default(),
                                            layer_id: id.clone(),
                                            title: None,
                                            visible_default: None,
                                            locked: Some(!locked),
                                            layer_order: None,
                                        },
                                        if *locked { "已解锁图层" } else { "已锁定图层" },
                                    );
                                }
                                if index > 0
                                    && ui.small_button("↑").on_hover_text("上移图层").clicked()
                                {
                                    let mut order = self.map_canvas.layer_order();
                                    order.swap(index, index - 1);
                                    let _ = self.apply_map_command(
                                        selected_map_id.as_deref().unwrap_or_default(),
                                        worldline_core::presentation_commands::Command::SetLayer {
                                            map_id: selected_map_id.clone().unwrap_or_default(),
                                            layer_id: id.clone(),
                                            title: None,
                                            visible_default: None,
                                            locked: None,
                                            layer_order: Some(order),
                                        },
                                        "已调整图层顺序",
                                    );
                                }
                                if index + 1 < layer_details.len()
                                    && ui.small_button("↓").on_hover_text("下移图层").clicked()
                                {
                                    let mut order = self.map_canvas.layer_order();
                                    order.swap(index, index + 1);
                                    let _ = self.apply_map_command(
                                        selected_map_id.as_deref().unwrap_or_default(),
                                        worldline_core::presentation_commands::Command::SetLayer {
                                            map_id: selected_map_id.clone().unwrap_or_default(),
                                            layer_id: id.clone(),
                                            title: None,
                                            visible_default: None,
                                            locked: None,
                                            layer_order: Some(order),
                                        },
                                        "已调整图层顺序",
                                    );
                                }
                            }
                        });
                    }

                    if let Some(request) = self.map_locate_request.clone() {
                        if request.map_id == selected_map_id.as_deref().unwrap_or_default() {
                            ui.separator();
                            ui.colored_label(crate::theme::GOLD, "命中对象位于隐藏图层");
                            ui.label(crate::theme::muted(format!(
                                "图层 `{}` 当前隐藏。是否临时显示以定位？",
                                request.layer_id
                            )));
                            ui.horizontal(|ui| {
                                if ui.button("临时显示并定位").clicked() {
                                    self.map_canvas
                                        .reveal_layer_for_session(&request.layer_id);
                                    self.map_canvas.select_placement_id(&request.placement_id);
                                    self.map_locate_request = None;
                                }
                                if ui.small_button("取消").clicked() {
                                    self.map_locate_request = None;
                                }
                            });
                        }
                    }

                    ui.separator();
                    ui.collapsing("对象反查", |ui| {
                        ui.label(crate::theme::muted("按对象名称或 ID 查找其地图标记。"));
                        ui.text_edit_singleline(&mut self.map_search);
                        let query = self.map_search.trim().to_lowercase();
                        if !query.is_empty() {
                            let matches = self
                                .snapshot
                                .as_ref()
                                .map(|snapshot| {
                                    snapshot
                                        .result
                                        .analysis
                                        .catalog
                                        .objects
                                        .iter()
                                        .filter(|object| {
                                            object.display.to_lowercase().contains(&query)
                                                || object.target.id.to_lowercase().contains(&query)
                                                || object.target.kind.to_lowercase().contains(&query)
                                        })
                                        .take(20)
                                        .cloned()
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default();
                            if matches.is_empty() {
                                ui.label(crate::theme::muted("没有匹配对象。"));
                            }
                            for object in matches {
                                let placements = self
                                    .snapshot
                                    .as_ref()
                                    .map(|snapshot| {
                                        snapshot.map_index.placements_for(&object.target)
                                    })
                                    .unwrap_or_default();
                                ui.label(format!(
                                    "{}  ·  {}:{}",
                                    object.display, object.target.kind, object.target.id
                                ));
                                if placements.is_empty() {
                                    ui.label(crate::theme::muted("  未放置在地图上"));
                                }
                                for placement in placements {
                                    if ui
                                        .small_button(format!(
                                            "  定位 {} / {}",
                                            placement.map_id, placement.placement_id
                                        ))
                                        .clicked()
                                    {
                                        self.locate_reference(&placement.map_id, &placement.placement_id);
                                    }
                                }
                            }
                        }
                    });

                    if self.map_canvas.is_edit_mode() {
                        ui.separator();
                        ui.label(egui::RichText::new("标记编辑").strong());
                        ui.label(crate::theme::muted(
                            "选择已有对象可保持引用；留空则创建说明标记。",
                        ));
                        ui.text_edit_singleline(&mut self.map_form.target_query);
                        let target_query = self.map_form.target_query.trim().to_lowercase();
                        if !target_query.is_empty() {
                            let candidates = self
                                .snapshot
                                .as_ref()
                                .map(|snapshot| {
                                    snapshot
                                        .result
                                        .analysis
                                        .catalog
                                        .objects
                                        .iter()
                                        .filter(|object| {
                                            object.display.to_lowercase().contains(&target_query)
                                                || object.target.id.to_lowercase().contains(&target_query)
                                        })
                                        .take(8)
                                        .cloned()
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default();
                            for object in candidates {
                                if ui
                                    .small_button(format!(
                                        "{}  ·  {}:{}",
                                        object.display, object.target.kind, object.target.id
                                    ))
                                    .clicked()
                                {
                                    self.map_form.target = Some(object.target);
                                    self.map_form.target_query.clear();
                                }
                            }
                        }
                        if let Some(target) = self.map_form.target.clone() {
                            ui.horizontal(|ui| {
                                ui.label(format!("已选：{}:{}", target.kind, target.id));
                                if ui.small_button("清除引用").clicked() {
                                    self.map_form.target = None;
                                }
                            });
                        }
                        ui.label("说明");
                        ui.text_edit_singleline(&mut self.map_form.annotation);
                        ui.label("role");
                        ui.text_edit_singleline(&mut self.map_form.role);
                        ui.label("自定义标签（可选）");
                        ui.text_edit_singleline(&mut self.map_form.label_override);
                        if let Some(selected) = selected_placement.as_ref() {
                            if ui.small_button("载入当前标记到表单").clicked() {
                                self.map_form.editing_placement = Some(selected.id.clone());
                                self.map_form.target = selected.target_ref.clone();
                                self.map_form.annotation = selected.annotation.clone();
                                self.map_form.role = selected.role.clone();
                                self.map_form.label_override =
                                    selected.label_override.clone().unwrap_or_default();
                            }
                        }
                        if let Some(placement_id) = self.map_form.editing_placement.clone() {
                            if ui.button("保存当前标记说明").clicked() {
                                let _ = self.apply_map_command(
                                    selected_map_id.as_deref().unwrap_or_default(),
                                    worldline_core::presentation_commands::Command::UpdatePlacement {
                                        map_id: selected_map_id.clone().unwrap_or_default(),
                                        placement_id: placement_id.clone(),
                                        geometry: None,
                                        target_ref: Some(self.map_form.target.clone()),
                                        annotation: Some(self.map_form.annotation.clone()),
                                        role: Some(self.map_form.role.clone()),
                                        label_override: Some(
                                            (!self.map_form.label_override.trim().is_empty())
                                                .then(|| self.map_form.label_override.clone()),
                                        ),
                                        layer_id: None,
                                    },
                                    "已保存标记说明",
                                );
                            }
                            if ui.button("删除标记（资料仍保留）").clicked() {
                                let _ = self.apply_map_command(
                                    selected_map_id.as_deref().unwrap_or_default(),
                                    worldline_core::presentation_commands::Command::DeletePlacement {
                                        map_id: selected_map_id.clone().unwrap_or_default(),
                                        placement_id,
                                    },
                                    "已删除地图标记，资料仍保留",
                                );
                                self.map_form.editing_placement = None;
                            }
                        }
                    }

                    if !self.map_canvas.raster_states().is_empty() {
                        ui.separator();
                        ui.label(egui::RichText::new("栅格图层").strong());
                        for (asset, available, error) in self.map_canvas.raster_states() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(&asset);
                                if available && error.is_none() {
                                    ui.colored_label(crate::theme::ACCENT, "已加载");
                                } else {
                                    ui.colored_label(crate::theme::GOLD, "不可用");
                                }
                            });
                            let unavailable = !available || error.is_some();
                            if let Some(error) = error.as_deref() {
                                ui.label(crate::theme::muted(error));
                            }
                            if unavailable
                                && ui.small_button("打开地图文档修复引用").clicked()
                            {
                                if let Some(map_id) = selected_map_id.as_deref() {
                                    match worldline_core::presentation_commands::map_document_path(
                                        &self.project,
                                        map_id,
                                    ) {
                                        Ok(path) => {
                                            let file = path.to_string_lossy().into_owned();
                                            self.jump_to_file(&file, 1, 1);
                                        }
                                        Err(error) => self.io_error = Some(error.to_string()),
                                    }
                                }
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
                            let target_resolved = self.snapshot.as_ref().is_some_and(|snapshot| {
                                snapshot.result.analysis.catalog.object(&target).is_some()
                            });
                            if !target_resolved {
                                ui.colored_label(crate::theme::GOLD, "对象引用未解析");
                                if ui.small_button("编辑展示并重绑定").clicked() {
                                    self.map_canvas.set_mode(CanvasMode::Edit);
                                    self.map_form.editing_placement = Some(placement.id.clone());
                                    self.map_form.target = None;
                                    self.map_form.target_query.clear();
                                    self.map_form.annotation = placement.annotation.clone();
                                    self.map_form.role = placement.role.clone();
                                    self.map_form.label_override = placement
                                        .label_override
                                        .clone()
                                        .unwrap_or_default();
                                }
                            }
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
                                ui.horizontal(|ui| {
                                    ui.label(crate::theme::muted(diagnostic.file.clone()));
                                    if ui.small_button("打开原文").clicked() {
                                        self.jump_to_file(
                                            &diagnostic.file,
                                            diagnostic.span.line,
                                            diagnostic.span.column,
                                        );
                                    }
                                });
                            }
                        });
                }
            });

        let mut retry_failed = false;
        let mut cancel_failed = false;
        egui::CentralPanel::default()
            .frame(crate::theme::panel().fill(crate::theme::BG))
            .show(ctx, |ui| {
                self.page_heading(ui, "地图画布", "查看和编辑注册地图、图层和标记。");
                if !self.map_canvas.map_title().is_empty() {
                    ui.label(egui::RichText::new(self.map_canvas.map_title()).strong());
                }
                self.map_canvas.toolbar(ui);
                if self.map_failed_command.is_some() {
                    ui.separator();
                    ui.colored_label(
                        crate::theme::GOLD,
                        "展示命令未提交，当前预览仍保留；重试会按当前地图版本重新检查。",
                    );
                    ui.horizontal(|ui| {
                        if self.map_canvas.is_edit_mode() {
                            if ui.button("按当前版本重试提交").clicked() {
                                retry_failed = true;
                            }
                        } else {
                            ui.label(crate::theme::muted("进入编辑展示后才能重试提交。"));
                        }
                        if ui.small_button("取消预览").clicked() {
                            cancel_failed = true;
                        }
                    });
                }
                ui.separator();
                self.map_canvas.show(ui);
            });

        if retry_failed {
            self.retry_failed_map_command();
        } else if cancel_failed {
            self.map_failed_command = None;
            self.map_canvas.reset_local_preview();
        }
        let (intents, baselines) = self.map_canvas.take_edit_batch();
        self.apply_map_intents(intents, baselines);
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

    fn point_canvas() -> MapCanvas {
        MapCanvas::new(MapRenderSnapshot {
            map_id: "map".into(),
            title: "测试地图".into(),
            extent: vec2(400.0, 400.0),
            raster_layers: Vec::new(),
            layers: vec![MapLayer {
                id: "places".into(),
                title: "地点".into(),
                visible: true,
                locked: false,
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
        })
    }

    #[test]
    fn clicking_an_existing_marker_selects_without_creating_history() {
        let ctx = egui::Context::default();
        let mut canvas = point_canvas();
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
        let point = canvas
            .camera()
            .normalized_to_screen(pos2(0.25, 0.25), canvas.viewport());
        click_canvas(&ctx, &mut canvas, screen_rect, point, 1.0);
        assert_eq!(
            canvas.selected,
            Some(("point".into(), GeometryHit::Vertex(0)))
        );
        assert!(canvas.edit_intents().is_empty());
        assert!(canvas.draft.is_none());
    }

    #[test]
    fn pointer_release_outside_canvas_finishes_drag_without_stale_capture() {
        let ctx = egui::Context::default();
        let mut canvas = point_canvas();
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
        let moved = start + vec2(24.0, 16.0);
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
                events: vec![Event::PointerMoved(moved)],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| canvas.show(ui));
            },
        );
        assert!(canvas.drag.is_some());
        let outside = pos2(-20.0, moved.y);
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

        assert!(canvas.drag.is_none());
        assert!(canvas.draft.is_none());
        assert!(matches!(
            canvas.edit_intents().first(),
            Some(EditIntent::Move { placement, .. }) if placement == "point"
        ));
    }

    #[test]
    fn delete_key_queues_placement_delete_only_in_edit_mode() {
        let ctx = egui::Context::default();
        let mut canvas = point_canvas();
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
        let point = canvas
            .camera()
            .normalized_to_screen(pos2(0.25, 0.25), canvas.viewport());
        click_canvas(&ctx, &mut canvas, screen_rect, point, 1.0);
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(screen_rect),
                events: vec![Event::Key {
                    key: egui::Key::Delete,
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
        assert!(matches!(
            canvas.edit_intents().first(),
            Some(EditIntent::Delete { placement }) if placement == "point"
        ));
    }

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
    fn browse_mode_clicks_and_commands_do_not_write_or_create_history() {
        let root = test_workspace("browse-read-only");
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
        app.tab = super::super::Tab::Map;
        let frame = || RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
            ..Default::default()
        };
        let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));
        assert!(!app.map_canvas.is_edit_mode());
        let source_before = app.project.sources();
        let dirty_before = app.project.is_dirty();
        let history_before = app.history.len();
        let revision_before = app.map_revision;

        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
                events: vec![
                    Event::PointerMoved(pos2(1000.0, 150.0)),
                    Event::PointerButton {
                        pos: pos2(1000.0, 150.0),
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                ..Default::default()
            },
            |ctx| app.map_tab(ctx),
        );
        let _ = ctx.run(frame(), |ctx| app.map_tab(ctx));

        let applied = app.apply_map_command(
            "harbor",
            worldline_core::presentation_commands::Command::SetLayer {
                map_id: "harbor".into(),
                layer_id: "places".into(),
                title: None,
                visible_default: None,
                locked: Some(true),
                layer_order: None,
            },
            "不应写入",
        );
        assert!(!applied);
        assert_eq!(app.project.sources(), source_before);
        assert_eq!(app.project.is_dirty(), dirty_before);
        assert_eq!(app.history.len(), history_before);
        assert_eq!(app.map_revision, revision_before);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_conflicted_drag_keeps_its_preview_for_retry_or_cancel() {
        let root = test_workspace("map-conflict-preview");
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
        app.tab = super::super::Tab::Map;
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
                ..Default::default()
            },
            |ctx| app.map_tab(ctx),
        );
        app.map_canvas.set_mode(CanvasMode::Edit);
        let source_before = app.project.sources();
        let pending = EditIntent::Move {
            placement: "lighthouse".into(),
            geometry: MapGeometry::Point(NormalizedPoint::new(0.7, 0.8)),
        };
        app.apply_map_intents(
            vec![pending],
            vec![Some(MapCommandBaseline {
                revision: worldline_core::presentation_commands::Revision::default(),
                expected_documents: BTreeMap::new(),
            })],
        );
        assert!(app.map_failed_command.is_some());
        assert_eq!(app.project.sources(), source_before);
        assert!(app.map_canvas.draft.is_some());
        assert_eq!(
            app.map_canvas
                .selected_placement()
                .map(|placement| placement.geometry),
            Some(MapGeometry::Point(NormalizedPoint::new(0.7, 0.8)))
        );
        app.map_canvas.reset_local_preview();
        app.map_failed_command = None;
        assert!(app.map_canvas.draft.is_none());
        assert_eq!(app.project.sources(), source_before);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stale_preview_retries_against_current_revision_once() {
        let root = test_workspace("map-stale-retry");
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
        app.tab = super::super::Tab::Map;
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
                ..Default::default()
            },
            |ctx| app.map_tab(ctx),
        );
        app.map_canvas.set_mode(CanvasMode::Edit);
        let pending = EditIntent::Move {
            placement: "lighthouse".into(),
            geometry: MapGeometry::Point(NormalizedPoint::new(0.7, 0.8)),
        };
        let stale = MapCommandBaseline {
            revision: worldline_core::presentation_commands::Revision::default(),
            expected_documents: BTreeMap::new(),
        };
        let map_path =
            worldline_core::presentation_commands::map_document_path(&app.project, "harbor")
                .expect("map path");
        let map_before = app
            .project
            .authoring_document(&map_path)
            .expect("map document")
            .bytes()
            .to_vec();
        app.apply_map_intents(vec![pending], vec![Some(stale)]);
        assert!(app.map_failed_command.is_some());
        assert_eq!(app.history.len(), 0);
        let revision_after_failure = app.map_revision;
        app.recompile();
        assert_ne!(app.map_revision, revision_after_failure);

        app.retry_failed_map_command();

        assert!(app.map_failed_command.is_none());
        assert_eq!(app.history.len(), 1);
        assert_ne!(
            app.project
                .authoring_document(&map_path)
                .expect("map document")
                .bytes(),
            map_before.as_slice()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn retrying_after_switching_maps_keeps_failed_preview_and_project_unchanged() {
        let root = test_workspace("map-stale-retry-switch");
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
        app.tab = super::super::Tab::Map;
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
                ..Default::default()
            },
            |ctx| app.map_tab(ctx),
        );
        app.map_canvas.set_mode(CanvasMode::Edit);
        app.apply_map_intents(
            vec![EditIntent::Move {
                placement: "lighthouse".into(),
                geometry: MapGeometry::Point(NormalizedPoint::new(0.7, 0.8)),
            }],
            vec![Some(MapCommandBaseline {
                revision: worldline_core::presentation_commands::Revision::default(),
                expected_documents: BTreeMap::new(),
            })],
        );
        let source_before = app.project.sources();
        let history_before = app.history.len();
        app.map_selection = Some("another-map".into());

        app.retry_failed_map_command();

        assert_eq!(app.project.sources(), source_before);
        assert_eq!(app.history.len(), history_before);
        assert!(matches!(
            app.map_failed_command.as_ref(),
            Some(PendingMapCommand { map_id, .. }) if map_id == "harbor"
        ));
        assert!(app
            .io_error
            .as_deref()
            .is_some_and(|message| message.contains("返回该地图")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn create_without_an_editable_layer_keeps_draft_for_retry() {
        let root = test_workspace("map-no-editable-layer");
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
        app.tab = super::super::Tab::Map;
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1100.0, 700.0))),
                ..Default::default()
            },
            |ctx| app.map_tab(ctx),
        );
        app.map_canvas.set_mode(CanvasMode::Edit);
        for layer in &mut app.map_canvas.snapshot.layers {
            layer.locked = true;
        }
        let geometry = MapGeometry::Point(NormalizedPoint::new(0.6, 0.6));
        app.map_canvas.draft = Some(geometry.clone());
        app.map_canvas.finish_draft();
        let (intents, baselines) = app.map_canvas.take_edit_batch();
        app.apply_map_intents(intents, baselines);

        assert_eq!(app.map_canvas.draft, Some(geometry));
        assert!(matches!(
            app.map_failed_command.as_ref(),
            Some(PendingMapCommand {
                map_id,
                intent: EditIntent::Create(_),
                ..
            }) if map_id == "harbor"
        ));
        assert_eq!(app.io_error.as_deref(), Some("当前没有可编辑的未锁定图层"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn session_visibility_override_does_not_mask_persisted_default_after_undo() {
        let mut canvas = point_canvas();
        canvas.set_layer_visible("places", false);
        assert_eq!(canvas.layer_default_visibility("places"), Some(true));

        let mut persisted_hidden = canvas.core_snapshot.clone();
        persisted_hidden.layers[0].visible = false;
        canvas.set_snapshot(1, persisted_hidden);
        assert_eq!(canvas.layer_default_visibility("places"), Some(false));
        assert!(!canvas.layer_states()[0].1);

        // Undo restores the persisted default. It must also clear the browse
        // override so the next refresh cannot hide the layer again.
        canvas.set_layer_default_visible("places", true);
        assert_eq!(canvas.layer_default_visibility("places"), Some(true));
        assert!(canvas.layer_states()[0].1);
        let mut persisted_visible = canvas.core_snapshot.clone();
        persisted_visible.layers[0].visible = true;
        canvas.set_snapshot(2, persisted_visible);
        assert!(canvas.layer_states()[0].1);
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
    fn shared_reference_navigation_keeps_project_and_reading_data_unchanged() {
        let root = test_workspace("shared-reference-navigation");
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx);
        let mut app = super::super::WorldeditApp::new(&creation, Some(root.join("world.wl")));
        let before = app.project.sources();
        let version = app.version;
        app.open_reading(worldline_core::catalog::TargetRef {
            kind: "world".into(),
            id: "harbor".into(),
        });
        app.locate_reference("harbor", "lighthouse");
        assert_eq!(app.tab, super::super::Tab::Map);
        assert_eq!(app.map_selection.as_deref(), Some("harbor"));
        let request = app.map_locate_request.as_ref().unwrap();
        assert_eq!(request.placement_id, "lighthouse");
        assert_eq!(request.layer_id, "places");
        assert!(app.reading_target.is_none());
        assert_eq!(app.project.sources(), before);
        assert_eq!(app.version, version);
        assert!(!app.project.is_dirty());
        assert!(app.history.is_empty());
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
                title: "地点".into(),
                visible: true,
                locked: false,
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
                title: "地点".into(),
                visible: true,
                locked: false,
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
    fn locked_layer_allows_selection_but_rejects_drag_intent() {
        let ctx = egui::Context::default();
        let mut canvas = MapCanvas::new(MapRenderSnapshot {
            map_id: "map".into(),
            title: "测试地图".into(),
            extent: vec2(400.0, 400.0),
            raster_layers: Vec::new(),
            layers: vec![MapLayer {
                id: "places".into(),
                title: "地点".into(),
                visible: true,
                locked: true,
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
        let point = canvas
            .camera()
            .normalized_to_screen(pos2(0.25, 0.25), canvas.viewport());
        click_canvas(&ctx, &mut canvas, screen_rect, point, 1.0);

        assert_eq!(
            canvas.selected,
            Some(("point".into(), GeometryHit::Vertex(0)))
        );
        assert!(canvas.edit_intents().is_empty());
        assert_eq!(canvas.validation_error(), Some("图层已锁定，只能浏览标记"));
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
                title: "地点".into(),
                visible: true,
                locked: false,
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
        assert!(canvas.draft.is_some());
        assert_eq!(canvas.validation_error(), Some("点必须位于地图范围内"));
    }

    #[test]
    fn refreshing_a_map_preserves_selection_and_draft_without_resetting_camera_or_layers() {
        let mut canvas = MapCanvas::new(MapRenderSnapshot {
            map_id: "map".into(),
            title: "测试地图".into(),
            extent: vec2(400.0, 400.0),
            raster_layers: Vec::new(),
            layers: vec![MapLayer {
                id: "places".into(),
                title: "地点".into(),
                visible: true,
                locked: false,
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
            initial_geometry: MapGeometry::Point(NormalizedPoint::new(0.25, 0.25)),
            start_screen: pos2(0.0, 0.0),
            grab_offset: [0.0, 0.0],
            baseline: None,
            active: true,
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
                    title: "地点".into(),
                    visible: true,
                    locked: false,
                    placements: vec![MapPlacement {
                        id: "point".into(),
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

        assert_eq!(
            canvas.selected,
            Some(("point".into(), GeometryHit::Vertex(0)))
        );
        assert!(canvas.drag.is_some());
        assert_eq!(
            canvas.draft,
            Some(MapGeometry::Point(NormalizedPoint::new(0.3, 0.3)))
        );
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
                title: "地点".into(),
                visible: true,
                locked: false,
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
                    title: "地点".into(),
                    visible: true,
                    locked: false,
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
