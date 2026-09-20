//! 独立锚点资料与对象关联；变化出处直接消费核心目录。
use super::catalog::kind_label;
use super::inspector::field;
use super::{Tab, WorldeditApp};
use crate::theme;
use egui::RichText;
use worldline_core::anchors::{anchor_target_kinds, AnchorDraft};
use worldline_core::ast::ChangeKind;
use worldline_core::catalog::{Catalog, TargetRef};

impl WorldeditApp {
    pub(super) fn new_anchor(&mut self) {
        let mut n = 1;
        if let Some(snapshot) = &self.snapshot {
            while snapshot
                .result
                .analysis
                .catalog
                .anchors
                .contains_key(&format!("anchor_{n}"))
            {
                n += 1;
            }
        }
        self.anchor_editor = Some((
            None,
            AnchorDraft {
                id: format!("anchor_{n}"),
                display: "新的叙事锚点".into(),
                ..Default::default()
            },
        ));
        self.catalog_target = None;
        self.tag_editor = None;
        self.state_editor = None;
        self.tab = Tab::Catalog;
        self.catalog_filter = "anchor".into();
    }

    pub(super) fn anchor_form(&mut self, ui: &mut egui::Ui, catalog: &Catalog) {
        if let Some(target) = &self.catalog_target {
            if target.kind != "anchor" {
                self.anchor_editor = None;
                return;
            }
            if self
                .anchor_editor
                .as_ref()
                .is_none_or(|(id, _)| id.as_ref() != Some(&target.id))
            {
                self.anchor_editor = catalog
                    .anchors
                    .get(&target.id)
                    .map(|a| (Some(a.id.clone()), AnchorDraft::from(a)));
            }
        }
        let Some((mut original, mut draft)) = self.anchor_editor.take() else {
            return;
        };
        theme::card().show(ui, |ui| {
            ui.label(RichText::new("叙事锚点").strong().size(19.0));
            ui.label(theme::muted("锚点 ID · 创建后保持稳定"));
            ui.add_enabled(
                original.is_none(),
                egui::TextEdit::singleline(&mut draft.id).desired_width(f32::INFINITY),
            );
            field(ui, "名称", &mut draft.display);
            ui.label(theme::muted("叙事意义"));
            ui.add(
                egui::TextEdit::multiline(&mut draft.description)
                    .desired_rows(5)
                    .desired_width(f32::INFINITY)
                    .hint_text("这次相遇、决定或变化，对故事意味着什么…"),
            );
            ui.add_space(12.0);
            ui.label(RichText::new("关联对象").strong());
            ui.label(theme::muted(
                "同时关联状态与事件，可查看该事件中此状态的变化出处。",
            ));
            for kind in anchor_target_kinds(self.project.compile_options()) {
                ui.collapsing(kind_label(kind), |ui| {
                    for object in catalog.objects.iter().filter(|o| &o.target.kind == kind) {
                        let mut selected = draft.targets.contains(&object.target);
                        if ui
                            .checkbox(
                                &mut selected,
                                format!("{} ({})", object.display, object.target.id),
                            )
                            .changed()
                        {
                            if selected {
                                draft.targets.push(object.target.clone());
                            } else {
                                draft.targets.retain(|t| t != &object.target);
                            }
                        }
                    }
                });
            }
            ui.add_space(12.0);
            if ui
                .add(theme::primary(if original.is_some() {
                    "应用锚点资料"
                } else {
                    "创建锚点"
                }))
                .clicked()
                && self.commit("锚点资料已更新", |p| {
                    p.write_anchor(original.as_deref(), &draft)
                })
            {
                original = Some(draft.id.clone());
                self.catalog_target = Some(TargetRef::new("anchor", &draft.id));
            }
        });
        if let Some(anchor) = original.as_ref().and_then(|id| catalog.anchors.get(id)) {
            ui.add_space(16.0);
            ui.label(RichText::new("已保存的关联与来源").strong());
            for link in &anchor.links {
                ui.horizontal_wrapped(|ui| {
                    let display = catalog
                        .object(&link.target)
                        .map(|o| o.display.as_str())
                        .unwrap_or(&link.target.id);
                    ui.label(format!("{} · {display}", kind_label(&link.target.kind)));
                    if ui.small_button("定位关联").clicked() {
                        self.jump_to_file(&link.file, link.line, 1);
                    }
                });
            }
            let changes = catalog.anchor_changes(&anchor.id);
            ui.add_space(12.0);
            ui.label(RichText::new(format!("状态变化出处 · {} 处", changes.len())).strong());
            ui.label(theme::muted(
                "显示已编写的变化与条件，不表示动作已经发生，也不推演唯一结局。",
            ));
            if changes.is_empty() {
                ui.label(theme::muted("所关联的状态与事件尚无共同的变化出处。"));
            }
            for (state, change) in changes {
                theme::card().show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{} · 事件 {}", state.display, change.event));
                        if ui.small_button("定位变化").clicked() {
                            self.jump_to_file(&change.file, change.line, 1);
                        }
                    });
                    let operation = match change.kind {
                        ChangeKind::AddTags => "增加标签",
                        ChangeKind::RemoveTags => "移除标签",
                        _ => "替换内容",
                    };
                    let tags: Vec<_> = change
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
                    ui.label(format!(
                        "{operation}：{}",
                        if tags.is_empty() {
                            "空集合".into()
                        } else {
                            tags.join(" · ")
                        }
                    ));
                    if let Some(note) = &change.note {
                        self.wiki_text(ui, note);
                    }
                    for context in &change.contexts {
                        ui.label(theme::muted(context));
                    }
                    ui.label(theme::muted(format!("{}:{}", change.file, change.line)));
                });
            }
        }
        self.anchor_editor = Some((original, draft));
    }
}
