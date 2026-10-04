//! 结果值、当次动作和覆盖分层展示，不把静态声明当成执行因果。
use super::{
    navigation::{source_button, ComparisonSourceRequest, NavigationAccess},
    ComparedRoutes, ComparisonState,
};
use crate::theme;
use worldline_core::{ast::ChangeKind, catalog::Catalog, TargetRef};
use worldline_runtime::RouteValueDifference;

fn tags_text(tags: &[String], catalog: Option<&Catalog>) -> String {
    if tags.is_empty() {
        return "空标签集".into();
    }
    tags.iter()
        .map(|id| {
            catalog
                .and_then(|catalog| catalog.tags.get(id))
                .map(|tag| tag.display.as_str())
                .filter(|display| !display.is_empty())
                .unwrap_or(id)
        })
        .collect::<Vec<_>>()
        .join(" · ")
}
fn tag_ids(tags: &[String]) -> String {
    if tags.is_empty() {
        "空标签集".into()
    } else {
        tags.join(", ")
    }
}
fn value(
    value: Option<&serde_json::Value>,
    state: bool,
    catalog: Option<&Catalog>,
) -> (String, Option<String>) {
    let Some(value) = value else {
        return ("∅ 此侧没有此值".into(), None);
    };
    if state {
        return match serde_json::from_value::<Vec<String>>(value.clone()) {
            Ok(tags) => (
                tags_text(&tags, catalog),
                catalog.map(|_| format!("标签 ID：{}", tag_ids(&tags))),
            ),
            Err(_) => ("无法按当前状态格式展示".into(), Some(value.to_string())),
        };
    }
    match serde_json::from_value::<worldline_runtime::Value>(value.clone()) {
        Ok(value) => (value.display(), Some(value.kind_label().into())),
        Err(_) => ("无法按当前变量格式展示".into(), Some(value.to_string())),
    }
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
fn state_caption(catalog: Option<&Catalog>, id: &str) -> String {
    let Some(state) = catalog.and_then(|catalog| catalog.states.get(id)) else {
        return format!("状态 · {id}");
    };
    let owner = catalog.and_then(|catalog| catalog.object(&state.target));
    owner.map_or_else(
        || state.display.clone(),
        |owner| format!("{} · {}", state.display, owner.display),
    )
}

fn difference(
    ui: &mut egui::Ui,
    compared: &ComparedRoutes,
    difference: &RouteValueDifference,
    select: bool,
    selected: &mut Option<String>,
    catalog: Option<&Catalog>,
    focus: super::focus::FocusReveal,
) {
    ui.push_id(("result-value", select, &difference.id), |ui| {
        egui::Frame::group(ui.style())
            .fill(theme::CARD())
            .show(ui, |ui| {
                if select {
                    let response = ui.selectable_label(
                        selected.as_ref() == Some(&difference.id),
                        state_caption(catalog, &difference.id),
                    );
                    focus.reveal(ui, &response);
                    if response.clicked() {
                        *selected = Some(difference.id.clone());
                    }
                    let identity = catalog
                        .and_then(|catalog| catalog.states.get(&difference.id))
                        .map_or_else(
                            || format!("state {}", difference.id),
                            |state| {
                                format!(
                                    "state {} → {} {}",
                                    difference.id, state.target.kind, state.target.id
                                )
                            },
                        );
                    ui.label(theme::muted(identity));
                } else {
                    ui.strong(format!("变量 · {}", difference.id));
                }
                ui.columns(2, |columns| {
                    let values = [&difference.left, &difference.right];
                    for (index, ui) in columns.iter_mut().enumerate() {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(if index == 0 { "A" } else { "B" });
                            let (text, hint) = value(
                                values[compared.display_index(index == 1)].as_ref(),
                                select,
                                catalog,
                            );
                            value_label(ui, text);
                            if let Some(hint) = hint {
                                ui.label(theme::muted(hint));
                            }
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
    access: &NavigationAccess<'_>,
    request: &mut Option<ComparisonSourceRequest>,
    target: &mut Option<TargetRef>,
    catalog: Option<&Catalog>,
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
            catalog,
            access.focus,
        );
    }
    for difference_value in &compared.result.variable_differences {
        difference(
            ui,
            compared,
            difference_value,
            false,
            &mut state.selected_state,
            catalog,
            access.focus,
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
    let state_picker = egui::ComboBox::from_id_salt("comparison-state-actions")
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
    access.focus.reveal(ui, &state_picker.response);
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
                    let response = ui.selectable_label(
                        selected,
                        format!(
                            "动作 #{} · {} · {}",
                            record.sequence,
                            kind(&record.kind),
                            record.state
                        ),
                    );
                    access.focus.reveal(ui, &response);
                    if response.clicked() {
                        state.selected_action = Some((actual_right, record.sequence));
                    }
                    value_label(
                        ui,
                        format!(
                            "{} → {}",
                            tags_text(&record.before, catalog),
                            tags_text(&record.after, catalog)
                        ),
                    );
                    if catalog.is_some() {
                        ui.label(theme::muted(format!(
                            "标签 ID：{} → {}",
                            tag_ids(&record.before),
                            tag_ids(&record.after)
                        )));
                    }
                    ui.label(theme::muted(format!(
                        "{} · 回合 {}",
                        record.node.as_deref().unwrap_or("无节点"),
                        record.turn
                    )));
                    if let Some(note) = &record.note {
                        value_label(ui, note.clone());
                    }
                    if let Some(owner) = &record.target {
                        let display = catalog
                            .and_then(|catalog| catalog.object(owner))
                            .map(|object| object.display.as_str())
                            .unwrap_or(&owner.id);
                        let response = ui.button(format!("查看所属对象 · {display}"));
                        access.focus.reveal(ui, &response);
                        if response.clicked() {
                            *target = Some(owner.clone());
                        }
                        ui.label(theme::muted(format!("{} {}", owner.kind, owner.id)));
                    } else {
                        ui.label(theme::muted("没有可确认的所属世界对象"));
                    }
                    if source_button(
                        ui,
                        compared,
                        record.source.as_ref(),
                        access,
                        "打开实际动作来源",
                        request,
                    ) {
                        state.selected_action = Some((actual_right, record.sequence));
                    }
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

#[cfg(test)]
#[path = "display_tests.rs"]
mod tests;
