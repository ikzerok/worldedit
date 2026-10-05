impl super::super::WorldeditApp {
    pub(super) fn map_search_panel(&mut self, ui: &mut egui::Ui) {
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
                        .map(|snapshot| snapshot.map_index.placements_for(&object.target))
                        .unwrap_or_default();
                    ui.label(format!(
                        "{}  ·  {}:{}",
                        object.display, object.target.kind, object.target.id
                    ));
                    if placements.is_empty() {
                        ui.label(crate::theme::muted("  未放置在地图上"));
                    }
                    for placement in placements {
                        self.map_reference_button(ui, &placement.map_id, &placement.placement_id);
                    }
                }
            }
        });
    }
}
