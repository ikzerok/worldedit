use worldline_core::vector_scene::SceneOp;

impl super::super::WorldeditApp {
    pub(super) fn preview_legacy_migration(&mut self, layer: Option<&str>) {
        let ids: Vec<_> = self
            .map_canvas
            .snapshot
            .layers
            .iter()
            .filter(|item| layer.is_none_or(|id| id == item.id))
            .flat_map(|item| item.placements.iter().map(|placement| placement.id.clone()))
            .collect();
        if ids.is_empty() {
            self.message = Some("此范围没有需要迁移的旧标记".into());
            return;
        }
        if self.map_canvas.has_uncommitted_work() {
            self.message = Some("请先应用或取消地图草稿，再预览旧标记迁移".into());
            return;
        }
        let mut ops = Vec::new();
        if self.map_canvas.scene.source.is_none() {
            ops.push(SceneOp::EnableScene);
        }
        ops.push(SceneOp::MigratePlacements { node_ids: ids });
        self.map_canvas.scene.review_requested = true;
        self.map_canvas.scene_queue(ops);
    }

    pub(super) fn scene_review_window(&mut self, ctx: &egui::Context) {
        let Some(plan) = self.map_canvas.scene.review_plan.take() else {
            return;
        };
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new("矢量修改预览 · 确认后一次应用")
            .id(egui::Id::new("scene-atomic-review")).default_width(560.0).show(ctx, |ui| {
                ui.horizontal(|ui| {
                    apply = ui.add(crate::theme::primary("确认应用（一次撤销）")).clicked();
                    cancel = ui.button("取消此次预览").clicked();
                });
                ui.label(format!("{} 个操作 · {} 个受影响 ID", plan.operation_count, plan.affected_nodes.len()));
                for diagnostic in &plan.diagnostics {
                    ui.colored_label(crate::theme::WARNING(), format!("{}：{}", diagnostic.code, diagnostic.message));
                }
                ui.label("迁移保持旧标记 ID 与关联，不创建另一份真源；分组跨越未选兄弟时会改变叠放关系，请核对列表。");
                egui::ScrollArea::vertical().max_height(280.0).show_rows(ui, 22.0, plan.affected_nodes.len(), |ui, range| {
                    for id in &plan.affected_nodes[range] { crate::theme::technical_value(ui, "ID", id); }
                });
            });
        if apply {
            if let Err(error) = self.apply_scene_plan(&plan) {
                self.map_canvas.scene.error = Some(error);
                self.map_canvas.scene.review_plan = Some(plan);
            }
        } else if cancel {
            self.map_canvas.scene.retry_operations.clear();
            self.map_canvas.scene.error = None;
        } else {
            self.map_canvas.scene.review_plan = Some(plan);
        }
    }
}
