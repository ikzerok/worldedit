use super::*;
impl WorldeditApp {
    pub(super) fn network_edge_list(&mut self, ui: &mut egui::Ui) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let catalog = &snapshot.result.analysis.catalog;
        let frozen = self
            .catalog_workbench
            .snapshot
            .clone()
            .filter(|_| self.query_scope_active());
        let labels = frozen
            .as_deref()
            .map_or_else(|| DisplayLabels::new(catalog), DisplayLabels::from_scope);
        let edges = self
            .network_state
            .result
            .as_ref()
            .map(|result| result.edges.clone())
            .unwrap_or_default();
        ui.separator();
        ui.label(RichText::new(format!("明确关系 · {}", edges.len())).strong());
        let mut open_relation = None;
        for edge in edges {
            ui.push_id(("network-edge", &edge.id), |ui| {
                let hidden = self.network_state.hidden.contains(&edge.id);
                ui.horizontal_wrapped(|ui| {
                    ui.label(if hidden { "○" } else { "●" });
                    if ui.link(format!("{} · {}", edge.label, edge.id)).clicked() {
                        open_relation = Some(TargetRef::new("relation", &edge.id));
                    }
                    if ui
                        .small_button(if hidden { "显示" } else { "隐藏" })
                        .clicked()
                    {
                        if hidden {
                            self.network_state.hidden.remove(&edge.id);
                        } else {
                            self.network_state.hide(&edge.id);
                        }
                    }
                });
                ui.label(theme::muted(format!(
                    "{} → {}{}",
                    labels.get(&edge.from_ref),
                    labels.get(&edge.to_ref),
                    if edge.direction == RelationDirection::Undirected {
                        "（无向）"
                    } else {
                        ""
                    }
                )));
            });
        }
        if let Some(target) = open_relation {
            self.open_reading(target);
        }
    }
}
