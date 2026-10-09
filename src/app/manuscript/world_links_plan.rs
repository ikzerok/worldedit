use super::world_links_layout::FocusReveal;
use crate::theme;
use std::path::Path;
use worldline_core::manuscript::WritingAuthoringPlan;

pub(super) fn draw(
    ui: &mut egui::Ui,
    root: &Path,
    plan: &WritingAuthoringPlan,
    focus_result: bool,
    focus: FocusReveal,
) {
    let heading = ui.label(egui::RichText::new("核对关联计划").strong());
    if focus_result {
        ui.scroll_to_rect(heading.rect, Some(egui::Align::Min));
    }
    ui.label(format!("稳定目标 {}:{}", plan.target.kind, plan.target.id));
    if plan.creates_object() {
        ui.colored_label(
            theme::WARNING(),
            "确认会应用下列文件的完整当前稿、资料声明和正文引用。",
        );
    } else {
        ui.label("确认只插入当前正文草稿，之后仍须明确应用正文。");
    }
    for buffer in &plan.included_buffers {
        ui.add(
            egui::Label::new(format!(
                "{} · {} · 草稿代次 {}",
                theme::relative_source(root, &buffer.path),
                if buffer.changed {
                    "含全文未应用输入"
                } else {
                    "与已应用稿一致"
                },
                buffer.generation
            ))
            .wrap(),
        );
    }
    if let Some(migration) = &plan.migration {
        ui.separator();
        ui.label(
            egui::RichText::new(format!(
                "显式语言迁移 {} → {}（尚未启用）",
                migration.current_language.as_str(),
                migration.target_language.as_str()
            ))
            .strong(),
        );
        ui.label(format!("新增能力：{}", migration.added_features.join("、")));
        for note in &migration.compatibility_notes {
            ui.label(note);
        }
        for change in &migration.keyword_changes {
            ui.colored_label(
                theme::WARNING(),
                format!(
                    "{}:{} {} → {} · {}",
                    change.file, change.line, change.before_kind, change.after_kind, change.source
                ),
            );
        }
        for diagnostic in &migration.diagnostics_after {
            ui.label(format!(
                "{}:{} {}：{}",
                diagnostic.file, diagnostic.span.line, diagnostic.code, diagnostic.message
            ));
        }
        if !migration.can_apply {
            ui.colored_label(theme::ERROR(), "迁移候选含错误，资料与正文都不会应用。");
        }
    }
    if plan.runtime_fingerprint_before != plan.runtime_fingerprint_after {
        ui.colored_label(
            theme::WARNING(),
            "本次会改变运行指纹，已有运行存档或检查点不能直接沿用。",
        );
    }
    ui.label(egui::RichText::new(format!("全部变更文件（{}）", plan.changes.len())).strong());
    for change in &plan.changes {
        let change = egui::CollapsingHeader::new(theme::relative_source(root, &change.path))
            .id_salt(("world-link-change", &plan.plan_digest, &change.path))
            .show(ui, |ui| {
                ui.label(if change.includes_unapplied_draft {
                    "包含该文件全文草稿"
                } else {
                    "无额外未应用草稿"
                });
                if let Some(before) = &change.before {
                    let before = egui::CollapsingHeader::new("应用前（已应用工程）")
                        .show(ui, |ui| source_pages(ui, before, "before", focus));
                    focus.reveal(ui, &before.header_response);
                } else {
                    ui.label("原文件不存在");
                }
                let after = egui::CollapsingHeader::new("关联后（完整候选）")
                    .show(ui, |ui| source_pages(ui, &change.after, "after", focus));
                focus.reveal(ui, &after.header_response);
            });
        focus.reveal(ui, &change.header_response);
    }
    ui.label(theme::muted("应用不写磁盘；保存后重开仍按稳定身份关联。"));
}

fn source_pages(ui: &mut egui::Ui, source: &str, side: &str, focus: FocusReveal) {
    const PAGE: usize = 16 * 1024;
    let id = ui.make_persistent_id(("world-link-source-page", side));
    let pages = source.len().div_ceil(PAGE).max(1);
    let mut page = ui
        .data(|data| data.get_temp::<usize>(id))
        .unwrap_or(0)
        .min(pages - 1);
    ui.horizontal_wrapped(|ui| {
        ui.label(format!(
            "完整原文 {} 字节 · {}/{} 页",
            source.len(),
            page + 1,
            pages
        ));
        let previous = crate::theme::add_enabled(ui, page > 0, egui::Button::new("前页"));
        focus.reveal(ui, &previous);
        if previous.clicked() {
            page -= 1;
        }
        let next = crate::theme::add_enabled(ui, page + 1 < pages, egui::Button::new("后页"));
        focus.reveal(ui, &next);
        if next.clicked() {
            page += 1;
        }
    });
    ui.data_mut(|data| data.insert_temp(id, page));
    let mut start = (page * PAGE).min(source.len());
    let mut end = ((page + 1) * PAGE).min(source.len());
    while start > 0 && !source.is_char_boundary(start) {
        start -= 1;
    }
    while end > start && !source.is_char_boundary(end) {
        end -= 1;
    }
    let mut text = source[start..end].to_owned();
    ui.add(
        egui::TextEdit::multiline(&mut text)
            .code_editor()
            .interactive(false)
            .desired_width(f32::INFINITY)
            .desired_rows(8),
    );
}
