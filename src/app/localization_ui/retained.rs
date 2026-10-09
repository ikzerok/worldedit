//! Inputs stay reachable after a filter or source snapshot no longer contains their old row.
use super::*;
pub(super) fn show(ui: &mut Ui, project: &Project, state: &mut LocalizationUiState) {
    let ids = state
        .workbench
        .id_inputs
        .values()
        .filter(|v| !v.is_empty())
        .count();
    let count = state.workbench.drafts.len() + ids;
    if count == 0 {
        return;
    }
    let mut open = None;
    let mut discard = None;
    let mut discard_id = None;
    let mut edited_id = false;
    let retained =
        egui::CollapsingHeader::new(format!("保留的未提交输入 · {count}")).show(ui, |ui| {
            ui.small("筛选、分页或源码变化不会丢弃这些输入；过期输入需要对照当前源文重新绑定");
            egui::ScrollArea::vertical()
                .id_salt("localization-retained-inputs")
                .max_height(180.0)
                .show(ui, |ui| {
                    for (key, draft) in &state.workbench.drafts {
                        ui.push_id(key, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(format!("{} · {}", draft.target_locale, draft.edit.id));
                                if ui.small_button("打开此稿").reveal_focus(ui).clicked() {
                                    open =
                                        Some((draft.target_locale.clone(), draft.edit.id.clone()));
                                }
                                if ui.small_button("放弃此稿").reveal_focus(ui).clicked() {
                                    discard = Some(key.clone());
                                }
                            })
                        });
                    }
                    for (key, value) in state
                        .workbench
                        .id_inputs
                        .iter_mut()
                        .filter(|(_, v)| !v.is_empty())
                    {
                        ui.push_id(key, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label("ID 输入");
                                edited_id |= ui
                                    .add(
                                        egui::TextEdit::singleline(value)
                                            .id(egui::Id::new((
                                                "localization-retained-id",
                                                &project.root,
                                                key,
                                            )))
                                            .desired_width(160.0),
                                    )
                                    .reveal_focus(ui)
                                    .changed();
                                if ui.small_button("放弃 ID 输入").reveal_focus(ui).clicked() {
                                    discard_id = Some(key.clone());
                                }
                            })
                        });
                    }
                });
        });
    retained.header_response.reveal_focus(ui);
    if edited_id {
        state.cancel_preview();
    }
    if let Some((locale, id)) = open {
        let return_to_play = state.return_to_play;
        state.open_translation(&locale, Some(&id));
        state.return_to_play = return_to_play;
    }
    if let Some(key) = discard {
        state.workbench.drafts.remove(&key);
        state.cancel_preview();
    }
    if let Some(key) = discard_id {
        state.workbench.id_inputs.remove(&key);
        state.cancel_preview();
    }
}
