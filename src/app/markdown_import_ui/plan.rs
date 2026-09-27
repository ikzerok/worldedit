use super::*;
use std::ops::Range;

impl Wizard {
    pub(super) fn show_plan_sections(&mut self, ui: &mut egui::Ui, plan: &MarkdownImportPlan) {
        ui.collapsing(format!("页面映射 · {} 项", plan.pages.len()), |ui| {
            let range = paged_range(plan.pages.len(), &mut self.page_offset, 30);
            show_pager(
                ui,
                plan.pages.len(),
                self.page_offset,
                30,
                &mut self.page_offset,
            );
            for page in &plan.pages[range] {
                ui.label(format!(
                    "{} → entity:{} · {} · {}",
                    page.source, page.id, page.title, page.entity_type
                ));
            }
        });
        ui.collapsing(
            format!("同名资料提示 · {} 项", plan.name_conflicts.len()),
            |ui| {
                let range = paged_range(
                    plan.name_conflicts.len(),
                    &mut self.name_conflict_offset,
                    30,
                );
                show_pager(
                    ui,
                    plan.name_conflicts.len(),
                    self.name_conflict_offset,
                    30,
                    &mut self.name_conflict_offset,
                );
                for conflict in &plan.name_conflicts[range] {
                    ui.label(format!(
                        "{}：{}；现有目标：{}；来源：{}",
                        conflict.title,
                        conflict.message,
                        conflict
                            .existing_targets
                            .iter()
                            .map(|target| format!("{}:{}", target.kind, target.id))
                            .collect::<Vec<_>>()
                            .join("、"),
                        conflict.sources.join("、")
                    ));
                }
            },
        );
        ui.collapsing(format!("来源链接 · {} 项", plan.links.len()), |ui| {
            let range = paged_range(plan.links.len(), &mut self.link_offset, 30);
            show_pager(
                ui,
                plan.links.len(),
                self.link_offset,
                30,
                &mut self.link_offset,
            );
            for link in &plan.links[range] {
                ui.label(format!(
                    "{}:{} · {} → {}:{} · {}",
                    link.source, link.line, link.href, link.target.kind, link.target.id, link.label
                ));
            }
        });
        ui.collapsing(
            format!("附件映射 · {} 项", plan.attachments.len()),
            |ui| {
                let range = paged_range(plan.attachments.len(), &mut self.attachment_offset, 30);
                show_pager(
                    ui,
                    plan.attachments.len(),
                    self.attachment_offset,
                    30,
                    &mut self.attachment_offset,
                );
                for attachment in &plan.attachments[range] {
                    ui.label(format!(
                        "{}:{} · {} → {} · {}",
                        attachment.source,
                        attachment.line,
                        attachment.href,
                        attachment.output_path,
                        attachment.alt
                    ));
                }
            },
        );
        ui.collapsing(format!("损失预览 · {} 项", plan.losses.len()), |ui| {
            let range = paged_range(plan.losses.len(), &mut self.loss_offset, 30);
            show_pager(
                ui,
                plan.losses.len(),
                self.loss_offset,
                30,
                &mut self.loss_offset,
            );
            for loss in &plan.losses[range] {
                ui.label(format!(
                    "{}:{} · {} · {}{}",
                    loss.source,
                    loss.line,
                    loss.code,
                    loss.message,
                    loss.preserved_at
                        .as_ref()
                        .map(|path| format!("；原文保留在 {path}"))
                        .unwrap_or_default()
                ));
            }
        });
        ui.collapsing(format!("阻塞冲突 · {} 项", plan.conflicts.len()), |ui| {
            let range = paged_range(plan.conflicts.len(), &mut self.conflict_offset, 20);
            show_pager(
                ui,
                plan.conflicts.len(),
                self.conflict_offset,
                20,
                &mut self.conflict_offset,
            );
            for conflict in &plan.conflicts[range] {
                ui.separator();
                ui.label(format!(
                    "{}{} · {}",
                    conflict
                        .source
                        .as_ref()
                        .map(|source| format!("{source} · "))
                        .unwrap_or_default(),
                    conflict.code,
                    conflict.message
                ));
                if let Some(source) = &conflict.source {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{source} 的目标 ID"));
                        let value = self
                            .id_overrides
                            .entry(source.clone())
                            .or_insert_with(|| conflict.preferred_id.clone().unwrap_or_default());
                        let changed = ui.add(
                            egui::TextEdit::singleline(value)
                                .hint_text("输入目标 ID")
                                .desired_width(180.0),
                        ).changed();
                        if changed {
                            self.stale = true;
                        }
                    });
                }
                if !conflict.candidates.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("可选 ID");
                        for candidate in &conflict.candidates {
                            if ui.small_button(candidate).clicked() {
                                if let Some(source) = &conflict.source {
                                    self.id_overrides
                                        .insert(source.clone(), candidate.clone());
                                    self.stale = true;
                                } else {
                                    self.namespace = candidate.clone();
                                }
                            }
                        }
                    });
                }
            }
            if !plan.conflicts.is_empty() {
                ui.label("修改映射或命名空间后，点击上方「预检导入」重新计算候选。冲突未解决时应用会保持禁用。");
            }
        });
        ui.collapsing(
            format!("写入文件预览 · {} 项", plan.files.len()),
            |ui| {
                let range = paged_range(plan.files.len(), &mut self.file_offset, 30);
                show_pager(
                    ui,
                    plan.files.len(),
                    self.file_offset,
                    30,
                    &mut self.file_offset,
                );
                for file in &plan.files[range] {
                    ui.label(format!(
                        "{} · {} · {} 字节{}",
                        file.kind,
                        file.path,
                        file.bytes,
                        file.source
                            .as_ref()
                            .map(|source| format!(" · 来源 {source}"))
                            .unwrap_or_default()
                    ));
                }
            },
        );
    }
}
fn paged_range(total: usize, offset: &mut usize, page_size: usize) -> Range<usize> {
    if total == 0 {
        *offset = 0;
        return 0..0;
    }
    *offset = (*offset / page_size) * page_size;
    *offset = (*offset).min((total - 1) / page_size * page_size);
    *offset..(*offset + page_size).min(total)
}

fn show_pager(
    ui: &mut egui::Ui,
    total: usize,
    offset: usize,
    page_size: usize,
    next_offset: &mut usize,
) {
    if total <= page_size {
        return;
    }
    ui.horizontal(|ui| {
        let end = (offset + page_size).min(total);
        ui.label(format!("显示 {}–{} / {total}", offset + 1, end));
        if ui
            .add_enabled(offset > 0, egui::Button::new("上一页"))
            .clicked()
        {
            *next_offset = offset.saturating_sub(page_size);
        }
        if ui
            .add_enabled(end < total, egui::Button::new("下一页"))
            .clicked()
        {
            *next_offset = end;
        }
    });
}
