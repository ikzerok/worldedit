use super::*;
use worldline_core::project::Project;
use worldline_core::reader_export::ReaderMapSelection;

impl ReaderPublishState {
    pub(super) fn refresh_map_choices(&mut self, project: &Project) {
        self.map_choices = project
            .map_index()
            .maps
            .into_values()
            .map(|map| MapChoice {
                id: map.id,
                title: map.title,
                placements: map
                    .placements
                    .into_values()
                    .map(|p| (p.id, p.label_override.unwrap_or_else(|| "地图标记".into())))
                    .collect(),
                rasters: map
                    .raster_layers
                    .into_iter()
                    .map(|r| (r.id, r.asset.id))
                    .collect(),
            })
            .collect();
        self.maps.retain(|id, selected| {
            let Some(choice) = self.map_choices.iter().find(|m| m.id == *id) else {
                return false;
            };
            selected
                .placements
                .retain(|id| choice.placements.iter().any(|(item, _)| item == id));
            selected
                .raster_layers
                .retain(|id| choice.rasters.iter().any(|(item, _)| item == id));
            true
        });
    }

    pub(super) fn map_choices_ui(&mut self, ui: &mut egui::Ui) -> bool {
        if self.map_choices.is_empty() {
            return false;
        }
        ui.heading("地图公开范围");
        ui.label("标记、底图层逐项选择；底图素材还需在附件中勾选。底图内文字也会公开。");
        let mut changed = false;
        for map in &self.map_choices {
            let mut selected = self.maps.contains_key(&map.id);
            if ui
                .checkbox(&mut selected, format!("{} ({})", map.title, map.id))
                .changed()
            {
                changed = true;
                if selected {
                    self.maps.insert(
                        map.id.clone(),
                        ReaderMapSelection {
                            id: map.id.clone(),
                            placements: Vec::new(),
                            raster_layers: Vec::new(),
                        },
                    );
                } else {
                    self.maps.remove(&map.id);
                }
            }
            if let Some(selection) = self.maps.get_mut(&map.id) {
                ui.indent(format!("reader-map-{}", map.id), |ui| {
                    for (id, title) in &map.placements {
                        changed |= choice_checkbox(
                            ui,
                            &mut selection.placements,
                            id,
                            &format!("标记：{title} ({id})"),
                        );
                    }
                    for (id, asset) in &map.rasters {
                        changed |= choice_checkbox(
                            ui,
                            &mut selection.raster_layers,
                            id,
                            &format!("底图：{id}；另选附件 {asset}"),
                        );
                    }
                });
            }
        }
        changed
    }
}

fn choice_checkbox(ui: &mut egui::Ui, choices: &mut Vec<String>, id: &str, label: &str) -> bool {
    let mut selected = choices.iter().any(|item| item == id);
    if !ui.checkbox(&mut selected, label).changed() {
        return false;
    }
    if selected {
        choices.push(id.to_owned());
    } else {
        choices.retain(|item| item != id);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_selection_is_explicit_and_survives_dto_roundtrip() {
        let mut state = ReaderPublishState::default();
        assert!(!state.has_selection());
        state.maps.insert(
            "atlas".into(),
            ReaderMapSelection {
                id: "atlas".into(),
                placements: vec!["public-pin".into()],
                raster_layers: Vec::new(),
            },
        );
        assert!(state.has_selection());
        let selection = state.selection();
        assert!(selection.objects.is_empty());
        assert!(selection.attachments.is_empty());
        assert_eq!(selection.maps[0].placements, ["public-pin"]);
        let json = serde_json::to_string(&selection).unwrap();
        let decoded: ReaderExportSelection = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, selection);
    }
}
