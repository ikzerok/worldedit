//! 通用实体创作页；所有类型使用相同的 core EntityDraft。
use super::authoring_forms::EntityForm;
use super::inspector::{field, properties};
use super::WorldeditApp;
use crate::theme;
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
        let mut open = true;
        let mut applied = false;
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
            field(ui, "名称", &mut form.draft.display);
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
            if ui
                .add_enabled(
                    current && capable && !form.draft.display.trim().is_empty(),
                    theme::primary("应用资料"),
                )
                .clicked()
            {
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
                    self.open_reading(TargetRef::new("entity", &form.draft.id));
                }
            }
            if let Some(id) = &form.original {
                if ui.button("删除此资料…").clicked() {
                    self.plan_content_deletion(TargetRef::new("entity", id));
                }
            }
        });
        if open && !applied {
            self.entity_editor = Some(form);
        }
    }
}
