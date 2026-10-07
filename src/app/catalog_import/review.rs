use super::*;
use crate::theme;
use worldline_core::ast::PropertyValue;

impl ImportState {
    pub(super) fn review_ui(&mut self, app: &mut WorldeditApp, ui: &mut egui::Ui) {
        let Some(plan) = self.plan.as_ref() else {
            ui.label("尚无预览，请先完成映射并刷新。");
            return;
        };
        let count = |op| plan.rows.iter().filter(|row| row.operation == op).count();
        ui.add(
            egui::Label::new(format!(
                "新增 {} · 更新 {} · 无变化 {} · 阻断 {} · 全部错误 {}",
                count("create"),
                count("update"),
                count("unchanged"),
                count("blocked"),
                plan.error_count
            ))
            .wrap(),
        );
        match plan.runtime_fingerprint_after {
            Some(after) if after != plan.runtime_fingerprint_before => {
                ui.colored_label(
                    theme::WARNING(),
                    "运行指纹将改变 · 人物显示名或标量等执行资料变化可能使旧存档不兼容",
                );
            }
            Some(_) => {
                ui.label("本批运行指纹不变；仍保留既有存档与严格重放校验。");
            }
            None => {
                ui.label("候选无效，无法判断最终运行指纹；不可应用。");
            }
        }
        if !plan.can_apply && plan.error_count == 0 {
            ui.colored_label(theme::WARNING(), "核心未允许应用此批次");
        }
        ui.collapsing(
            format!(
                "批次详情 · 忽略 {} 列 · {} 个文件",
                plan.ignored_columns.len(),
                plan.changed_files.len()
            ),
            |ui| {
                ui.label(format!(
                    "运行指纹：{} → {}",
                    plan.runtime_fingerprint_before,
                    plan.runtime_fingerprint_after
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "无有效候选".into())
                ));
                ui.label(format!(
                    "CSV 引号内换行规范化：{} 处",
                    plan.normalization_count
                ));
                ui.label(format!("新增对象目标：{}", plan.destination.display()));
                if plan.ignored_columns.is_empty() {
                    ui.label("忽略列：无");
                }
                for name in &plan.ignored_columns {
                    ui.add(egui::Label::new(format!("明确忽略：{name}")).wrap());
                }
                for file in &plan.changed_files {
                    ui.add(egui::Label::new(format!("修改来源：{}", file.display())).wrap());
                }
            },
        );
        if !plan.diagnostics.is_empty() {
            egui::CollapsingHeader::new(format!(
                "错误详情 · 显示 {} / {}",
                plan.diagnostics.len(),
                plan.error_count
            ))
            .default_open(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("catalog-import-errors")
                    .max_height(100.0)
                    .show(ui, |ui| {
                        for diagnostic in &plan.diagnostics {
                            ui.add(
                                egui::Label::new(format!(
                                    "{} · 数据行 {} · 列 {} · CSV 第 {} 行：{}",
                                    diagnostic.code,
                                    diagnostic
                                        .row
                                        .map(|v| v.to_string())
                                        .unwrap_or_else(|| "整批".into()),
                                    diagnostic
                                        .column
                                        .map(|v| v.to_string())
                                        .unwrap_or_else(|| "—".into()),
                                    diagnostic
                                        .line
                                        .map(|v| v.to_string())
                                        .unwrap_or_else(|| "—".into()),
                                    diagnostic.message
                                ))
                                .wrap(),
                            );
                        }
                        if plan.error_count > plan.diagnostics.len() {
                            ui.label("错误显示已达上限；未展示的错误仍阻断整批应用。");
                        }
                    });
            });
            if ui.button("显式启用语言与资料能力…").clicked() {
                app.open_capabilities();
            }
        }
        if plan.rows.is_empty() {
            ui.label("没有可审阅的数据行；工程未变化。");
            return;
        }
        self.selected_row = self.selected_row.min(plan.rows.len() - 1);
        ui.separator();
        let remaining = remaining_height(ui);
        if ui.available_width() >= 780.0 {
            ui.columns(2, |columns| {
                egui::ScrollArea::vertical()
                    .id_salt("catalog-import-rows-wide")
                    .max_height(remaining)
                    .min_scrolled_height(0.0)
                    .auto_shrink([false, false])
                    .show(&mut columns[0], |ui| {
                        rows_ui(plan, &mut self.selected_row, ui)
                    });
                egui::ScrollArea::vertical()
                    .id_salt("catalog-import-field-details-wide")
                    .max_height(remaining)
                    .min_scrolled_height(0.0)
                    .auto_shrink([false, false])
                    .show(&mut columns[1], |ui| {
                        row_detail(plan, self.selected_row, app, ui)
                    });
            });
        } else {
            egui::ScrollArea::vertical()
                .id_salt("catalog-import-rows-narrow")
                .max_height(remaining * 0.38)
                .min_scrolled_height(0.0)
                .auto_shrink([false, false])
                .show(ui, |ui| rows_ui(plan, &mut self.selected_row, ui));
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("catalog-import-field-details-narrow")
                .max_height(remaining_height(ui))
                .min_scrolled_height(0.0)
                .auto_shrink([false, false])
                .show(ui, |ui| row_detail(plan, self.selected_row, app, ui));
        }
    }
}

fn rows_ui(plan: &CatalogImportPlan, selected: &mut usize, ui: &mut egui::Ui) {
    ui.label("数据行 · 选择后查看完整字段差异");
    for (index, row) in plan.rows.iter().enumerate() {
        let target = row
            .target
            .as_ref()
            .map(|t| format!("{}:{}", t.kind, t.id))
            .unwrap_or_else(|| "身份无效".into());
        let status = match row.operation.as_str() {
            "create" => "新增",
            "update" => "更新",
            "unchanged" => "无变化",
            _ => "阻断",
        };
        let label = format!("第 {} 行 · {status} · {target}", row.row);
        if ui
            .add(egui::Button::new(label).selected(*selected == index).wrap())
            .clicked()
        {
            *selected = index;
        }
    }
}

fn row_detail(plan: &CatalogImportPlan, index: usize, app: &mut WorldeditApp, ui: &mut egui::Ui) {
    let row = &plan.rows[index];
    ui.heading(format!("第 {} 行字段详情", row.row));
    ui.label(format!("CSV 物理行：{}", row.line));
    if let Some(source) = &row.source {
        ui.add(egui::Label::new(format!("源码：{}", source.display())).wrap());
    }
    if let Some(target) = &row.target {
        ui.add(egui::Label::new(format!("身份：{}:{}", target.kind, target.id)).wrap());
        let object = app
            .snapshot
            .as_ref()
            .and_then(|s| s.result.analysis.catalog.object(target))
            .cloned();
        ui.horizontal_wrapped(|ui| {
            if crate::theme::add_enabled(ui, object.is_some(), egui::Button::new("看资料"))
                .clicked()
            {
                app.open_reading(target.clone());
            }
            if crate::theme::add_enabled(ui, object.is_some(), egui::Button::new("回到来源"))
                .clicked()
            {
                if let Some(object) = &object {
                    app.jump_to_file(&object.file, object.line, 1);
                }
            }
        });
        if object.is_none() {
            ui.label("此对象尚未应用或当前不存在；应用成功后可看资料与回源。");
        }
    }
    if row.fields.is_empty() {
        ui.label("没有字段变化；未映射字段保持原样。");
    }
    for field in &row.fields {
        ui.separator();
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!("{} · {}", field.field, field.value_type)).strong(),
            )
            .wrap(),
        );
        ui.add(egui::Label::new(format!("原值：{}", value_label(field.before.as_ref()))).wrap());
        ui.add(egui::Label::new(format!("新值：{}", value_label(field.after.as_ref()))).wrap());
    }
}
fn value_label(value: Option<&PropertyValue>) -> String {
    match value {
        None => "（未设置）".into(),
        Some(PropertyValue::Str(text)) => format!("{text:?}"),
        Some(PropertyValue::Num(number)) => number.to_string(),
        Some(PropertyValue::Bool(value)) => value.to_string(),
        Some(PropertyValue::Ref(target)) => format!("{}:{}", target.kind, target.id),
    }
}

fn remaining_height(ui: &egui::Ui) -> f32 {
    // The clip bottom is captured immediately after the fixed footer is drawn;
    // unlike max_rect, it cannot grow as a taller child lays out its content.
    (ui.clip_rect().bottom() - ui.cursor().min.y)
        .min(ui.available_height())
        .max(0.0)
}
