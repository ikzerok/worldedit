use super::*;

impl MapCanvas {
    pub(in crate::app) fn reset_local_preview(&mut self) {
        self.measurement.calibration = None;
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

    /// 成功提交后只结束本地手势；下一份核心快照按稳定 ID 校验选择。
    pub(in crate::app) fn accept_local_preview(&mut self) {
        self.measurement.calibration = None;
        self.drag = None;
        self.draft = None;
        self.edit_intents.clear();
        self.intent_baselines.clear();
        self.command_baseline = None;
        self.last_error = None;
    }

    pub(in crate::app) fn reset_for_history(&mut self) {
        let selected = self.selected.clone();
        let scene_selection = self.scene.selection.clone();
        self.reset_local_preview();
        self.svg_import = Default::default();
        self.scene = Default::default();
        self.scene.selection = scene_selection;
        self.selected = selected;
    }

    pub(in crate::app) fn discard_local_work(&mut self) {
        let source = self.scene.source.clone();
        self.reset_for_history();
        self.sync_scene(source.as_ref());
    }
}
