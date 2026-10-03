//! 人物索引只消费core，身份不因筛选或同名而改变。
use super::WorldeditApp;
use crate::theme;
impl WorldeditApp {
    pub(super) fn character_collection(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("character-index")
            .default_width(210.0)
            .width_range(180.0..=230.0)
            .frame(theme::index_panel())
            .show(ctx, |ui| self.character_collection_content(ui));
    }
    pub(super) fn character_collection_content(&mut self, ui: &mut egui::Ui) {
        let choices = self
            .snapshot
            .as_ref()
            .map(|s| {
                let symbols = &s.result.analysis.symbols;
                symbols
                    .character_order
                    .iter()
                    .filter_map(|id| {
                        symbols
                            .characters
                            .get(id)
                            .map(|p| (id.clone(), p.display.clone(), p.events.len()))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        ui.heading("人物索引");
        ui.add_space(theme::SPACE_XS);
        ui.add(
            egui::TextEdit::singleline(&mut self.character_focus.query)
                .id(egui::Id::new("character-index-query-input"))
                .hint_text("搜索名称或 ID")
                .desired_width(f32::INFINITY),
        );
        let query = self.character_focus.query.trim().to_lowercase();
        let matches = choices
            .iter()
            .filter(|(id, name, _)| {
                name.to_lowercase().contains(&query) || id.to_lowercase().contains(&query)
            })
            .collect::<Vec<_>>();
        ui.label(theme::muted(format!(
            "{} / {} 位人物 · 全人物索引",
            matches.len(),
            choices.len()
        )));
        egui::ScrollArea::vertical()
            .id_salt("character-index-items")
            .show_rows(ui, 48.0, matches.len(), |ui, range| {
                for index in range {
                    let (id, name, count) = matches[index];
                    let selected = self
                        .character_editor
                        .as_ref()
                        .is_some_and(|e| e.original.as_ref() == Some(id));
                    if ui
                        .add_sized(
                            [ui.available_width(), 48.0],
                            egui::Button::selectable(
                                selected,
                                format!("{}\n{count} 个事件", crate::visual::truncated(name, 18)),
                            )
                            .wrap(),
                        )
                        .on_hover_text(format!("{name} · character:{id}"))
                        .clicked()
                    {
                        if !selected {
                            self.remember_author_position();
                            self.select_character(id);
                        }
                        self.character_focus.index_open = false;
                    }
                }
            });
    }
}
