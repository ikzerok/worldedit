//! 结果值、当次动作和覆盖分层展示，不把静态声明当成执行因果。
use super::{
    navigation::{source_button, ComparisonSourceRequest},
    ComparedRoutes, ComparisonState,
};
use crate::theme;
use worldline_core::{ast::ChangeKind, TargetRef};
use worldline_runtime::RouteValueDifference;

fn value(value: Option<&serde_json::Value>) -> String {
    value.map_or_else(
        || "∅ 此侧没有此值".into(),
        |value| match value {
            serde_json::Value::String(text) => text.clone(),
            _ => value.to_string(),
        },
    )
}
fn value_label(ui: &mut egui::Ui, text: String) {
    let short: String = text.chars().take(200).collect();
    let truncated = short.len() < text.len();
    ui.add(
        egui::Label::new(if truncated {
            format!("{short}…")
        } else {
            short
        })
        .wrap(),
    )
    .on_hover_text(text);
}
fn difference(
    ui: &mut egui::Ui,
    compared: &ComparedRoutes,
    difference: &RouteValueDifference,
    select: bool,
    selected: &mut Option<String>,
) {
    ui.push_id(("result-value", select, &difference.id), |ui| {
        egui::Frame::group(ui.style())
            .fill(theme::CARD())
            .show(ui, |ui| {
                if select {
                    if ui
                        .selectable_label(
                            selected.as_ref() == Some(&difference.id),
                            format!("状态 · {}", difference.id),
                        )
                        .clicked()
                    {
                        *selected = Some(difference.id.clone());
                    }
                } else {
                    ui.strong(format!("变量 · {}", difference.id));
                }
                ui.columns(2, |columns| {
                    let values = [&difference.left, &difference.right];
                    for (index, ui) in columns.iter_mut().enumerate() {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(if index == 0 { "A" } else { "B" });
                            value_label(
                                ui,
                                value(values[compared.display_index(index == 1)].as_ref()),
                            );
                        });
                    }
                });
            });
    });
}
fn kind(kind: &ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Become => "替换标签",
        ChangeKind::AddTags => "增加标签",
        ChangeKind::RemoveTags => "移除标签",
        _ => kind.label(),
    }
}

pub(super) fn render(
    ui: &mut egui::Ui,
    compared: &ComparedRoutes,
    state: &mut ComparisonState,
    blocked: Option<&str>,
    request: &mut Option<ComparisonSourceRequest>,
    target: &mut Option<TargetRef>,
) {
    ui.heading("实际停止状态差异");
    if compared.result.state_differences.is_empty()
        && compared.result.variable_differences.is_empty()
    {
        ui.label(if compared.result.differences_complete {
            "当前实际结果值没有差异；这不表示两条路线等价"
        } else {
            "没有完整可比较的实际状态；不能据此认定没有差异"
        });
    }
    for difference_value in &compared.result.state_differences {
        difference(
            ui,
            compared,
            difference_value,
            true,
            &mut state.selected_state,
        );
    }
    for difference_value in &compared.result.variable_differences {
        difference(
            ui,
            compared,
            difference_value,
            false,
            &mut state.selected_state,
        );
    }
    ui.add_space(theme::SPACE_MD);
    ui.heading("实际状态动作");
    let ids: std::collections::BTreeSet<_> = [&compared.result.left, &compared.result.right]
        .into_iter()
        .flat_map(|side| {
            side.state_actions
                .records
                .iter()
                .map(|record| record.state.as_str())
        })
        .collect();
    if ids.is_empty() {
        ui.label("当前没有可展示的实际动作证据；不会回填检查点旧历史或静态候选来源");
    }
    egui::ComboBox::from_id_salt("comparison-state-actions")
        .selected_text(
            state
                .selected_state
                .as_deref()
                .unwrap_or("选择状态查看实际动作"),
        )
        .show_ui(ui, |ui| {
            for id in ids {
                ui.selectable_value(&mut state.selected_state, Some(id.to_owned()), id);
            }
        });
    for display_right in [false, true] {
        let side = compared.side(display_right);
        let actual_right = compared.display_index(display_right) == 1;
        ui.strong(format!(
            "{} · {}",
            if display_right { "B" } else { "A" },
            compared.name(display_right)
        ));
        if let Some(detail) = &side.detail {
            ui.colored_label(theme::WARNING(), detail);
        }
        if side.state_actions.omitted || side.omitted {
            ui.colored_label(
                theme::WARNING(),
                format!(
                    "动作证据达到边界：实际发生 {} 项，仅展示保留记录；遗漏不表示未发生",
                    side.state_actions.total_actions
                ),
            );
        }
        let mut found = false;
        for record in side
            .state_actions
            .records
            .iter()
            .filter(|record| state.selected_state.as_deref() == Some(record.state.as_str()))
        {
            found = true;
            ui.push_id(("actual-action", actual_right, record.sequence), |ui| {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    let selected = state.selected_action == Some((actual_right, record.sequence));
                    if ui
                        .selectable_label(
                            selected,
                            format!(
                                "动作 #{} · {} · {}",
                                record.sequence,
                                kind(&record.kind),
                                record.state
                            ),
                        )
                        .clicked()
                    {
                        state.selected_action = Some((actual_right, record.sequence));
                    }
                    value_label(
                        ui,
                        format!(
                            "{} → {}",
                            if record.before.is_empty() {
                                "空".into()
                            } else {
                                record.before.join(" · ")
                            },
                            if record.after.is_empty() {
                                "空".into()
                            } else {
                                record.after.join(" · ")
                            }
                        ),
                    );
                    ui.label(theme::muted(format!(
                        "{} · 回合 {}",
                        record.node.as_deref().unwrap_or("无节点"),
                        record.turn
                    )));
                    if let Some(note) = &record.note {
                        value_label(ui, note.clone());
                    }
                    if let Some(owner) = &record.target {
                        if ui
                            .button(format!("查看所属对象 · {} {}", owner.kind, owner.id))
                            .clicked()
                        {
                            *target = Some(owner.clone());
                        }
                    } else {
                        ui.label(theme::muted("没有可确认的所属世界对象"));
                    }
                    source_button(
                        ui,
                        compared,
                        record.source.as_ref(),
                        blocked,
                        "打开实际动作来源",
                        request,
                    );
                });
            });
        }
        if !found && state.selected_state.is_some() {
            ui.label(theme::muted("此侧没有保留该状态的本段动作证据"));
        }
    }
    ui.add_space(theme::SPACE_MD);
    ui.heading("访问与选择范围");
    ui.label("未测试不表示不可达；相同节点集合不表示选择相同，也不能证明所有路线已覆盖");
    for display_right in [false, true] {
        let side = compared.side(display_right);
        let letter = if display_right { "B" } else { "A" };
        ui.strong(format!(
            "{letter} 本段执行：{} 个节点，{} 类选择",
            side.coverage.executed.visited_nodes.len(),
            side.coverage.executed.selected_choices.len()
        ));
        ui.label(format!(
            "{letter} 检查点继承：{} 个节点，{} 类选择；不计入本段新执行",
            side.coverage.inherited.visited_nodes.len(),
            side.coverage.inherited.selected_choices.len()
        ));
        egui::CollapsingHeader::new(format!("查看 {letter} 实际执行与继承次数"))
            .id_salt(("comparison-coverage", letter))
            .show(ui, |ui| {
                for (name, coverage) in [
                    ("本段执行", &side.coverage.executed),
                    ("起点继承", &side.coverage.inherited),
                ] {
                    for (node, count) in &coverage.visited_nodes {
                        ui.label(format!("{name} · 节点 ×{count} · {node}"));
                    }
                    for choice in &coverage.selected_choices {
                        ui.label(format!(
                            "{name} · 选择 ×{} · {} · {}",
                            choice.count, choice.node, choice.label
                        ));
                    }
                }
            });
    }
}
