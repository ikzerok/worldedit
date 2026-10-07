impl super::super::WorldeditApp {
    pub(super) fn map_search_panel(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.collapsing("对象反查", |ui| {
            ui.label(crate::theme::muted("按名称、ID、类型或别名查找地图标记。"));
            ui.text_edit_singleline(&mut self.map_search);
            let matches = self.map_object_candidates(ui, "reverse-lookup", &self.map_search);
            for object in matches {
                let placements = self
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.map_index.placements_for(&object.target))
                    .unwrap_or_default();
                ui.label(format!(
                    "{}  ·  {}:{}",
                    object.display, object.target.kind, object.target.id
                ))
                .on_hover_text(format!("{}:{}", object.file, object.line));
                if placements.is_empty() {
                    ui.label(crate::theme::muted("  未放置在地图上"));
                }
                for placement in placements {
                    self.map_reference_button(ui, &placement.map_id, &placement.placement_id);
                }
            }
        });
    }
}
