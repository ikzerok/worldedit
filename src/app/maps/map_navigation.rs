use super::*;
impl super::super::WorldeditApp {
    pub(super) fn ensure_map_navigation(&mut self, map_id: &str, title: &str) {
        let reset = self
            .map_navigation
            .as_ref()
            .is_none_or(|navigation| navigation.current().map_id != map_id);
        if reset {
            self.map_navigation = Some(navigation::MapNavigationController::new(
                map_id,
                title,
                self.map_canvas.camera_state(),
            ));
            self.pending_map_camera = None;
        }
    }

    /// 由地图标记的“进入地图”操作调用；只改变个人浏览状态。
    pub(in crate::app) fn enter_submap(&mut self, target: navigation::MapNavigationDto) -> bool {
        if self.map_navigation_blocked() {
            return false;
        }
        if !target.available {
            self.message = Some(format!(
                "目标地图 `{}` 不可用，请打开地图原文修复后再进入",
                target.map_id
            ));
            return false;
        }
        let Some(title) = self.snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .map_index
                .maps
                .get(&target.map_id)
                .map(|map| map.title.clone())
        }) else {
            self.message = Some(format!(
                "目标地图 `{}` 的文档不可用，请打开地图原文修复后再进入",
                target.map_id
            ));
            return false;
        };
        let Some(map_navigation) = self.map_navigation.as_mut() else {
            self.message = Some("当前没有可用的地图导航上下文".into());
            return false;
        };
        map_navigation.update_current_camera(self.map_canvas.camera_state());
        if !map_navigation.enter(&target, title, navigation::CameraState::default()) {
            return false;
        }
        self.map_selection = Some(target.map_id);
        self.pending_map_camera = None;
        if !self.map_canvas.reset_for_navigation() {
            self.message =
                Some("当前地图有未提交的展示修改，请保存、重试或取消后再切换地图。".into());
            return false;
        }
        true
    }

    /// 返回实际访问路径中的上一个地图，并安排恢复它离开时的个人镜头。
    pub(in crate::app) fn back_from_map_navigation(&mut self) -> bool {
        if self.map_navigation_blocked() {
            return false;
        }
        let Some(map_navigation) = self.map_navigation.as_mut() else {
            return false;
        };
        map_navigation.update_current_camera(self.map_canvas.camera_state());
        let Some(previous) = map_navigation.back() else {
            return false;
        };
        self.map_selection = Some(previous.map_id);
        self.pending_map_camera = Some(previous.camera);
        true
    }
}
