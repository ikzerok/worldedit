//! 通用实体创作页；所有类型使用相同的 core EntityDraft。
use super::authoring_forms::EntityForm;
use super::inspector::properties;
use super::WorldeditApp;
use crate::theme;
use std::path::Path;
use worldline_core::TargetRef;

pub(super) const ENTITY_KINDS: &[(&str, &str)] = &[
    ("cosmology", "宇宙观与位面"),
    ("place", "地理与地点"),
    ("species", "生态与物种"),
    ("daily_life", "日常生活"),
    ("culture", "文化与社会"),
    ("language", "语言与命名"),
    ("organization", "组织与制度"),
    ("economy", "经济与技艺"),
    ("technology", "技术与知识"),
    ("belief", "宗教与思想"),
    ("ability", "魔法与能力"),
    ("item_building", "物品与建筑"),
    ("history", "历史与史料"),
    ("narrative", "故事与叙事"),
    ("item", "物品（通用）"),
    ("concept", "概念（通用）"),
];
impl WorldeditApp {
    pub(super) fn edit_entity(&mut self, id: Option<&str>) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let path = if self.project.documents.contains_key(&self.active_file) {
            self.active_file.clone()
        } else {
            self.project.entry.clone()
        };
        let path = super::workspace_source_path(&self.project, &path);
        match EntityForm::open(
            &self.project,
            &snapshot.result.analysis.catalog,
            self.version,
            id,
            path,
        ) {
            Ok(form) => self.entity_editor = Some(form),
            Err(error) => self.io_error = Some(error),
        }
    }
    pub(super) fn entity_editor_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.entity_editor.take() else {
            return;
        };
        if form.source_selection.is_some()
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            return;
        }
        let mut open = true;
        let mut applied = false;
        let mut cancelled = false;
        let mut keyboard_apply = false;
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
        egui::Window::new(if form.original.is_some() {
            "编辑通用资料"
        } else {
            "新建通用资料"
        })
        .id(egui::Id::new("entity-editor"))
        .open(&mut open)
        .default_width(620.0)
        .default_height(760.0)
        .resizable(true)
        .vscroll(true)
        .show(ctx, |ui| {
            keyboard_apply = form.source_selection.is_some()
                && ui.input_mut(|input| {
                    input.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                });
            ui.label(theme::muted("名称"));
            let name_response = ui.add(
                egui::TextEdit::singleline(&mut form.draft.display)
                    .id(egui::Id::new("entity-editor-name"))
                    .desired_width(f32::INFINITY),
            );
            if form._focus_name_on_open {
                name_response.request_focus();
                form._focus_name_on_open = false;
            }
            ui.horizontal_wrapped(|ui| {
                ui.label("分类");
                egui::ComboBox::from_id_salt("entity-kind")
                    .selected_text(
                        ENTITY_KINDS
                            .iter()
                            .find(|(id, _)| *id == form.draft.entity_type)
                            .map(|(_, label)| *label)
                            .unwrap_or("自定义"),
                    )
                    .show_ui(ui, |ui| {
                        for (kind, label) in ENTITY_KINDS {
                            ui.selectable_value(
                                &mut form.draft.entity_type,
                                (*kind).into(),
                                *label,
                            );
                        }
                    });
                ui.text_edit_singleline(&mut form.draft.entity_type);
            });
            ui.label(theme::muted(
                "分类只是作者资料，不生成运行状态，也不要求放到地图上。",
            ));
            ui.label("正文 / 说明");
            ui.add(
                egui::TextEdit::multiline(&mut form.draft.description)
                    .desired_rows(12)
                    .desired_width(f32::INFINITY),
            );
            if let Some(suggestion) = super::templates::template_panel(
                ui,
                "entity",
                Some(&form.draft.entity_type),
                &template_index,
                &catalog,
                &mut form.draft.properties,
            ) {
                if let Some(id) = form.original.clone() {
                    self.edit_relation(None, Some(TargetRef::new("entity", &id)));
                    self.message = Some(format!(
                        "已按“{suggestion}”打开关系草稿；仍需明确选择关系类型和另一端"
                    ));
                } else {
                    self.message = Some("请先保存新资料，再从模板建议打开关系草稿".into());
                }
            }
            egui::CollapsingHeader::new("自定义属性")
                .show(ui, |ui| properties(ui, &mut form.draft.properties));
            egui::CollapsingHeader::new("稳定身份与来源文件").show(ui, |ui| {
                ui.label("显示名称和分类可修改；已有 ID 保持不变，避免断开引用。");
                ui.add_enabled(
                    form.original.is_none(),
                    egui::TextEdit::singleline(&mut form.draft.id),
                );
                ui.label(
                    form.path
                        .strip_prefix(&self.project.root)
                        .unwrap_or(&form.path)
                        .display()
                        .to_string(),
                );
            });
            let current = form.guard.is_current(&self.project, self.version);
            let capable = self.project.language_version() == "1.10";
            if !current {
                ui.colored_label(
                    theme::GOLD,
                    "工程已变化。输入已保留；请复制所需内容并重新打开表单后合并。",
                );
            }
            if !capable {
                ui.colored_label(
                    theme::GOLD,
                    "此工程使用语言 1.9。通用实体需要显式启用 1.10；不会自动迁移旧作品。",
                );
            }
            let apply_clicked = ui
                .add_enabled(
                    current && capable && !form.draft.display.trim().is_empty(),
                    theme::primary("应用资料"),
                )
                .clicked();
            if apply_clicked || keyboard_apply {
                let created_from_source = form.source_selection.is_some();
                let before = self.project.clone();
                let result = form.apply(&mut self.project, self.version);
                applied = self.finish_content_command(
                    before,
                    result,
                    "通用资料已更新；保存全部可写入作品目录",
                );
                if applied {
                    self.catalog_filter = "entity".into();
                    self.catalog_query.clear();
                    self.catalog_target = Some(TargetRef::new("entity", &form.draft.id));
                    self.wiki_target = self.catalog_target.clone();
                    if created_from_source {
                        self.active_file = form.path.clone();
                        self.tab = super::Tab::Edit;
                        let target = TargetRef::new("entity", &form.draft.id);
                        if let Some(link) = self.snapshot.as_ref().and_then(|snapshot| {
                            snapshot
                                .result
                                .analysis
                                .catalog
                                .text_links
                                .iter()
                                .find(|link| {
                                    link.target == target
                                        && Path::new(&link.file) == form.path.as_path()
                                        && form.source_selection.as_ref().is_some_and(|selection| {
                                            link.label == selection.expected_text
                                        })
                                })
                        }) {
                            self.jump = Some((link.line, link.column));
                        }
                        let domain = if self
                            .project
                            .authoring_documents
                            .contains_key(&self.active_file)
                        {
                            "authoring-source"
                        } else {
                            "source"
                        };
                        ctx.memory_mut(|memory| {
                            memory.request_focus(egui::Id::new((domain, &self.active_file)))
                        });
                    } else {
                        self.open_reading(TargetRef::new("entity", &form.draft.id));
                    }
                }
            }
            if let Some(id) = &form.original {
                if ui.button("删除此资料…").clicked() {
                    self.plan_content_deletion(TargetRef::new("entity", id));
                }
            }
            if ui.button("取消").clicked() {
                cancelled = true;
            }
        });
        if open && !applied && !cancelled {
            self.entity_editor = Some(form);
        }
    }
}
