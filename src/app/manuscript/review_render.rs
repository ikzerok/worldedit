//! 审稿只排版 core 的结构树；正文连续，结构边界采用紧凑次级层。
use super::review_navigation::ReviewRequest;
use crate::{app::writing_workspace::Typography, theme};
use std::sync::Arc;
use worldline_core::{
    manuscript::{ReviewKind, ReviewNode, ReviewPart, ReviewProjection, ReviewSource},
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
    let mut renderer = Renderer {
        review,
        key,
        typography,
        blocked,
        actions,
    };
    for (index, node) in review.nodes.iter().enumerate() {
        renderer.node(ui, node, &format!("{}", index + 1), 0);
    }
}

struct Renderer<'a> {
    review: &'a Arc<ReviewProjection>,
    key: &'a str,
    typography: Typography,
    blocked: Option<&'a str>,
    actions: &'a mut Actions,
}

impl Renderer<'_> {
    fn node(&mut self, ui: &mut egui::Ui, node: &ReviewNode, number: &str, depth: usize) {
        ui.push_id(number, |ui| {
            match node.kind {
                ReviewKind::If => self.condition_group(ui, node, number, depth),
                ReviewKind::ChoiceGroup | ReviewKind::Scene => {
                    self.group_frame(depth).show(ui, |ui| {
                        let title = if node.kind == ReviewKind::ChoiceGroup {
                            "选择组 · 各选择分别审阅"
                        } else {
                            &node.label
                        };
                        self.heading(ui, title, node.source.as_ref(), number, depth);
                        self.children(ui, node, number, depth);
                        self.end(ui, node, number);
                    });
                }
                ReviewKind::Branch => self.branch(ui, node, number, depth),
                ReviewKind::Choice => {
                    self.heading(ui, "选择", node.source.as_ref(), number, depth);
                    paragraph(ui, &node.parts, self.typography, self.blocked, self.actions);
                    if let Some(condition) = &node.condition {
                        ui.add(
                            egui::Label::new(theme::muted(format!(
                                "可见条件（未求值）：{condition}"
                            )))
                            .wrap(),
                        );
                    }
                    if let Some(enable) = &node.enable {
                        ui.add(
                            egui::Label::new(theme::muted(format!("可选条件（未求值）：{enable}")))
                                .wrap(),
                        );
                    }
                    if node.once {
                        ui.label(theme::muted("一次性选择 · 未判定是否已选"));
                    }
                    if let Some(reason) = &node.disabled_reason {
                        ui.add(
                            egui::Label::new(theme::muted(format!("不可选说明：{reason}"))).wrap(),
                        );
                    }
                    self.children(ui, node, number, depth);
                }
                ReviewKind::Text | ReviewKind::Say | ReviewKind::Description => self.body(ui, node),
                _ => {
                    // 声明、去向、call/return 是次级说明，不为每一行再造一张大卡。
                    let label = match node.kind {
                        ReviewKind::Call => format!("片段调用 · 未展开：{}", node.label),
                        ReviewKind::Structure => format!("结构 · {}", node.label),
                        ReviewKind::Divert => format!("流程去向 · {}", node.label),
                        _ => node.label.clone(),
                    };
                    self.heading(ui, &label, node.source.as_ref(), number, depth);
                    self.children(ui, node, number, depth);
                    self.end(ui, node, number);
                }
            }
            ui.add_space(3.0);
        });
    }

    fn condition_group(
        &mut self,
        ui: &mut egui::Ui,
        node: &ReviewNode,
        number: &str,
        depth: usize,
    ) {
        self.group_frame(depth).show(ui, |ui| {
            // 合并同一 If 的单 Branch 视觉容器，不合并相邻 If，也不改变 DTO。
            if node.children.len() != 1 {
                self.heading(ui, "条件组 · 按次序择一", None, number, depth);
            }
            // If 和首 Branch 通常是同一真实条件头；同一范围只提供一个按钮。
            if let Some(source) = node.source.as_ref().filter(|source| {
                !node
                    .children
                    .iter()
                    .any(|branch| branch.source.as_ref() == Some(source))
            }) {
                self.source_button(ui, source);
            }
            for (index, branch) in node.children.iter().enumerate() {
                if index > 0 {
                    ui.separator();
                }
                let path = format!("{number}.{}", index + 1);
                ui.push_id(&path, |ui| self.branch(ui, branch, &path, depth + 1));
            }
            self.end(ui, node, number);
        });
    }

    fn branch(&mut self, ui: &mut egui::Ui, node: &ReviewNode, number: &str, depth: usize) {
        let title = node.condition.as_ref().map_or_else(
            || node.label.clone(),
            |condition| format!("{} · {condition}", node.label),
        );
        self.heading(ui, &title, node.source.as_ref(), number, depth);
        self.children(ui, node, number, depth);
    }

    fn children(&mut self, ui: &mut egui::Ui, node: &ReviewNode, number: &str, depth: usize) {
        for (index, child) in node.children.iter().enumerate() {
            if index > 0 && node.kind == ReviewKind::ChoiceGroup {
                ui.separator();
            }
            self.node(ui, child, &format!("{number}.{}", index + 1), depth + 1);
        }
    }

    fn group_frame(&self, depth: usize) -> egui::Frame {
        egui::Frame::new()
            .stroke(egui::Stroke::new(1.0_f32, theme::MUTED()))
            .inner_margin(if depth < 3 { 6 } else { 1 })
    }

    fn heading(
        &mut self,
        ui: &mut egui::Ui,
        title: &str,
        source: Option<&ReviewSource>,
        number: &str,
        depth: usize,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.add(egui::Label::new(theme::muted(title).strong()).wrap())
                .on_hover_text(format!(
                    "结构边界 {number} · 第{}层 · 静态审稿，未执行",
                    depth + 1
                ));
            if let Some(source) = source {
                self.source_button(ui, source);
            }
        });
    }

    fn end(&self, ui: &mut egui::Ui, node: &ReviewNode, number: &str) {
        if let Some(end) = &node.end_label {
            ui.add(egui::Label::new(theme::muted(format!("└ {end}"))).wrap())
                .on_hover_text(format!("结构边界 {number} 结束"));
        }
    }

    fn body(&mut self, ui: &mut egui::Ui, node: &ReviewNode) {
        if let Some(speaker) = &node.speaker {
            ui.horizontal_wrapped(|ui| {
                let response = ui.add_enabled(
                    self.blocked.is_none(),
                    egui::Button::new(egui::RichText::new(&speaker.display).strong()).frame(false),
                );
                let response = response.on_hover_text(format!(
                    "人物 {}:{} · 旁查资料",
                    speaker.target.kind, speaker.target.id
                ));
                if response.clicked() {
                    self.actions.reference = Some(speaker.target.clone());
                }
                focus_feedback(ui, &response);
                ui.menu_button("身份", |ui| {
                    ui.label(format!(
                        "人物 {}:{}",
                        speaker.target.kind, speaker.target.id
                    ));
                });
            });
        }
        if !node.label.is_empty() {
            let label = if node.kind == ReviewKind::Say {
                format!("演出说明：{}", node.label)
            } else {
                node.label.clone()
            };
            ui.add(egui::Label::new(theme::muted(label)).wrap());
        }
        paragraph(ui, &node.parts, self.typography, self.blocked, self.actions);
        ui.horizontal_wrapped(|ui| {
            if let Some(source) = &node.source {
                self.source_button(ui, source);
            }
            if node.glue {
                ui.label(theme::muted("粘接标记 · 不跨分支合并"));
            }
        });
        ui.add_space(self.typography.size * 0.2);
    }

    fn source_button(&mut self, ui: &mut egui::Ui, source: &ReviewSource) {
        let detail = format!(
            "{}:{}:{} · 字节 {}..{}\n{}\n{}",
            source.file,
            source.line,
            source.column,
            source.byte_start,
            source.byte_end,
            source.excerpt,
            self.blocked
                .unwrap_or("定位同一文件草稿的真实范围；不应用或保存。Alt+Left 返回")
        );
        let response = ui
            .add_enabled(
                self.blocked.is_none(),
                egui::Button::new("定位原文").small(),
            )
            .on_hover_text(detail);
        if response.clicked() {
            self.actions.source = Some(ReviewRequest {
                key: self.key.into(),
                review: self.review.clone(),
                source: source.clone(),
            });
        }
        focus_feedback(ui, &response);
    }
}

fn focus_feedback(ui: &egui::Ui, response: &egui::Response) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.expand(2.0),
            2.0,
            egui::Stroke::new(2.0_f32, theme::ACCENT()),
            egui::StrokeKind::Outside,
        );
        if response.gained_focus() {
            response.scroll_to_me(Some(egui::Align::Center));
        }
    }
}

/// 所有 inline parts 在同一 LayoutJob 内；分段链接不会增加空格、换行或行高。
pub(super) fn paragraph_job(
    parts: &[ReviewPart],
    typography: Typography,
    width: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = width;
    for part in parts {
        job.append(
            &part.text,
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(typography.size),
                color: if part.target.is_some() {
                    theme::ACCENT()
                } else {
                    theme::TEXT()
                },
                line_height: Some(typography.size * typography.spacing),
                underline: if part.target.is_some() {
                    egui::Stroke::new(1.0_f32, theme::ACCENT())
                } else {
                    egui::Stroke::NONE
                },
                italics: part.dynamic,
                ..Default::default()
            },
        );
    }
    job
}

fn paragraph(
    ui: &mut egui::Ui,
    parts: &[ReviewPart],
    typography: Typography,
    blocked: Option<&str>,
    actions: &mut Actions,
) {
    if parts.is_empty() {
        return;
    }
    let job = paragraph_job(parts, typography, ui.available_width());
    let galley = ui.fonts(|fonts| fonts.layout_job(job));
    let response = ui.add(
        egui::Label::new(galley.clone())
            .wrap()
            .sense(egui::Sense::click()),
    );
    if let Some(position) = response.hover_pos().filter(|_| blocked.is_none()) {
        let character = galley.cursor_from_pos(position - response.rect.min).index;
        let mut start = 0;
        for part in parts {
            let end = start + part.text.chars().count();
            if (start..end).contains(&character) {
                if let Some(target) = &part.target {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    if response.clicked() {
                        actions.reference = Some(target.clone());
                    }
                }
                break;
            }
            start = end;
        }
    }
    let mut seen = Vec::new();
    for part in parts {
        if let Some(target) = &part.target {
            if seen.contains(target) {
                continue;
            }
            seen.push(target.clone());
            let response = ui
                .add_enabled(
                    blocked.is_none(),
                    egui::Button::new(egui::RichText::new(format!("旁查 {}", part.text)).small())
                        .wrap(),
                )
                .on_hover_text(format!("{}:{}", target.kind, target.id));
            if response.clicked() {
                actions.reference = Some(target.clone());
            }
            focus_feedback(ui, &response);
        }
    }
}
