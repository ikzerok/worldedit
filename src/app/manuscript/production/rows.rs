use super::*;
use worldline_core::localization::LocalizationPart;

pub(super) fn status_label(status: ProductionStatus) -> &'static str {
    match status {
        ProductionStatus::Source => "源文",
        ProductionStatus::Translated => "已译",
        ProductionStatus::Missing => "缺译",
        ProductionStatus::Stale => "过期",
        ProductionStatus::Invalid => "无效",
    }
}
pub(super) fn summary(ui: &mut egui::Ui, snapshot: &ProductionScriptSnapshot, offset: &mut usize) {
    let summary = snapshot.summary();
    ui.separator();
    ui.strong(if summary.includes_fragment_closure {
        "完整静态定义范围 · 条件未求值"
    } else {
        "直接定义范围 · 调用片段未扩展"
    });
    ui.label(format!(
        "{} 章出现 · {} 根目标 · {} 定义（新增 {} 片段） · {} 调用点 · {} 源文件 · {} 匹配行",
        summary.selected_chapter_occurrences,
        summary.root_targets,
        summary.definition_count,
        summary.added_fragment_definitions,
        summary.call_sites,
        summary.source_files,
        summary.matching_rows
    ));
    ui.label(theme::muted("同一来源只列一次；调用点不是运行次数，排列不是执行顺序。人物名称与演出备注为源语言元数据。"));
    egui::CollapsingHeader::new("核对纳入的定义与调用关系").show(ui, |ui| {
        let definitions = snapshot.definitions();
        *offset = (*offset).min(definitions.len().saturating_sub(1) / 20 * 20);
        ui.horizontal_wrapped(|ui| {
            if theme::add_enabled(ui, *offset > 0, egui::Button::new("前20定义")).clicked() {
                *offset = offset.saturating_sub(20);
            }
            if theme::add_enabled(
                ui,
                *offset + 20 < definitions.len(),
                egui::Button::new("后20定义"),
            )
            .clicked()
            {
                *offset += 20;
            }
            ui.label(format!(
                "定义 {}–{} / {}",
                if definitions.is_empty() {
                    0
                } else {
                    *offset + 1
                },
                (*offset + 20).min(definitions.len()),
                definitions.len()
            ));
        });
        for definition in definitions.iter().skip(*offset).take(20) {
            ui.label(format!(
                "{} · {}:{} · {}:{}",
                if definition.is_root {
                    "所选根定义"
                } else {
                    "纳入的共享片段定义"
                },
                definition.target.kind,
                definition.target.id,
                definition.source.file,
                definition.source.line
            ));
        }
        ui.collapsing(
            format!("完整调用表 · {}处", snapshot.call_sites().len()),
            |ui| {
                let range = context_page(ui, snapshot.call_sites().len(), "production-all-calls");
                for usage in &snapshot.call_sites()[range] {
                    ui.label(format!(
                        "{}:{} → {}:{} · {}:{}",
                        usage.caller.kind,
                        usage.caller.id,
                        usage.callee.kind,
                        usage.callee.id,
                        usage.source.file,
                        usage.source.line
                    ));
                    controls(ui, &usage.control_ancestry);
                }
            },
        );
        ui.collapsing(
            format!("所选章节出现 · {}次", snapshot.chapter_occurrences().len()),
            |ui| {
                let range = context_page(
                    ui,
                    snapshot.chapter_occurrences().len(),
                    "production-all-chapters",
                );
                for chapter in &snapshot.chapter_occurrences()[range] {
                    ui.label(format!(
                        "{} / {} → {}:{}",
                        chapter.manuscript_id,
                        chapter.chapter_id,
                        chapter.target.kind,
                        chapter.target.id
                    ));
                }
            },
        );
    });
}
pub(super) fn row(
    ui: &mut egui::Ui,
    snapshot: &ProductionScriptSnapshot,
    row: &ProductionRow,
    enabled: bool,
    selected: &mut Option<ProductionRow>,
    reference: &mut Option<worldline_core::TargetRef>,
) {
    ui.push_id(&row.row_key, |ui| {
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if let Some(speaker) = &row.speaker {
                if theme::add_enabled(
                    ui,
                    enabled,
                    egui::Button::new(egui::RichText::new(&speaker.display).strong()).frame(false),
                )
                .clicked()
                {
                    *reference = Some(speaker.target.clone());
                }
                ui.label(theme::muted(format!(
                    "{}:{}",
                    speaker.target.kind, speaker.target.id
                )));
            } else {
                ui.strong(match row.kind {
                    ProductionKind::Choice => "选择文案",
                    _ => "旁白",
                });
            }
            ui.label(status_label(row.status));
            if row.used_source_fallback {
                ui.colored_label(theme::WARNING(), "源文回退");
            }
            if theme::add_enabled(ui, enabled, egui::Button::new("回到原文")).clicked() {
                *selected = Some(row.clone());
            }
        });
        parts(ui, &row.selected_parts);
        if row.target_locale.is_some() {
            egui::CollapsingHeader::new("源文对照").show(ui, |ui| parts(ui, &row.source_parts));
        }
        egui::CollapsingHeader::new(format!(
            "来源与上下文 · {}:{}",
            row.source.file, row.source.line
        ))
        .show(ui, |ui| {
            ui.label(format!(
                "{}:{} · locale={} · {}",
                row.declaration.kind,
                row.declaration.id,
                row.target_locale.as_deref().unwrap_or("source"),
                row.stable_line_id.as_deref().unwrap_or("无持久行 ID")
            ));
            ui.label(theme::muted(format!(
                "源 revision：{}；当前锚仅适用于此快照",
                row.source_revision
            )));
            ui.strong("定义内部结构 · 未求值");
            controls(ui, &row.control_ancestry);
            ui.strong("外部调用使用关系 · 与内部条件分开");
            if row.external_call_uses.is_empty() {
                ui.label(theme::muted("此选定图中无直接外部调用"));
            }
            let range = context_page(ui, row.external_call_uses.len(), "production-row-calls");
            for usage in &row.external_call_uses[range] {
                ui.label(format!(
                    "{}:{} → {}:{} · {}:{}",
                    usage.caller.kind,
                    usage.caller.id,
                    usage.callee.kind,
                    usage.callee.id,
                    usage.source.file,
                    usage.source.line
                ));
                controls(ui, &usage.control_ancestry);
            }
            egui::CollapsingHeader::new("作者私密演出备注（源语言）").show(ui, |ui| {
                match snapshot.author_direction(&row.row_key) {
                    Some(direction) => {
                        ui.add(egui::Label::new(direction).wrap());
                    }
                    None => {
                        ui.label(theme::muted("没有演出备注"));
                    }
                }
                ui.label(theme::muted("此处查看不会启用交付备注；导出默认不含备注。"));
            });
        });
    });
}
fn controls(ui: &mut egui::Ui, controls: &[ProductionControl]) {
    if controls.is_empty() {
        ui.label(theme::muted("无结构祖先"));
    }
    for control in controls {
        ui.label(theme::muted(format!(
            "{} · {}:{} · 未求值{}{}{}",
            control.kind,
            control.source.file,
            control.source.line,
            control
                .condition
                .as_ref()
                .map(|text| format!(" · 条件 {text}"))
                .unwrap_or_default(),
            control
                .enable
                .as_ref()
                .map(|text| format!(" · 可选 {text}"))
                .unwrap_or_default(),
            if control.once { " · once" } else { "" }
        )));
    }
}
pub(super) fn parts(ui: &mut egui::Ui, parts: &[LocalizationPart]) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for part in parts {
            match part {
                LocalizationPart::Text { text } => {
                    ui.add(
                        egui::Label::new(egui::RichText::new(text).font(theme::body_font(
                            ui.text_style_height(&egui::TextStyle::Body),
                        )))
                        .wrap(),
                    );
                }
                LocalizationPart::Placeholder { token } => {
                    ui.label(
                        egui::RichText::new(format!("⟦{token} · 未求值⟧")).color(theme::BLUE()),
                    );
                }
                LocalizationPart::Link { token, label } => {
                    ui.label(egui::RichText::new(label).color(theme::BLUE()))
                        .on_hover_text(token);
                }
            }
        }
    });
}

fn context_page(ui: &mut egui::Ui, total: usize, role: &str) -> std::ops::Range<usize> {
    let id = ui.make_persistent_id(role);
    let mut offset = ui
        .ctx()
        .data(|data| data.get_temp::<usize>(id))
        .unwrap_or(0)
        .min(total.saturating_sub(1) / 20 * 20);
    ui.horizontal_wrapped(|ui| {
        if theme::add_enabled(ui, offset > 0, egui::Button::new("前20关系")).clicked() {
            offset = offset.saturating_sub(20);
        }
        if theme::add_enabled(ui, offset + 20 < total, egui::Button::new("后20关系")).clicked() {
            offset += 20;
        }
        ui.label(format!(
            "{}–{} / {}",
            if total == 0 { 0 } else { offset + 1 },
            (offset + 20).min(total),
            total
        ));
    });
    ui.ctx().data_mut(|data| data.insert_temp(id, offset));
    offset..(offset + 20).min(total)
}
