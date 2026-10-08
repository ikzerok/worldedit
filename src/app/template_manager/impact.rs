use super::*;
mod properties;

pub(super) fn show(ui: &mut egui::Ui, preview: &ProjectTemplatePreview, page: &mut usize) {
    ui.separator();
    ui.heading(match &preview.mutation {
        ProjectTemplateMutation::Import { .. } => "导入影响预览",
        ProjectTemplateMutation::Replace { .. } => "替换影响预览",
        ProjectTemplateMutation::Delete { .. } => "停用影响预览",
        ProjectTemplateMutation::RepairInvalid { .. } => "修复影响预览",
    });
    if !preview.complete {
        ui.colored_label(
            theme::ERROR(),
            "影响检查不完整，不能应用；以下已知对象不代表全部实例。",
        );
        if let Some(reason) = &preview.incomplete_reason {
            ui.label(reason);
        }
    }
    for (label, template) in [
        ("原模板", preview.current_template.as_ref()),
        ("新模板", preview.proposed_template.as_ref()),
    ] {
        if let Some(template) = template {
            ui.label(format!(
                "{label}：{} · {} · {} / {}",
                template.title,
                template.id,
                template.applies_to.kind,
                template.applies_to_entity_type.as_deref().unwrap_or("全部")
            ));
        }
    }
    ui.label("字段变化");
    if preview.field_changes.is_empty() {
        ui.label(theme::muted(
            "没有字段结构变化；模板名称或适用范围仍以上方信息为准。",
        ));
    }
    for change in &preview.field_changes {
        ui.label(format!(
            "{} · {} · {} → {} · {:?} → {:?}",
            change_label(&change.change),
            change.field_id,
            change.old_key.as_deref().unwrap_or("—"),
            change.new_key.as_deref().unwrap_or("—"),
            change.old_type,
            change.new_type
        ));
        properties::show(ui, change);
        if change.change == "position_changed" {
            ui.label(format!(
                "位置：{} 第 {} 项 → {} 第 {} 项",
                change.old_parent_id.as_deref().unwrap_or("根层"),
                change.old_index.map(|index| index + 1).unwrap_or(0),
                change.new_parent_id.as_deref().unwrap_or("根层"),
                change.new_index.map(|index| index + 1).unwrap_or(0)
            ));
        }
    }
    ui.label(if preview.complete {
        format!("实例影响 · {} 个对象", preview.instances.len())
    } else {
        format!(
            "实例影响 · 已知 {} 个对象（不完整）",
            preview.instances.len()
        )
    });
    let total = preview.instances.len();
    ui.push_id("template-impact-pages", |ui| {
        ui.horizontal_wrapped(|ui| {
            let _ = page_range(total, page);
            let pages = total.div_ceil(PAGE_SIZE).max(1);
            if theme::add_enabled(ui, *page > 0, egui::Button::new("首页")).clicked() {
                *page = 0;
            }
            if theme::add_enabled(ui, *page > 0, egui::Button::new("上一页")).clicked() {
                *page -= 1;
            }
            if theme::add_enabled(ui, *page + 1 < pages, egui::Button::new("下一页")).clicked() {
                *page += 1;
            }
            if theme::add_enabled(ui, *page + 1 < pages, egui::Button::new("末页")).clicked() {
                *page = pages - 1;
            }
            ui.label(format!("第 {} / {pages} 页", *page + 1));
        });
    });
    let range = page_range(total, page);
    ui.label(if total == 0 {
        "当前没有已知实例行".into()
    } else {
        format!(
            "显示 {}–{} / {total} 个对象；应用与导出仍依据完整报告",
            range.start + 1,
            range.end
        )
    });
    for instance in &preview.instances[range] {
        ui.label(format!(
            "{}:{} · 原模板{} / 新模板{}",
            instance.target.kind,
            instance.target.id,
            if instance.current_applicable {
                "适用"
            } else {
                "不适用"
            },
            if instance.proposed_applicable {
                "适用"
            } else {
                "不适用"
            }
        ));
        for field in &instance.fields {
            ui.label(format!(
                "{} · {} · {} · {:?}",
                field.template_state, field.key, field.field_id, field.state
            ));
        }
    }
    for diagnostic in &preview.diagnostics {
        ui.colored_label(
            if diagnostic.severity == worldline_core::Severity::Error {
                theme::ERROR()
            } else {
                theme::GOLD()
            },
            format!("{} · {}", diagnostic.code, diagnostic.message),
        );
    }
    ui.label(theme::muted(
        "预览不会改写实例值；字段类型变化不会转换或清除现有资料。",
    ));
}
fn change_label(change: &str) -> &str {
    match change {
        "added" => "新增字段",
        "removed" => "删除字段",
        "renamed" => "属性 key 变化",
        "type_changed" => "类型变化",
        "label_changed" => "显示名称变化",
        "constraints_changed" => "约束提示变化",
        "position_changed" => "字段位置变化",
        _ => change,
    }
}

const PAGE_SIZE: usize = 50;
pub(super) fn page_range(total: usize, page: &mut usize) -> std::ops::Range<usize> {
    *page = (*page).min(total.saturating_sub(1) / PAGE_SIZE);
    let start = *page * PAGE_SIZE;
    start..start.saturating_add(PAGE_SIZE).min(total)
}
