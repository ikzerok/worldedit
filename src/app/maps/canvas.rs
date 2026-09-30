use super::render::read_raster;
use super::*;
use std::path::Path;
impl MapCanvas {
    pub(in crate::app) fn new(snapshot: MapRenderSnapshot) -> Self {
        Self {
            svg_import: Default::default(),
            text_sizes: HashMap::new(),
            camera: Camera2D::new(snapshot.extent),
            fit_pending: true,
            core_snapshot: snapshot.clone(),
            snapshot,
            source_version: 0,
            viewport: Rect::NOTHING,
            mode: CanvasMode::Browse,
            form_blocked: false,
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

    pub(in crate::app) fn clear(&mut self) {
        self.svg_import = Default::default();
        self.text_sizes.clear();
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

    pub(in crate::app) fn invalidate_rasters(&mut self) {
        self.textures.clear();
        self.raster_errors.clear();
        self.raster_attempts.clear();
    }

    pub(in crate::app) fn needs_snapshot(&self, source_version: u64, map_id: &str) -> bool {
        self.source_version != source_version || self.snapshot.map_id != map_id
    }

    pub(in crate::app) fn set_snapshot(
        &mut self,
        source_version: u64,
        snapshot: MapRenderSnapshot,
    ) -> bool {
        let same_map = self.snapshot.map_id == snapshot.map_id
            && self.snapshot.extent == snapshot.extent
            && !snapshot.map_id.is_empty();
        if !same_map && self.has_uncommitted_work() {
            return false;
        }
        let same_source = self.source_version == source_version;
        let preserve_local = same_map && !same_source && self.has_uncommitted_work();
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
        true
    }

    #[cfg(test)]
    pub(in crate::app) fn layer_states(&self) -> Vec<(String, bool, usize)> {
        self.snapshot
            .layers
            .iter()
            .map(|layer| (layer.id.clone(), layer.visible, layer.placements.len()))
            .collect()
    }

    pub(in crate::app) fn layer_details(&self) -> Vec<(String, String, bool, bool, usize)> {
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

    pub(in crate::app) fn layer_order(&self) -> Vec<String> {
        self.snapshot
            .layers
            .iter()
            .map(|layer| layer.id.clone())
            .collect()
    }

    pub(in crate::app) fn layer_default_visibility(&self, id: &str) -> Option<bool> {
        self.core_snapshot
            .layers
            .iter()
            .find(|layer| layer.id == id)
            .map(|layer| layer.visible)
    }

    pub(in crate::app) fn set_layer_visible(&mut self, id: &str, visible: bool) {
        if let Some(layer) = self.snapshot.layers.iter_mut().find(|layer| layer.id == id) {
            self.session_layer_visibility.insert(id.to_owned(), visible);
            layer.visible = visible;
            if !visible {
                self.selected = None;
            }
        }
    }

    pub(in crate::app) fn set_layer_default_visible(&mut self, id: &str, visible: bool) {
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

    pub(in crate::app) fn set_command_baseline(&mut self, baseline: Option<MapCommandBaseline>) {
        self.command_baseline = baseline;
    }

    pub(in crate::app) fn selected_placement(&self) -> Option<MapPlacement> {
        let (id, _) = self.selected.as_ref()?;
        self.snapshot
            .layers
            .iter()
            .filter(|layer| layer.visible)
            .flat_map(|layer| layer.placements.iter())
            .find(|placement| placement.id == *id)
            .cloned()
    }

    pub(in crate::app) fn select_placement_id(&mut self, id: &str) -> bool {
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

    pub(in crate::app) fn placement_layer(&self, id: &str) -> Option<(String, bool, bool)> {
        self.snapshot.layers.iter().find_map(|layer| {
            layer
                .placements
                .iter()
                .find(|placement| placement.id == id)
                .map(|_| (layer.id.clone(), layer.visible, layer.locked))
        })
    }

    pub(in crate::app) fn reveal_layer_for_session(&mut self, id: &str) -> bool {
        let Some(layer) = self.snapshot.layers.iter_mut().find(|layer| layer.id == id) else {
            return false;
        };
        self.session_layer_visibility.insert(id.to_owned(), true);
        layer.visible = true;
        true
    }

    pub(in crate::app) fn is_edit_mode(&self) -> bool {
        self.mode == CanvasMode::Edit
    }

    pub(in crate::app) fn map_title(&self) -> &str {
        &self.snapshot.title
    }

    pub(in crate::app) fn map_id(&self) -> &str {
        &self.snapshot.map_id
    }

    #[cfg(test)]
    pub(in crate::app) fn raster_bytes(&self) -> usize {
        self.textures.used_bytes()
    }

    #[cfg(test)]
    pub(in crate::app) fn set_raster_budget(&mut self, budget: usize) {
        self.textures = RasterTextureCache::with_budget(budget);
        self.raster_errors.clear();
        self.raster_attempts.clear();
    }

    #[cfg(test)]
    pub(in crate::app) fn raster_attempt_count(&self) -> usize {
        self.raster_attempts.len()
    }

    pub(in crate::app) fn raster_states(&self) -> Vec<(String, bool, Option<String>)> {
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

    pub(in crate::app) fn prepare_rasters(&mut self, ctx: &egui::Context, root: &Path) {
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

    pub(in crate::app) fn camera_state(&self) -> navigation::CameraState {
        let (zoom, pan) = self.camera.state();
        navigation::CameraState { zoom, pan }
    }

    pub(in crate::app) fn restore_camera(&mut self, state: navigation::CameraState) {
        self.camera.restore(state.zoom, state.pan);
        self.fit_pending = false;
    }

    pub(in crate::app) fn reset_for_navigation(&mut self) -> bool {
        if self.has_uncommitted_work() {
            return false;
        }
        self.camera = Camera2D::new(self.snapshot.extent);
        self.fit_pending = true;
        self.selected = None;
        self.drag = None;
        self.last_error = None;
        true
    }

    pub(in crate::app) fn has_uncommitted_work(&self) -> bool {
        self.svg_import.open
            || self.drag.is_some()
            || self.draft.is_some()
            || !self.edit_intents.is_empty()
            || self.intent_baselines.iter().any(Option::is_some)
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

    pub(in crate::app) fn take_edit_batch(
        &mut self,
    ) -> (Vec<EditIntent>, Vec<Option<MapCommandBaseline>>) {
        (
            std::mem::take(&mut self.edit_intents),
            std::mem::take(&mut self.intent_baselines),
        )
    }

    pub(in crate::app) fn restore_failed_preview(&mut self, intent: &EditIntent) {
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

    pub(in crate::app) fn reset_local_preview(&mut self) {
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

    pub(in crate::app) fn validation_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
}
