use super::*;
use worldline_core::authoring::EntityDraft;
use worldline_core::authoring_intents::{AuthoringIntent, IntentTarget, PlacementRequest};
impl super::super::WorldeditApp {
    pub(in crate::app) fn map_navigation_blocked(&mut self) -> bool {
        if self.map_canvas.has_uncommitted_work()
            || self.map_failed_command.is_some()
            || self.map_form.has_uncommitted_work()
            || self.map_creation.open
        {
            self.message =
                Some("当前地图有未提交的展示修改，请保存、重试或取消后再切换地图。".into());
            true
        } else {
            false
        }
    }

    pub(in crate::app) fn cancel_map_form(&mut self) {
        self.map_form.clear();
        self.message = Some("已取消未提交的标记表单".into());
    }

    pub(super) fn commit_pending_place(&mut self) -> bool {
        if !self.map_canvas.is_edit_mode() {
            self.io_error = Some("请先进入编辑展示模式".into());
            return false;
        }
        let Some(pending) = self.map_form.pending_place.as_ref() else {
            return false;
        };
        if self.map_selection.as_deref() != Some(pending.map_id.as_str()) {
            self.io_error = Some("地图已切换，请保留草稿并返回原地图".into());
            return false;
        }
        let draft = EntityDraft {
            id: pending.entity_id.clone(),
            entity_type: "place".into(),
            display: self.map_form.place_name.trim().into(),
            description: self.map_form.place_description.clone(),
            ..Default::default()
        };
        let intent = AuthoringIntent {
            expected_baseline: pending.expected_baseline.clone(),
            target: IntentTarget::CreateEntity {
                path: pending.source_path.clone(),
                draft,
            },
            selection: None,
            placement: Some(PlacementRequest {
                map_id: pending.map_id.clone(),
                placement_id: pending.placement_id.clone(),
                layer_id: pending.layer_id.clone(),
                geometry: core_geometry(&pending.geometry),
                annotation: if self.map_form.annotation.trim().is_empty() {
                    self.map_form.place_name.trim().into()
                } else {
                    self.map_form.annotation.trim().into()
                },
                role: if self.map_form.role.trim().is_empty() {
                    "地点入口".into()
                } else {
                    self.map_form.role.trim().into()
                },
                label_override: (!self.map_form.label_override.trim().is_empty())
                    .then(|| self.map_form.label_override.trim().into()),
            }),
        };
        let before = self.project.clone();
        let result = self.project.apply_authoring_intent(&intent).map(|_| ());
        if self.finish_content_command(before, result, "已新建地点并放置入口（可撤销）")
        {
            self.map_canvas.reset_local_preview();
            self.map_form.clear();
            self.map_failed_command = None;
            true
        } else {
            false
        }
    }
}
