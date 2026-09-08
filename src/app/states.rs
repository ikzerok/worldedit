//! 状态资料与变更出处。显示编写的变化，不为无序事件推演唯一结局。
use super::catalog::kind_label;
use super::inspector::field;
use super::{Tab, WorldeditApp};
use crate::theme::{self, ACCENT};
use egui::RichText;
use worldline_core::catalog::{Catalog, TargetRef};
use worldline_core::states::StateDraft;

impl WorldeditApp {
    pub(super) fn new_state(&mut self, target: TargetRef) {
        let mut n = 1;
        if let Some(snapshot) = &self.snapshot {
            while snapshot
                .result
                .analysis
                .catalog
                .states
                .contains_key(&format!("state_{n}"))
            {
                n += 1;
            }
        }
        self.state_editor = Some((
            None,
            StateDraft {
                id: format!("state_{n}"),
                display: "新的状态".into(),
                target,
                tags: Vec::new(),
            },
        ));
        self.catalog_target = None;
        self.tag_editor = None;
        self.anchor_editor = None;
        self.tab = Tab::Catalog;
        self.catalog_filter = "state".into();
    }

    pub(super) fn state_form(&mut self, ui: &mut egui::Ui, catalog: &Catalog) {
        if let Some(target) = &self.catalog_target {
            if target.kind != "state" {
                self.state_editor = None;
                return;
            }
            if self
                .state_editor
                .as_ref()
                .is_none_or(|(id, _)| id.as_ref() != Some(&target.id))
            {
                self.state_editor = catalog.states.get(&target.id).map(|s| {
                    (
                        Some(s.id.clone()),
                        StateDraft {
                            id: s.id.clone(),
                            display: s.display.clone(),
                            target: s.target.clone(),
                            tags: s.tags.clone(),
                        },
                    )
                });
            }
        }
        let Some((mut original, mut draft)) = self.state_editor.take() else {
            return;
        };
        theme::card().show(ui, |ui| {
            ui.label(RichText::new("状态资料").strong().size(19.0));
            ui.label(theme::muted("状态 ID · 用于引用和追踪变化"));
            ui.add_enabled(
                original.is_none(),
                egui::TextEdit::singleline(&mut draft.id).desired_width(f32::INFINITY),
            );
            field(ui, "名称", &mut draft.display);
            ui.label(theme::muted("指向的完整对象"));
            egui::ComboBox::from_id_salt("state-target")
                .width(ui.available_width().min(430.0))
                .selected_text(format!(
                    "{} · {}",
                    kind_label(&draft.target.kind),
                    catalog
                        .object(&draft.target)
                        .map(|o| o.display.as_str())
                        .unwrap_or(&draft.target.id)
                ))
                .show_ui(ui, |ui| {
                    for object in &catalog.objects {
                        ui.selectable_value(
                            &mut draft.target,
                            object.target.clone(),
                            format!(
                                "{} · {} ({})",
                                kind_label(&object.target.kind),
                                object.display,
                                object.target.id
                            ),
                        );
                    }
                });
            ui.add_space(10.0);
            ui.label(RichText::new("初始内容").strong());
            ui.label(theme::muted(
                "可同时选择多个标签；全部取消表示空状态。事件中的变更独立记录。",
            ));
            ui.push_id(("state-tags", &draft.id), |ui| {
                super::tags::picker(ui, catalog, &mut draft.tags, &[]);
            });
            if catalog.tags.values().all(|t| !t.declared) {
                ui.label(theme::muted("先创建标签，也可以保存空状态。"));
            }
            ui.add_space(12.0);
            if ui
                .add(theme::primary(if original.is_some() {
                    "应用状态资料"
                } else {
                    "创建状态"
                }))
                .clicked()
                && self.commit("状态资料已更新", |p| {
                    p.write_state(original.as_deref(), &draft)
                })
            {
                original = Some(draft.id.clone());
                self.catalog_target = Some(TargetRef::new("state", &draft.id));
            }
        });
        if let Some(state) = original.as_ref().and_then(|id| catalog.states.get(id)) {
            ui.add_space(18.0);
            ui.label(
                RichText::new(format!("变更出处 · {} 处", state.changes.len()))
                    .strong()
                    .size(17.0),
            );
            ui.label(theme::muted("按源文件位置排列。条件分支和未指定先后的事件保留各自的变化，不代表已经发生的顺序。"));
            if state.changes.is_empty() {
                ui.label(theme::muted(
                    "在事件详情中添加前置或后置效果，选择此状态与目标标签，即可在这里追踪。",
                ));
            }
            for (index, change) in state.changes.iter().enumerate() {
                ui.push_id(index, |ui| {
                    theme::card().show(ui, |ui| {
                        let timing = match change.timing.as_str() {
                            "enter" => "前置效果",
                            "exit" => "后置效果",
                            "done" => "自然完成",
                            _ => "事件过程中",
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(timing).color(ACCENT));
                            if ui.button(format!("事件 · {}", change.event)).clicked() {
                                self.select_event(&change.event);
                                self.tab = Tab::Timeline;
                            }
                            if ui.small_button("定位变更").clicked() {
                                self.jump_to_file(&change.file, change.line, 1);
                            }
                        });
                        let names: Vec<_> = change
                            .tags
                            .iter()
                            .map(|id| {
                                catalog
                                    .tags
                                    .get(id)
                                    .map(|t| t.display.as_str())
                                    .unwrap_or(id)
                            })
                            .collect();
                        let action = match change.kind {
                            worldline_core::ast::ChangeKind::AddTags => "增加标签",
                            worldline_core::ast::ChangeKind::RemoveTags => "移除标签",
                            _ => "替换标签",
                        };
                        ui.label(format!(
                            "{action}：{}",
                            if names.is_empty() {
                                "空集合".into()
                            } else {
                                names.join(" · ")
                            }
                        ));
                        if let Some(note) = &change.note {
                            ui.label(note);
                        }
                        for context in &change.contexts {
                            ui.label(theme::muted(context));
                        }
                        ui.label(theme::muted(format!(
                            "{} · {}:{}",
                            change.node,
                            std::path::Path::new(&change.file)
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy(),
                            change.line
                        )));
                    });
                });
            }
        }
        self.state_editor = Some((original, draft));
    }
}
