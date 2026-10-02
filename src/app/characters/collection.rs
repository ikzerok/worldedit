//! 人物集合索引与图画布分离，搜索是个人临时状态，不写工程。
use super::WorldeditApp;
use crate::theme;

impl WorldeditApp {
    pub(super) fn character_collection(&mut self, ctx: &egui::Context) {
        let choices =
            self.snapshot
                .as_ref()
                .map(|snapshot| {
                    let symbols = &snapshot.result.analysis.symbols;
                    symbols
                        .character_order
                        .iter()
                        .filter_map(|id| {
                            symbols.characters.get(id).map(|person| {
                                (id.clone(), person.display.clone(), person.events.len())
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
        egui::SidePanel::left("character-index")
            .default_width(theme::INDEX_WIDTH)
            .width_range(180.0..=320.0)
            .frame(theme::index_panel())
            .show(ctx, |ui| {
                ui.heading("人物索引");
                let id = egui::Id::new("character-index-query");
                let mut query = ctx
                    .data(|data| data.get_temp::<String>(id))
                    .unwrap_or_default();
                ui.add(
                    egui::TextEdit::singleline(&mut query)
                        .hint_text("搜索名称或 ID")
                        .desired_width(f32::INFINITY),
                );
                ctx.data_mut(|data| data.insert_temp(id, query.clone()));
                let query = query.trim().to_lowercase();
                let matches = choices
                    .iter()
                    .filter(|(id, name, _)| {
                        name.to_lowercase().contains(&query) || id.to_lowercase().contains(&query)
                    })
                    .collect::<Vec<_>>();
                ui.label(theme::muted(format!(
                    "{} / {} 位人物",
                    matches.len(),
                    choices.len()
                )));
                ui.add_space(theme::SPACE_XS);
                egui::ScrollArea::vertical()
                    .id_salt("character-index-items")
                    .show(ui, |ui| {
                        for (id, name, count) in matches {
                            let selected = self
                                .character_editor
                                .as_ref()
                                .is_some_and(|editor| editor.draft.id == *id);
                            if ui
                                .add_sized(
                                    [ui.available_width(), 48.0],
                                    egui::Button::selectable(
                                        selected,
                                        format!("{name}\n{count} 个事件"),
                                    )
                                    .wrap(),
                                )
                                .on_hover_text(format!("{name} · character:{id}"))
                                .clicked()
                            {
                                self.select_character(id);
                            }
                        }
                    });
            });
    }
}
