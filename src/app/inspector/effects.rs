//! 既有事件效果助手；不属于有界条件作者器的三个槽。
use super::field;
use crate::theme;
use egui::RichText;
use worldline_core::{ast::EffectWhen, authoring::EffectDraft, catalog::Catalog};
fn effect_label(when: EffectWhen) -> &'static str {
    match when {
        EffectWhen::Enter => "前置 · enter",
        EffectWhen::Exit => "后置 · exit",
        EffectWhen::Done => "自然完成 · done",
    }
}

pub(super) fn effect_cards(
    ui: &mut egui::Ui,
    effects: &mut Vec<EffectDraft>,
    catalog: Option<&Catalog>,
) {
    ui.label(RichText::new("事件效果").strong());
    ui.label(theme::muted("前置在准入通过后执行；后置在完成或离开时执行。自然完成效果仅在正文走完时执行，早于后置效果。"));
    let mut remove = None;
    for (i, effect) in effects.iter_mut().enumerate() {
        ui.push_id(("effect", i), |ui| {
            theme::card().show(ui, |ui| {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("when")
                        .selected_text(effect_label(effect.when))
                        .show_ui(ui, |ui| {
                            for when in [EffectWhen::Enter, EffectWhen::Exit, EffectWhen::Done] {
                                ui.selectable_value(&mut effect.when, when, effect_label(when));
                            }
                        });
                    if ui.small_button("删除").clicked() {
                        remove = Some(i);
                    }
                });
                field(
                    ui,
                    "执行条件（可选，留空则始终执行）",
                    &mut effect.condition,
                );
                ui.label(theme::muted(
                    "例如 has(mood, calm)；同一时机按卡片顺序执行。",
                ));
                ui.label(theme::muted("动作代码"));
                ui.add(
                    egui::TextEdit::multiline(&mut effect.actions)
                        .code_editor()
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .hint_text("become mood with alert"),
                );
                if let Some(catalog) = catalog {
                    super::super::tags::condition(ui, catalog, &mut effect.condition);
                    super::super::tags::actions(ui, catalog, &mut effect.actions);
                }
            });
        });
        ui.add_space(6.0);
    }
    if let Some(i) = remove {
        effects.remove(i);
    }
    if ui.button("＋ 添加效果").clicked() {
        effects.push(EffectDraft {
            when: EffectWhen::Enter,
            condition: String::new(),
            actions: String::new(),
        });
    }
}
