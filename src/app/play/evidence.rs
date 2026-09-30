//! Actual condition evidence, separated from advanced replay transport controls.
use egui::{Color32, RichText};
use worldline_runtime::{ChoiceExplanation, EvidenceOutcome, Value};

pub(super) fn render(
    ui: &mut egui::Ui,
    choices: &[ChoiceExplanation],
    run_version: u64,
    edit_version: u64,
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
    for choice in choices {
        let status = if choice.condition.as_ref().is_some_and(|c| c.error.is_some())
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
                    ui.colored_label(Color32::LIGHT_RED, reason);
                }
                ui.label(crate::theme::muted(format!(
                    "{} · 第 {} 行",
                    choice.choice.node, choice.choice.line
                )));
                let Some(condition) = &choice.condition else {
                    ui.label("无条件限制");
                    return;
                };
                if let Some(evidence) = &condition.evidence {
                    ui.add(
                        egui::Label::new(RichText::new(&evidence.display_expression).monospace())
                            .wrap(),
                    );
                    let mut depths = Vec::with_capacity(evidence.nodes.len());
                    for node in &evidence.nodes {
                        let depth = node.parent.and_then(|p| depths.get(p)).map_or(0, |d| d + 1);
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
                                (format!("求值错误 · {message}"), Color32::LIGHT_RED)
                            }
                            EvidenceOutcome::NotEvaluated => (
                                "未求值 · 前序求值已出错".into(),
                                ui.visuals().weak_text_color(),
                            ),
                            EvidenceOutcome::Omitted => (
                                "证据已省略 · 不影响真实求值".into(),
                                ui.visuals().weak_text_color(),
                            ),
                        };
                        ui.horizontal_top(|ui| {
                            ui.add_space((depth as f32 * 10.0).min(80.0));
                            ui.add(
                                egui::Label::new(
                                    RichText::new(format!("{} → {value}", node.label)).color(color),
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
                    ui.colored_label(Color32::LIGHT_RED, error);
                }
            });
    }
}
