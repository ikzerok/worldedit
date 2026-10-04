//! 审稿只排版 core 的结构树，不解释 DSL，不切碎中英文段落。
use super::review_navigation::ReviewRequest;
use crate::{app::writing_workspace::Typography, theme};
use std::sync::Arc;
use worldline_core::{
    manuscript::{ReviewKind, ReviewNode, ReviewPart, ReviewProjection},
    TargetRef,
};

#[derive(Default)]
pub(super) struct Actions {
    pub source: Option<ReviewRequest>,
    pub reference: Option<TargetRef>,
}

pub(super) fn draw(
    ui: &mut egui::Ui,
    review: &Arc<ReviewProjection>,
    key: &str,
    typography: Typography,
    blocked: Option<&str>,
    actions: &mut Actions,
) {
    for (index, node) in review.nodes.iter().enumerate() {
        draw_node(ui, node, &format!("{}", index + 1), 0, review, key, typography, blocked, actions);
    }
}

fn is_body(kind: ReviewKind) -> bool {
    matches!(kind, ReviewKind::Text | ReviewKind::Say | ReviewKind::Description)
}

fn kind_label(kind: ReviewKind) -> &'static str {
    match kind {
        ReviewKind::Text => "正文",
        ReviewKind::Say => "台词",
        ReviewKind::Description => "描述",
        ReviewKind::If => "条件组 · 各分支互斥",
        ReviewKind::Branch => "条件分支",
        ReviewKind::ChoiceGroup => "选择组 · 各选择分别审阅",
        ReviewKind::Choice => "选择",
        ReviewKind::Scene => "场景边界",
        ReviewKind::Call => "片段调用 · 未展开",
        ReviewKind::Return => "返回调用处",
        ReviewKind::Divert => "流程去向",
        ReviewKind::Structure => "结构说明 · 未执行",
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_node(
    ui: &mut egui::Ui,
    node: &ReviewNode,
    number: &str,
    depth: usize,
    review: &Arc<ReviewProjection>,
    key: &str,
    typography: Typography,
    blocked: Option<&str>,
    actions: &mut Actions,
) {
    ui.push_id(number, |ui| {
        let body = is_body(node.kind);
        let inset = (depth.min(3) as f32 * 10.0).min(ui.available_width() / 8.0);
        ui.horizontal_top(|ui| {
            ui.add_space(inset);
            ui.vertical(|ui| {
                ui.set_max_width((ui.available_width() - 2.0).max(1.0));
                let frame = if body { egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)) }
                    else { egui::Frame::new().fill(theme::PANEL()).stroke(egui::Stroke::new(1.0_f32, theme::MUTED())).inner_margin(8) };
                frame.show(ui, |ui| {
                    if !body {
                        ui.label(egui::RichText::new(format!("┌ {} · {} · 第{}层", kind_label(node.kind), number, depth + 1)).strong());
                        if !node.label.is_empty() { ui.add(egui::Label::new(&node.label).wrap()); }
                    }
                    if let Some(speaker) = &node.speaker {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new(&speaker.display).strong());
                            let response = ui.small_button(format!("人物 {}:{}", speaker.target.kind, speaker.target.id));
                            if response.clicked() { actions.reference = Some(speaker.target.clone()); }
                            focus_feedback(ui, &response);
                        });
                    }
                    if body && !node.label.is_empty() {
                        ui.add(egui::Label::new(theme::muted(if node.kind == ReviewKind::Say {
                            format!("演出说明：{}", node.label)
                        } else { node.label.clone() })).wrap());
                    }
                    if let Some(condition) = &node.condition {
                        ui.add(egui::Label::new(format!("可见条件（未求值）：{condition}")).wrap());
                    }
                    if let Some(enable) = &node.enable {
                        ui.add(egui::Label::new(format!("可选条件（未求值）：{enable}")).wrap());
                    }
                    if node.once { ui.label("一次性选择 · 此处未判定是否已选"); }
                    if let Some(reason) = &node.disabled_reason { ui.add(egui::Label::new(format!("不可选说明：{reason}")).wrap()); }
                    if !node.parts.is_empty() {
                        paragraph(ui, &node.parts, typography, actions);
                    }
                    if node.glue { ui.label(theme::muted("原文含粘接标记 · 不跨分支合并")); }
                    if let Some(target) = &node.target {
                        ui.add(egui::Label::new(theme::muted(format!("目标身份：{}:{}", target.kind, target.id))).wrap());
                    }
                    if let Some(source) = &node.source {
                        ui.horizontal_wrapped(|ui| {
                            let response = ui.add_enabled(blocked.is_none(), egui::Button::new("定位原文").small());
                            let response = response.on_hover_text(blocked.unwrap_or("定位当前文件草稿的真实源码范围；不会应用或保存。Alt+Left 返回"));
                            if response.clicked() {
                                actions.source = Some(ReviewRequest { key: key.into(), review: review.clone(), source: source.clone() });
                            }
                            focus_feedback(ui, &response);
                            let file = std::path::Path::new(&source.file).file_name().unwrap_or_default().to_string_lossy();
                            ui.add(egui::Label::new(theme::muted(format!("{} · {}:{}", file, source.line, source.column))).wrap())
                                .on_hover_text(format!("{}\n字节 {}..{}\n{}", source.file, source.byte_start, source.byte_end, source.excerpt));
                        });
                    }
                });
            });
        });
        for (index, child) in node.children.iter().enumerate() {
            draw_node(ui, child, &format!("{number}.{}", index + 1), depth + 1, review, key, typography, blocked, actions);
        }
        if let Some(end) = &node.end_label {
            ui.add(egui::Label::new(theme::muted(format!("└ {number} 结束 · {end}"))).wrap());
            ui.separator();
        }
        ui.add_space(if body { typography.size * 0.35 } else { 4.0 });
    });
}

fn focus_feedback(ui: &egui::Ui, response: &egui::Response) {
    if response.has_focus() {
        ui.painter().rect_stroke(response.rect.expand(2.0), 2.0, egui::Stroke::new(2.0_f32, theme::ACCENT()), egui::StrokeKind::Outside);
        if response.gained_focus() { response.scroll_to_me(Some(egui::Align::Center)); }
    }
}

/// 所有 inline parts 在同一 LayoutJob 内；分段链接不会增加空格、换行或行高。
pub(super) fn paragraph_job(parts: &[ReviewPart], typography: Typography, width: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = width;
    for part in parts {
        job.append(&part.text, 0.0, egui::TextFormat {
            font_id: egui::FontId::proportional(typography.size),
            color: if part.target.is_some() { theme::ACCENT() } else { theme::TEXT() },
            line_height: Some(typography.size * typography.spacing),
            underline: if part.target.is_some() { egui::Stroke::new(1.0_f32, theme::ACCENT()) } else { egui::Stroke::NONE },
            italics: part.dynamic,
            ..Default::default()
        });
    }
    job
}

fn paragraph(ui: &mut egui::Ui, parts: &[ReviewPart], typography: Typography, actions: &mut Actions) {
    let job = paragraph_job(parts, typography, ui.available_width());
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    let response = ui.add(egui::Label::new(galley.clone()).wrap().sense(egui::Sense::click()));
    if let Some(position) = response.hover_pos() {
        let character = galley.cursor_from_pos(position - response.rect.min).index;
        let mut start = 0;
        for part in parts {
            let end = start + part.text.chars().count();
            if (start..end).contains(&character) {
                if let Some(target) = &part.target {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    if response.clicked() { actions.reference = Some(target.clone()); }
                }
                break;
            }
            start = end;
        }
    }
    let mut seen = Vec::new();
    for part in parts {
        if let Some(target) = &part.target {
            if seen.contains(target) { continue; }
            seen.push(target.clone());
            let label = format!("旁查 {} · {}:{}", part.text, target.kind, target.id);
            let response = ui.add(egui::Button::new(egui::RichText::new(label).small()).wrap());
            if response.clicked() { actions.reference = Some(target.clone()); }
            focus_feedback(ui, &response);
        }
    }
}
