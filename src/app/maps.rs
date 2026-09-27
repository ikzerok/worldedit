//! 地图画布与展示编辑。

mod camera;
mod canvas;
mod canvas_interaction;
mod geometry;
mod map_commands;
mod map_form;
mod map_layers_ui;
mod map_markers_ui;
mod map_navigation;
mod map_overview;
mod map_search;
mod map_ui;
pub(super) mod navigation;
mod raster;
mod render;

use camera::Camera2D;
use egui::{Color32, Pos2, Rect, Vec2};
use geometry::{GeometryHit, MapGeometry, NormalizedPoint};
use raster::RasterTextureCache;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
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
    pub(super) navigation: Option<navigation::MapNavigationDto>,
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
                .map(|placement| {
                    let navigation = navigation::project_placement_navigation(placement);
                    MapPlacement {
                        id: navigation.placement_id,
                        target_ref: placement.target_ref.clone(),
                        navigation: navigation.navigation,
                        annotation: placement.annotation.clone(),
                        role: placement.role.clone(),
                        label_override: placement.label_override.clone(),
                        geometry: geometry_from_core(&placement.geometry),
                        style: MapStyle::default(),
                    }
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
    pub(super) create_place_on_next_point: bool,
    pub(super) pending_place: Option<PendingPlace>,
    pub(super) place_name: String,
    pub(super) place_description: String,
}

#[derive(Clone)]
pub(super) struct PendingPlace {
    map_id: String,
    layer_id: String,
    geometry: MapGeometry,
    expected_baseline: String,
    entity_id: String,
    placement_id: String,
    source_path: PathBuf,
}

impl PlacementForm {
    pub(super) fn has_uncommitted_work(&self) -> bool {
        self.pending_place.is_some()
            || self.editing_placement.is_some()
            || self.target.is_some()
            || !self.target_query.trim().is_empty()
            || !self.annotation.trim().is_empty()
            || !self.role.trim().is_empty()
            || !self.label_override.trim().is_empty()
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn clipboard_text(&self) -> String {
        let target = self
            .target
            .as_ref()
            .map(|target| format!("{}:{}", target.kind, target.id))
            .unwrap_or_default();
        format!(
            "target: {target}\nquery: {}\nannotation: {}\nrole: {}\nlabel: {}\nplace: {}\ndescription: {}",
            self.target_query,
            self.annotation,
            self.role,
            self.label_override,
            self.place_name,
            self.place_description
        )
    }
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

#[cfg(test)]
#[path = "maps/tests/app.rs"]
mod tests_app;
#[cfg(test)]
#[path = "maps/tests/camera.rs"]
mod tests_camera;
#[cfg(test)]
#[path = "maps/tests/canvas_input.rs"]
mod tests_canvas_input;
#[cfg(test)]
#[path = "maps/tests/canvas_state.rs"]
mod tests_canvas_state;
#[cfg(test)]
#[path = "maps/tests/map_creation.rs"]
mod tests_map_creation;
#[cfg(test)]
#[path = "maps/tests/navigation.rs"]
mod tests_navigation;
#[cfg(test)]
#[path = "maps/tests/support.rs"]
mod tests_support;
