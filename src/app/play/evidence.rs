//! Actual condition evidence, separated from advanced replay transport controls.
use super::evidence_navigation::{EvidenceNavigationAccess, EvidenceNavigationRequest};
use egui::RichText;
use worldline_core::evidence_source::{EvidenceSource, EvidenceSourceOwner};
use worldline_runtime::{ChoiceExplanation, EvidenceOutcome, Value};

pub(super) fn render(
    ui: &mut egui::Ui,
    choices: &[ChoiceExplanation],
    run_version: u64,
    edit_version: u64,
    access: &EvidenceNavigationAccess,
    jump: &mut Option<EvidenceNavigationRequest>,
) {
    ui.label(RichText::new(format!("条件证据 · 运行版本 #{run_version}")).strong());
    if run_version != edit_version {
        ui.label(crate::theme::muted(
            "此证据来自旧运行；重新开始后使用最新稿件。",
        ));
    }
    ui.label(crate::theme::muted(
        "来自这一次真实求值；展开查看各项的值。",
    ));
    if choices.is_empty() {
        ui.label("当前尚无已执行的选择组证据。");
    }
    if let Some(reason) = &access.reason {
        ui.label(crate::theme::muted(format!("作者定位不可用：{reason}")));
    }
    for choice in choices {
        let status = if choice.condition.as_ref().is_some_and(|c| c.error.is_some())
            || choice
                .enable_condition
                .as_ref()
                .is_some_and(|c| c.error.is_some())
            || choice
                .unavailable_reason
                .as_deref()
                .is_some_and(|reason| reason.starts_with("选择标签求值失败"))
        {
            "求值错误"
        } else if choice
            .unavailable_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("once"))
        {
            "once 已使用"
        } else if choice.available {
            "已满足"
        } else {
            "未满足"
        };
        egui::CollapsingHeader::new(format!("{} · {status}", choice.choice.label))
            .id_salt(("choice-evidence", &choice.choice.id))
            .default_open(!choice.available)
            .show(ui, |ui| {
                if let Some(reason) = choice
                    .unavailable_reason
                    .as_deref()
                    .filter(|reason| reason.starts_with("选择标签求值失败"))
                {
                    ui.colored_label(crate::theme::ERROR(), reason);
                }
                ui.label(crate::theme::muted(format!(
                    "{} · 第 {} 行",
                    choice.choice.node, choice.choice.line
                )));
                source_button(
                    ui,
                    choice.source.as_ref(),
                    "定位选择声明",
                    choices,
                    access,
                    jump,
                );
                if let Some(reason) = choice
                    .unavailable_reason
                    .as_deref()
                    .filter(|reason| !reason.starts_with("选择标签求值失败"))
                {
                    ui.label(reason);
                }
                if choice.condition.is_none() && choice.enable_condition.is_none() {
                    ui.label("无表达式条件");
                    return;
                }
                for (caption, condition) in [
                    ("显示条件", &choice.condition),
                    ("可选条件", &choice.enable_condition),
                ] {
                    let Some(condition) = condition else {
                        continue;
                    };
                    ui.label(egui::RichText::new(caption).strong());
                    if let Some(evidence) = &condition.evidence {
                        ui.add(
                            egui::Label::new(
                                RichText::new(&evidence.display_expression).monospace(),
                            )
                            .wrap(),
                        );
                        let mut depths = Vec::with_capacity(evidence.nodes.len());
                        let mut shown_sources = Vec::new();
                        for node in &evidence.nodes {
                            if let Some(source) = &node.source {
                                if !shown_sources.contains(source) {
                                    shown_sources.push(source.clone());
                                    if let EvidenceSourceOwner::Rule { name } = &source.owner {
                                        ui.label(crate::theme::muted(format!(
                                            "rule {name} · 第 {} 行",
                                            source.line
                                        )));
                                    }
                                    source_button(
                                        ui,
                                        Some(source),
                                        "定位规则定义",
                                        choices,
                                        access,
                                        jump,
                                    );
                                }
                            }
                            let depth =
                                node.parent.and_then(|p| depths.get(p)).map_or(0, |d| d + 1);
                            depths.push(depth);
                            let (value, color) = match &node.outcome {
                                EvidenceOutcome::Evaluated {
                                    value: Value::Bool(value),
                                } => (
                                    if *value {
                                        "已满足 · true"
                                    } else {
                                        "未满足 · false"
                                    }
                                    .into(),
                                    ui.visuals().text_color(),
                                ),
                                EvidenceOutcome::Evaluated { value } => (
                                    format!("{} · {}", value.kind_label(), value.display()),
                                    ui.visuals().text_color(),
                                ),
                                EvidenceOutcome::Error { message } => {
                                    (format!("求值错误 · {message}"), crate::theme::ERROR())
                                }
                                EvidenceOutcome::NotEvaluated => {
                                    ("未求值 · 前序求值已出错".into(), crate::theme::MUTED())
                                }
                                EvidenceOutcome::Omitted => {
                                    ("证据已省略 · 不影响真实求值".into(), crate::theme::MUTED())
                                }
                            };
                            ui.horizontal_top(|ui| {
                                ui.add_space((depth as f32 * 10.0).min(80.0));
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(format!("{} → {value}", node.label))
                                            .color(color),
                                    )
                                    .wrap(),
                                );
                            });
                        }
                        if evidence.omitted {
                            ui.label(crate::theme::muted(
                                "部分证据已省略（记录边界）；真实求值仍按原顺序完成或返回原错误。",
                            ));
                        }
                    } else {
                        ui.label("没有本次实际求值证据");
                    }
                    if let Some(error) = &condition.error {
                        ui.colored_label(crate::theme::ERROR(), error);
                    }
                }
            });
    }
}

fn source_button(
    ui: &mut egui::Ui,
    source: Option<&EvidenceSource>,
    caption: &str,
    choices: &[ChoiceExplanation],
    access: &EvidenceNavigationAccess,
    jump: &mut Option<EvidenceNavigationRequest>,
) {
    let reason = access.source_reason(source);
    let response =
        crate::theme::add_enabled(ui, reason.is_none(), egui::Button::new(caption).small());
    if response.clicked() {
        if let Some(source) = source {
            *jump = Some(EvidenceNavigationRequest {
                source: source.clone(),
                cached: choices.to_vec(),
            });
        }
    }
    if let Some(reason) = reason {
        response.on_disabled_hover_text(reason);
        if access.reason.is_none() {
            ui.label(crate::theme::muted(reason));
        }
    } else {
        response.on_hover_text("定位真实声明头；不会重新求值或应用草稿，可用 Alt+Left 返回");
    }
}
