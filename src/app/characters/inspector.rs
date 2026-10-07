use super::*;
use crate::app::inspector::{field, properties_with_references};

impl WorldeditApp {
    pub(super) fn character_inspector_content(&mut self, ui: &mut egui::Ui) {
        self.capture_new_draft_baselines();
        let template_index = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.template_index.clone())
            .unwrap_or_else(|| self.project.template_index());
        let catalog = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.result.analysis.catalog.clone())
            .unwrap_or_default();
        let protected = self.dirty_draft_names().contains(&"人物资料");
        let Some(mut editor) = self.character_editor.take() else {
            ui.label(RichText::new("人物档案").strong().size(17.0));
            ui.label(theme::muted(
                "选择一位人物,编辑姓名、属性和关系,查看其关联事件。",
            ));
            return;
        };
        let mut close = false;
        let mut apply = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new("人物档案").strong().size(17.0));
            if ui.small_button("收起档案").clicked() {
                close = true;
            }
        });
        egui::ScrollArea::vertical()
            .id_salt("person-form")
            .show(ui, |ui| {
                ui.label("姓名");
                ui.add(
                    egui::TextEdit::singleline(&mut editor.draft.display)
                        .id(egui::Id::new("character-name-input"))
                        .desired_width(f32::INFINITY),
                );
                egui::CollapsingHeader::new("身份与来源")
                    .default_open(editor.original.is_none())
                    .show(ui, |ui| {
                        field(
                            ui,
                            "角色 ID（改名会同步全部结构引用）",
                            &mut editor.draft.id,
                        );
                        theme::source_path(ui, &self.project.root, &editor.path);
                    });
                ui.separator();
                ui.label(RichText::new("人物属性").strong());
                ui.menu_button("＋ 常用资料栏目", |ui| {
                    for (key, label) in [
                        ("appearance", "外貌与识别特征"),
                        ("background", "背景经历"),
                        ("personality", "性格与行为习惯"),
                        ("motivation", "欲望与动机"),
                        ("boundaries", "底线与恐惧"),
                        ("voice", "说话方式"),
                        ("dialogue_examples", "口吻例句"),
                    ] {
                        let exists = editor.draft.properties.iter().any(|(name, _)| name == key);
                        if crate::theme::add_enabled(ui, !exists, egui::Button::new(label))
                            .clicked()
                        {
                            editor.draft.properties.push((
                                key.into(),
                                worldline_core::ast::PropertyValue::Str(String::new()),
                            ));
                            ui.close();
                        }
                    }
                });
                ui.label(theme::muted(
                    "栏目可留空；例句是创作参考，不会成为发生过的事件。",
                ));
                properties_with_references(
                    ui,
                    &mut editor.draft.properties,
                    &catalog,
                    self.project.compile_options(),
                );
                if let Some(suggestion) = crate::app::templates::template_panel(
                    ui,
                    "character",
                    None,
                    &template_index,
                    &catalog,
                    &mut editor.draft.properties,
                ) {
                    if let Some(id) = editor.original.clone() {
                        self.edit_relation(
                            None,
                            Some(worldline_core::TargetRef::new("character", &id)),
                        );
                        self.message = Some(format!(
                            "已按“{suggestion}”打开关系草稿；仍需明确关系类型和另一端"
                        ));
                    } else {
                        self.message = Some("请先保存新人物，再从模板建议打开关系草稿".into());
                    }
                }
                ui.separator();
                ui.label(RichText::new("旧式人物关系 · 可编辑").strong());
                let ids = self
                    .snapshot
                    .as_ref()
                    .map(|s| s.result.analysis.symbols.character_order.clone())
                    .unwrap_or_default();
                let mut remove = None;
                for (i, (target, label)) in editor.draft.relations.iter_mut().enumerate() {
                    ui.push_id(("relation", i), |ui| {
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("target")
                                .width(150.0)
                                .selected_text(target.as_str())
                                .show_ui(ui, |ui| {
                                    for id in &ids {
                                        ui.selectable_value(target, id.clone(), id);
                                    }
                                });
                            if ui.small_button("收起档案").clicked() {
                                remove = Some(i);
                            }
                        });
                        ui.add(
                            egui::TextEdit::singleline(label)
                                .hint_text("关系名称")
                                .desired_width(f32::INFINITY),
                        );
                    });
                }
                if let Some(index) = remove {
                    editor.draft.relations.remove(index);
                }
                if ui.button("＋ 添加关系").clicked() {
                    editor.draft.relations.push((
                        ids.iter()
                            .find(|id| **id != editor.draft.id)
                            .cloned()
                            .unwrap_or_default(),
                        "关联".into(),
                    ));
                }
                ui.add_space(8.0);
                apply = ui
                    .add_sized([ui.available_width(), 36.0], theme::primary("应用人物档案"))
                    .clicked();
                ui.separator();
                if let Some(id) = &editor.original {
                    self.object_links(
                        ui,
                        &worldline_core::catalog::TargetRef::new("character", id),
                    );
                }
                ui.label(RichText::new("关联事件 · 反向索引").strong());
                let info = self
                    .snapshot
                    .as_ref()
                    .and_then(|s| {
                        editor
                            .original
                            .as_ref()
                            .and_then(|id| s.result.analysis.symbols.characters.get(id))
                    })
                    .cloned();
                if let Some(info) = info {
                    if info.events.is_empty() {
                        ui.label(theme::muted("尚未关联事件,可在事件详情中勾选此人物"));
                    }
                    for event in &info.events {
                        if ui
                            .add_sized(
                                [ui.available_width(), 32.0],
                                egui::Button::new(format!("↗  {event}")),
                            )
                            .clicked()
                        {
                            if protected {
                                self.message =
                                    Some("人物资料有未应用输入，请先应用；输入已保留".into());
                            } else {
                                self.remember_author_position();
                                self.select_event(event);
                                self.tab = Tab::Timeline;
                            }
                        }
                    }
                    ui.label(theme::muted("包含 with、meet、part 与效果块的引用"));
                    if ui.small_button("查看人物源文件").clicked() {
                        self.jump_to_file(&info.decl_file, info.decl_span.line, 1);
                    }
                }
            });
        if apply
            && self.commit("人物档案与引用已更新", |p| {
                p.write_character(&editor.path, editor.original.as_deref(), &editor.draft)
            })
        {
            editor.original = Some(editor.draft.id.clone());
        }
        self.character_editor = Some(editor);
        if close {
            self.character_focus.inspector_open = false;
        }
    }
}
