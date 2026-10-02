use super::*;
use worldline_core::project::Project;
use worldline_core::reader_export::ReaderMapSelection;

impl ReaderPublishState {
    pub(super) fn refresh_map_choices(&mut self, project: &Project) {
        self.map_choices = project
            .map_index()
            .maps
            .into_values()
            .map(|map| {
                let mut placements = map
                    .placements
                    .into_values()
                    .map(|p| {
                        let label = match p.geometry {
                            worldline_core::presentation::MapGeometry::Text { text, .. } => text,
                            _ => p.label_override.unwrap_or_else(|| "地图标记".into()),
                        };
                        (p.id, label)
                    })
                    .collect::<Vec<_>>();
                if let Some(scene) = map.scene {
                    placements.extend(scene.nodes.into_values().map(|node| {
                        let label = if matches!(
                            node.geometry,
                            worldline_core::vector_scene::SceneGeometry::Group { .. }
                        ) {
                            format!("组 {}（子项须单独选择）", node.name)
                        } else if node.name.is_empty() {
                            "矢量图元".into()
                        } else {
                            node.name
                        };
                        (node.id, label)
                    }));
                }
                MapChoice {
                    id: map.id,
                    title: map.title,
                    placements,
                    rasters: map
                        .raster_layers
                        .into_iter()
                        .map(|r| (r.id, r.asset.id))
                        .collect(),
                }
            })
            .collect();
    }

    pub(super) fn map_choices_ui(&mut self, ui: &mut egui::Ui) -> bool {
        if self.map_choices.is_empty() {
            return false;
        }
        ui.heading("地图公开范围");
        ui.label("地图校准与临时尺子不会公开到读者站；完整工程导出保留已保存校准。");
        ui.label("标记、底图层逐项选择；底图素材还需在附件中勾选。底图内文字也会公开。");
        let mut changed = false;
        let query = self.query.trim().to_lowercase();
        let choices = self
            .map_choices
            .iter()
            .filter(|map| {
                [&map.id, &map.title]
                    .iter()
                    .any(|value| value.to_lowercase().contains(&query))
                    || map
                        .placements
                        .iter()
                        .chain(map.rasters.iter())
                        .any(|(id, title)| {
                            id.to_lowercase().contains(&query)
                                || title.to_lowercase().contains(&query)
                        })
            })
            .collect::<Vec<_>>();
        let range = selection_ui::page_range(ui, &mut self.map_page, choices.len());
        for map in &choices[range] {
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
                    let map_matches = [&map.id, &map.title]
                        .iter()
                        .any(|value| value.to_lowercase().contains(&query));
                    let nodes = map
                        .placements
                        .iter()
                        .filter(|(id, title)| {
                            map_matches
                                || id.to_lowercase().contains(&query)
                                || title.to_lowercase().contains(&query)
                        })
                        .collect::<Vec<_>>();
                    if ui.button("选择此地图的全部筛选图元").clicked() {
                        for (id, _) in &nodes {
                            if !selection.placements.contains(id) {
                                selection.placements.push(id.clone());
                            }
                        }
                        changed = true;
                    }
                    let page_id = ui.make_persistent_id(("reader-map-nodes", &map.id, &query));
                    let mut page = ui
                        .data(|data| data.get_temp::<usize>(page_id))
                        .unwrap_or_default();
                    let range = selection_ui::page_range(ui, &mut page, nodes.len());
                    ui.data_mut(|data| data.insert_temp(page_id, page));
                    for (id, title) in &nodes[range] {
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
