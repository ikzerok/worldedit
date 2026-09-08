//! 从工程真源集中阅读事件，不执行分支、不改变先后关系。
use super::{Tab, WorldeditApp};
use crate::theme::{self, BG};
use egui::RichText;

impl WorldeditApp {
    pub(super) fn overview_tab(&mut self, ctx: &egui::Context) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        if self
            .overview_cache
            .as_ref()
            .is_none_or(|(version, _)| *version != self.version)
        {
            self.overview_cache = Some((self.version, self.project.event_drafts()));
        }
        let storylines = snapshot.result.analysis.graph.storyline_order.clone();
        let entries = self
            .overview_cache
            .as_ref()
            .map(|(_, entries)| entries.clone())
            .unwrap_or_default();
        egui::CentralPanel::default()
            .frame(theme::panel().fill(BG))
            .show(ctx, |ui| {
                ui.heading("正文概览");
                ui.label(theme::muted(
                    "跨文件阅读同一世界的完整事件。这里的排列仅供阅读，不表示发生顺序。",
                ));
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("overview-storyline")
                        .selected_text(if self.overview_storyline.is_empty() {
                            "全部故事线"
                        } else {
                            &self.overview_storyline
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.overview_storyline,
                                String::new(),
                                "全部故事线",
                            );
                            for (id, display) in &storylines {
                                ui.selectable_value(
                                    &mut self.overview_storyline,
                                    id.clone(),
                                    display,
                                );
                            }
                        });
                    ui.add(
                        egui::TextEdit::singleline(&mut self.overview_query)
                            .hint_text("查找事件、人物或正文")
                            .desired_width(280.0),
                    );
                });
                ui.add_space(12.0);
                let query = self.overview_query.to_lowercase();
                egui::ScrollArea::vertical()
                    .id_salt("overview-events")
                    .show(ui, |ui| {
                        let mut count = 0;
                        for (path, draft) in &entries {
                            if !self.overview_storyline.is_empty()
                                && self.overview_storyline != draft.storyline
                            {
                                continue;
                            }
                            if !format!(
                                "{} {} {} {}",
                                draft.id,
                                draft.summary,
                                draft.characters.join(" "),
                                draft.body
                            )
                            .to_lowercase()
                            .contains(&query)
                            {
                                continue;
                            }
                            count += 1;
                            ui.push_id(&draft.id, |ui| {
                                theme::card().show(ui, |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            RichText::new(if draft.summary.is_empty() {
                                                &draft.id
                                            } else {
                                                &draft.summary
                                            })
                                            .strong()
                                            .size(20.0),
                                        );
                                        if ui.small_button("编辑事件").clicked() {
                                            self.select_event(&draft.id);
                                            self.tab = Tab::Timeline;
                                        }
                                        if ui.small_button("源文件").clicked() {
                                            let line = self
                                                .snapshot
                                                .as_ref()
                                                .and_then(|s| {
                                                    s.result
                                                        .program
                                                        .events
                                                        .iter()
                                                        .find(|e| e.name == draft.id)
                                                })
                                                .map(|e| e.loc.line)
                                                .unwrap_or(1);
                                            self.jump_to_file(&path.to_string_lossy(), line, 1);
                                        }
                                    });
                                    ui.label(theme::muted(format!(
                                        "{} · {} · {}",
                                        draft.storyline,
                                        draft.id,
                                        path.strip_prefix(&self.project.root)
                                            .unwrap_or(path)
                                            .display()
                                    )));
                                    if !draft.characters.is_empty() {
                                        ui.label(theme::muted(format!(
                                            "关联人物：{}",
                                            draft.characters.join("、")
                                        )));
                                    }
                                    if !draft.after.is_empty() {
                                        ui.label(theme::muted(format!(
                                            "前置要求：{}",
                                            draft.after
                                        )));
                                    }
                                    ui.add_space(12.0);
                                    self.linked_source(ui, &draft.body, &path.to_string_lossy());
                                    if !draft.effects.is_empty() {
                                        ui.collapsing(
                                            format!("效果 · {}", draft.effects.len()),
                                            |ui| {
                                                for effect in &draft.effects {
                                                    ui.label(
                                                        RichText::new(effect.when.label()).strong(),
                                                    );
                                                    if !effect.condition.is_empty() {
                                                        ui.label(format!(
                                                            "条件：{}",
                                                            effect.condition
                                                        ));
                                                    }
                                                    ui.monospace(&effect.actions);
                                                }
                                            },
                                        );
                                    }
                                });
                            });
                            ui.add_space(12.0);
                        }
                        if count == 0 {
                            ui.label(theme::muted("没有匹配的事件"));
                        }
                    });
            });
    }
}
