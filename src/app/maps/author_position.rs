//! 地图位置仅保存个人身份与镜头；恢复始终重新消费当前 core DTO。
use super::*;
use crate::app::{Tab, WorldeditApp};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(in crate::app) struct AuthorMapPosition {
    map_id: String,
    path: PathBuf,
    baseline: String,
    camera: navigation::CameraState,
    placement: Option<String>,
    scene: Vec<String>,
    visibility: HashMap<String, bool>,
    navigation: Option<navigation::MapNavigationController>,
}

impl WorldeditApp {
    pub(in crate::app) fn capture_map_position(&self) -> Option<AuthorMapPosition> {
        if self.tab != Tab::Map {
            return None;
        }
        let map_id = self.map_selection.as_ref()?;
        if self.map_canvas.map_id() != map_id {
            return None;
        }
        let path =
            worldline_core::presentation_commands::map_document_path(&self.project, map_id).ok()?;
        let document = self.project.authoring_document(&path).ok()?;
        let baseline = worldline_core::presentation_commands::document_hash(document.bytes());
        let mut navigation = self.map_navigation.clone();
        if let Some(navigation) = &mut navigation {
            navigation.update_current_camera(self.map_canvas.camera_state());
        }
        Some(AuthorMapPosition {
            map_id: map_id.clone(),
            path,
            baseline,
            camera: self.map_canvas.camera_state(),
            placement: self.map_canvas.selected.as_ref().map(|(id, _)| id.clone()),
            scene: self.map_canvas.scene.selection.iter().cloned().collect(),
            visibility: self.map_canvas.session_layer_visibility.clone(),
            navigation,
        })
    }

    pub(in crate::app) fn restore_map_position(
        &mut self,
        saved: Option<&AuthorMapPosition>,
    ) -> bool {
        if self.map_navigation_blocked() {
            return false;
        }
        let Some(saved) = saved else {
            // 兼容旧设备状态；没有身份时不能猜测历史地图。
            self.message = Some("旧位置没有地图身份，保留当前地图入口".into());
            return false;
        };
        if let Err(error) = self.project.verify_review_navigation() {
            self.message = Some(format!("地图来源尚未确认，保留当前入口：{error}"));
            return false;
        }
        let Some(snapshot) = &self.snapshot else {
            self.message = Some("当前地图索引不可用，未恢复旧地图位置".into());
            return false;
        };
        let Some(map) = snapshot.map_index.maps.get(&saved.map_id).cloned() else {
            self.message = Some("上次的地图已不存在，保留当前入口；未按同名替换".into());
            return false;
        };
        let path =
            worldline_core::presentation_commands::map_document_path(&self.project, &saved.map_id);
        if path.as_ref().ok() != Some(&saved.path) {
            self.message = Some("上次的地图来源身份已变化，保留当前入口；未按同名替换".into());
            return false;
        }
        let same_source = self
            .project
            .authoring_document(&saved.path)
            .is_ok_and(|document| {
                worldline_core::presentation_commands::document_hash(document.bytes())
                    == saved.baseline
            });
        let same_map = self.map_canvas.map_id() == saved.map_id;
        if !self
            .map_canvas
            .set_snapshot(self.version, render_snapshot(&map))
        {
            return false;
        }
        self.map_canvas.sync_scene(map.scene.as_ref());
        self.map_selection = Some(saved.map_id.clone());
        self.map_locate_request = None;
        self.pending_map_camera = None;
        self.map_canvas.selected = None;
        self.map_canvas.scene.selection.clear();
        self.map_canvas.scene.inspector = None;
        if same_source {
            if !same_map {
                for (layer, visible) in &saved.visibility {
                    self.map_canvas.set_layer_visible(layer, *visible);
                }
            }
            let mut skipped = false;
            if let Some(id) = &saved.placement {
                if self
                    .map_canvas
                    .placement_layer(id)
                    .is_some_and(|(_, visible, _)| visible)
                {
                    self.map_canvas.selected = Some((id.clone(), GeometryHit::Body));
                } else {
                    skipped = true;
                }
            }
            for id in &saved.scene {
                if self
                    .map_canvas
                    .placement_layer(id)
                    .is_some_and(|(_, visible, _)| visible)
                {
                    self.map_canvas.select_scene(id, true);
                } else {
                    skipped = true;
                }
            }
            self.map_canvas.restore_camera(saved.camera);
            let mut navigation = saved
                .navigation
                .clone()
                .filter(|navigation| navigation.current().map_id == saved.map_id);
            if let Some(navigation) = &mut navigation {
                navigation.validate_history(&snapshot.map_index);
            }
            self.map_navigation = navigation.or_else(|| {
                Some(navigation::MapNavigationController::new(
                    &map.id,
                    &map.title,
                    saved.camera,
                ))
            });
            if skipped {
                self.message = Some(
                    "已返回地图与镜头；部分标记已不存在或隐藏，未恢复这些选择或显示图层".into(),
                );
            }
        } else {
            self.map_canvas.reset_for_navigation();
            self.map_navigation = Some(navigation::MapNavigationController::new(
                &map.id,
                &map.title,
                self.map_canvas.camera_state(),
            ));
            self.message = Some(
                "已返回当前地图；来源版本已变化，未恢复旧选择、镜头或访问路径，当前内容完整保留"
                    .into(),
            );
        }
        true
    }

    pub(in crate::app) fn map_reference_caption(
        &self,
        map_id: &str,
        placement_id: &str,
    ) -> Option<(String, String)> {
        let snapshot = self.snapshot.as_ref()?;
        let map = snapshot.map_index.maps.get(map_id)?;
        let (layer, title) = if let Some(placement) = map.placements.get(placement_id) {
            let title = placement
                .label_override
                .clone()
                .filter(|title| !title.trim().is_empty())
                .or_else(|| {
                    placement.target_ref.as_ref().and_then(|target| {
                        snapshot
                            .result
                            .analysis
                            .catalog
                            .object(target)
                            .map(|object| object.display.clone())
                    })
                })
                .or_else(|| {
                    (!placement.annotation.trim().is_empty()).then(|| placement.annotation.clone())
                })
                .unwrap_or_else(|| placement.id.clone());
            (&placement.layer_id, title)
        } else {
            let node = map.scene.as_ref()?.nodes.get(placement_id)?;
            let title = node
                .target_ref
                .as_ref()
                .and_then(|target| {
                    snapshot
                        .result
                        .analysis
                        .catalog
                        .object(target)
                        .map(|object| object.display.clone())
                })
                .unwrap_or_else(|| {
                    if node.name.trim().is_empty() {
                        node.id.clone()
                    } else {
                        node.name.clone()
                    }
                });
            (&node.layer_id, title)
        };
        Some((
            format!("{} / {title}", map.title),
            format!("{map_id} · {placement_id} · 图层 {layer}"),
        ))
    }

    pub(in crate::app) fn map_reference_button(
        &mut self,
        ui: &mut egui::Ui,
        map_id: &str,
        placement_id: &str,
    ) {
        let Some((title, identity)) = self.map_reference_caption(map_id, placement_id) else {
            ui.label(crate::theme::muted("地图位置已失效，请刷新资料"));
            return;
        };
        if ui
            .add(egui::Button::new(format!("定位 {title}")).wrap())
            .clicked()
        {
            let origin = self.author_location(Some(ui.ctx()));
            self.locate_reference_from(map_id, placement_id, origin);
        }
        ui.add(egui::Label::new(crate::theme::muted(identity)).wrap());
    }
}
