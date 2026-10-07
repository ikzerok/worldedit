use super::*;
use worldline_core::reader_export::READER_SITE_SCHEMA_VERSION;
pub(super) const PAGE_SIZE: usize = 100;

pub(super) fn page_range(
    ui: &mut egui::Ui,
    page: &mut usize,
    count: usize,
) -> std::ops::Range<usize> {
    let pages = count.div_ceil(PAGE_SIZE).max(1);
    *page = (*page).min(pages - 1);
    ui.horizontal_wrapped(|ui| {
        if crate::theme::add_enabled(ui, *page > 0, egui::Button::new("上一页")).clicked() {
            *page -= 1;
        }
        ui.label(format!("第 {} / {} 页 · 共 {} 项", *page + 1, pages, count));
        if crate::theme::add_enabled(ui, *page + 1 < pages, egui::Button::new("下一页")).clicked()
        {
            *page += 1;
        }
    });
    let start = *page * PAGE_SIZE;
    start..(start + PAGE_SIZE).min(count)
}

impl ReaderPublishState {
    pub(super) fn selection_ui(&mut self, ui: &mut egui::Ui) -> (bool, Option<PublishAction>) {
        if self.reviewed.is_none() {
            ui.label("尚未生成预览；未选任何内容时不会创建空包。");
        }
        let action = self.profile_controls(ui);
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("读者站点标题");
            changed |= ui
                .add(
                    egui::TextEdit::singleline(&mut self.site_title)
                        .desired_width(ui.available_width()),
                )
                .changed();
        });
        let v3 = self
            .profile
            .as_ref()
            .is_none_or(|profile| profile.selection.schema_version == READER_SITE_SCHEMA_VERSION);
        if v3 {
            ui.label("选中资料会公开全部别名与类型结构；属性字段、地图、章节和附件仍须逐项勾选。");
            changed |= ui
                .checkbox(
                    &mut self.story_details,
                    "公开静态条件与效果说明（不执行故事）",
                )
                .changed();
        } else {
            ui.label("当前配置保持旧版公开语义；升级须先预览并确认新增授权。");
        }
        ui.horizontal_wrapped(|ui| {
            for (group, label) in [
                (SelectionGroup::Objects, "资料对象"),
                (SelectionGroup::Maps, "地图"),
                (SelectionGroup::Chapters, "书稿章节"),
                (SelectionGroup::Attachments, "附件"),
            ] {
                ui.selectable_value(&mut self.group, group, label);
            }
        });
        if ui
            .add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("搜索名称、别名、类型或 ID")
                    .desired_width(f32::INFINITY),
            )
            .changed()
        {
            self.object_page = 0;
            self.map_page = 0;
        }
        match self.group {
            SelectionGroup::Objects => changed |= self.object_selection_ui(ui),
            SelectionGroup::Maps => changed |= self.map_choices_ui(ui),
            SelectionGroup::Chapters => changed |= self.chapter_selection_ui(ui),
            SelectionGroup::Attachments => changed |= self.attachment_selection_ui(ui),
        }
        changed |= self.unavailable_ui(ui);
        (changed, action)
    }

    fn object_selection_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        let kinds = self
            .object_choices
            .iter()
            .map(|choice| choice.target.kind.as_str())
            .collect::<BTreeSet<_>>();
        egui::ComboBox::from_id_salt("reader-object-kind")
            .selected_text(if self.kind_filter.is_empty() {
                "全部类型"
            } else {
                &self.kind_filter
            })
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(&mut self.kind_filter, String::new(), "全部类型")
                    .changed()
                {
                    self.object_page = 0;
                }
                for kind in kinds {
                    if ui
                        .selectable_value(&mut self.kind_filter, kind.to_owned(), kind)
                        .changed()
                    {
                        self.object_page = 0;
                    }
                }
            });
        let query = self.query.trim().to_lowercase();
        let choices = self
            .object_choices
            .iter()
            .filter(|choice| {
                (self.kind_filter.is_empty() || choice.target.kind == self.kind_filter)
                    && [&choice.display, &choice.target.kind, &choice.target.id]
                        .into_iter()
                        .chain(choice.aliases.iter())
                        .any(|value| value.to_lowercase().contains(&query))
            })
            .collect::<Vec<_>>();
        ui.label(format!(
            "{} 项匹配 / {} 项资料 · 已选 {} 项",
            choices.len(),
            self.object_choices.len(),
            self.objects.len()
        ));
        ui.horizontal_wrapped(|ui| {
            if ui.button("选择全部筛选资料").clicked() {
                self.objects
                    .extend(choices.iter().map(|choice| choice.target.clone()));
                changed = true;
            }
            if ui.button("取消全部筛选资料").clicked() {
                for choice in &choices {
                    self.objects.remove(&choice.target);
                    self.fields.remove(&choice.target);
                }
                changed = true;
            }
        });
        ui.label("批量操作包括所有匹配页；不自动选择属性字段、地图、章节或附件。");
        let range = page_range(ui, &mut self.object_page, choices.len());
        for choice in &choices[range] {
            ui.push_id((&choice.target.kind, &choice.target.id), |ui| {
                let mut checked = self.objects.contains(&choice.target);
                if ui
                    .checkbox(
                        &mut checked,
                        format!(
                            "{} · {} ({})",
                            choice.display, choice.target.kind, choice.target.id
                        ),
                    )
                    .changed()
                {
                    if checked {
                        self.objects.insert(choice.target.clone());
                    } else {
                        self.objects.remove(&choice.target);
                        self.fields.remove(&choice.target);
                    }
                    changed = true;
                }
                if checked && !choice.fields.is_empty() {
                    egui::CollapsingHeader::new(format!(
                        "公开属性（{} 项，已选 {} 项）",
                        choice.fields.len(),
                        self.fields.get(&choice.target).map_or(0, BTreeSet::len)
                    ))
                    .id_salt("fields")
                    .default_open(self.objects.len() <= 10)
                    .show(ui, |ui| {
                        ui.label("属性默认不选；键名和值都公开");
                        for field in &choice.fields {
                            let keys = self.fields.entry(choice.target.clone()).or_default();
                            let mut selected = keys.contains(&field.key);
                            if ui
                                .checkbox(
                                    &mut selected,
                                    format!("{}：{}", field.key, field.preview),
                                )
                                .changed()
                            {
                                if selected {
                                    keys.insert(field.key.clone());
                                } else {
                                    keys.remove(&field.key);
                                }
                                changed = true;
                            }
                        }
                    });
                }
            });
        }
        if choices.is_empty() {
            ui.label("没有匹配的资料；请调整类型或搜索。");
        }
        changed
    }

    fn chapter_selection_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let query = self.query.trim().to_lowercase();
        let mut changed = false;
        for book in &self.manuscript_choices {
            ui.strong(&book.title);
            if let Some(reason) = &book.unavailable {
                ui.colored_label(crate::theme::WARNING(), reason);
                continue;
            }
            for (id, title) in &book.chapters {
                if ![&book.title, &book.id, id, title]
                    .iter()
                    .any(|value| value.to_lowercase().contains(&query))
                {
                    continue;
                }
                let selected = self.chapters.entry(book.id.clone()).or_default();
                let mut checked = selected.contains(id);
                if ui
                    .checkbox(&mut checked, format!("{title} ({id})"))
                    .changed()
                {
                    if checked {
                        selected.insert(id.clone());
                    } else {
                        selected.remove(id);
                    }
                    changed = true;
                }
            }
        }
        if self.manuscript_choices.is_empty() {
            ui.label("当前工程没有注册书稿。");
        }
        changed
    }

    fn attachment_selection_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        let query = self.query.trim().to_lowercase();
        for choice in &self.attachment_choices {
            if ![&choice.id, &choice.display]
                .iter()
                .any(|value| value.to_lowercase().contains(&query))
            {
                continue;
            }
            let mut checked = self.attachments.contains(&choice.id);
            if crate::theme::add_enabled(
                ui,
                choice.available,
                egui::Checkbox::new(&mut checked, format!("{} ({})", choice.display, choice.id)),
            )
            .changed()
            {
                if checked {
                    self.attachments.insert(choice.id.clone());
                } else {
                    self.attachments.remove(&choice.id);
                }
                changed = true;
            }
            if !choice.available {
                ui.colored_label(
                    crate::theme::WARNING(),
                    "此附件不可用；已载选择不会自动移除。",
                );
            }
        }
        if self.attachment_choices.is_empty() {
            ui.label("当前工程没有已登记附件。");
        }
        changed
    }
}

#[cfg(test)]
mod tests;
