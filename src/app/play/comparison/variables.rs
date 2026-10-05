//! 变量终值与实际全局写入。只消费 runtime 证据，不推断未执行分支或静态来源。
use super::{
    navigation::{source_button, variable_source, ComparisonSourceRequest, NavigationAccess},
    ComparedRoutes, ComparisonState,
};
use crate::theme;
use std::collections::BTreeSet;
use worldline_core::evidence_source::VariableWriteOperation;
use worldline_runtime::{RouteSideResult, Value, VariableWriteRecord};

mod values;

fn variables(compared: &ComparedRoutes) -> BTreeSet<&str> {
    [&compared.result.left, &compared.result.right]
        .into_iter()
        .flat_map(|side| {
            side.vars
                .iter()
                .flat_map(|vars| vars.keys().map(String::as_str))
                .chain(
                    side.variable_writes
                        .records
                        .iter()
                        .filter(|_| side.variable_writes.captured)
                        .map(|record| record.variable.as_str()),
                )
        })
        .collect()
}

fn operation(operation: VariableWriteOperation) -> &'static str {
    match operation {
        VariableWriteOperation::Let => "let · 声明写入",
        VariableWriteOperation::Const => "const · 常量初始化",
        VariableWriteOperation::Set => "set · 赋值",
    }
}

fn evidence_summary(side: &RouteSideResult) -> String {
    let evidence = &side.variable_writes;
    if !evidence.captured {
        return "旧结果未提供变量写入证据；验证尚未建立时也不会提供，不能据此判断是否写入".into();
    }
    if evidence.omitted {
        return format!(
            "写入证据已省略：实际 {} 项，保留 {} 项；与状态动作共享上限，遗漏不表示未发生",
            evidence.total_writes,
            evidence.records.len()
        );
    }
    if !side.complete {
        return format!(
            "本段验证不完整：已记录实际写入 {} 项；仅解释停止前的已知执行",
            evidence.total_writes
        );
    }
    format!("本段完整验证，实际变量写入 {} 项", evidence.total_writes)
}

fn empty_selection(side: &RouteSideResult, variable: &str) -> String {
    let evidence = &side.variable_writes;
    if !evidence.captured {
        "此侧未提供写入记录，不能认定没有发生写入".into()
    } else if evidence.omitted {
        format!("未保留 {variable} 的写入证据；可能因额度省略")
    } else if !side.complete {
        format!("停止前未记录 {variable} 的写入；后续执行尚未验证")
    } else {
        format!("此侧本段没有对 {variable} 的全局写入")
    }
}

fn terminal_card(
    ui: &mut egui::Ui,
    compared: &ComparedRoutes,
    state: &mut ComparisonState,
    variable: &str,
    access: &NavigationAccess<'_>,
) {
    ui.push_id(("variable-value", compared.id, variable), |ui| {
        egui::Frame::group(ui.style())
            .fill(theme::CARD())
            .show(ui, |ui| {
                let id = egui::Id::new((
                    "comparison-variable",
                    compared.scope.workspace_root(),
                    compared.id,
                    variable,
                ));
                let response = super::focus::widget(ui, id, |ui| {
                    ui.add(
                        egui::Button::selectable(
                            state.selected_variable.as_deref() == Some(variable),
                            format!("变量 · {variable}"),
                        )
                        .wrap(),
                    )
                });
                access.focus.reveal(ui, &response);
                if response.clicked() {
                    state.selected_variable = Some(variable.to_owned());
                    state.selected_write = None;
                }
                for display_right in [false, true] {
                    let letter = if display_right { "B" } else { "A" };
                    let side = compared.side(display_right);
                    ui.push_id(letter, |ui| {
                        ui.label(theme::muted(format!("{letter} · 实际停止值")));
                        match side.vars.as_ref() {
                            Some(vars) => {
                                values::render(ui, "终值", vars.get(variable), access.focus)
                            }
                            None => {
                                ui.label("未取得此侧实际变量值");
                            }
                        }
                    });
                }
                if let Some((left, right)) = compared
                    .result
                    .left
                    .vars
                    .as_ref()
                    .zip(compared.result.right.vars.as_ref())
                {
                    if let Some((a, b)) = left.get(variable).zip(right.get(variable)) {
                        ui.label(theme::muted(if a == b {
                            "两侧终值相同；实际写入仍可能不同"
                        } else {
                            "两侧实际停止值不同"
                        }));
                    }
                }
            });
    });
}

fn write_card(
    ui: &mut egui::Ui,
    compared: &ComparedRoutes,
    state: &mut ComparisonState,
    actual_right: bool,
    record: &VariableWriteRecord,
    access: &NavigationAccess<'_>,
    request: &mut Option<ComparisonSourceRequest>,
) {
    ui.push_id(
        ("variable-write", compared.id, actual_right, record.sequence),
        |ui| {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                let selected = state.selected_write == Some((actual_right, record.sequence));
                let id = egui::Id::new((
                    "comparison-write",
                    compared.scope.workspace_root(),
                    compared.id,
                    actual_right,
                    record.sequence,
                ));
                let response = super::focus::widget(ui, id, |ui| {
                    ui.add(
                        egui::Button::selectable(
                            selected,
                            format!(
                                "写入 #{} · {} · {}",
                                record.sequence,
                                operation(record.operation),
                                record.variable
                            ),
                        )
                        .wrap(),
                    )
                });
                access.focus.reveal(ui, &response);
                if response.clicked() {
                    state.selected_write = Some((actual_right, record.sequence));
                }
                values::render(ui, "写入前", record.before.as_ref(), access.focus);
                ui.label(theme::muted("↓ 实际写入"));
                values::render(ui, "写入后", Some(&record.after), access.focus);
                if record.before.as_ref() == Some(&record.after) {
                    ui.label("同值写入：值未改变，仍实际执行了这次写入");
                }
                ui.add(
                    egui::Label::new(format!(
                        "事件 {} · 节点 {} · 回合 {}",
                        record.event.as_deref().unwrap_or("未提供"),
                        record.node.as_deref().unwrap_or("未提供"),
                        record.turn
                    ))
                    .wrap(),
                );
                // 独占一行，长值或狭窄视口不能把来源按钮推出横向可达区域。
                if source_button(
                    ui,
                    compared,
                    egui::Id::new(("write", actual_right, record.sequence)),
                    variable_source(record),
                    access,
                    "打开实际写入来源",
                    request,
                ) {
                    state.selected_variable = Some(record.variable.clone());
                    state.selected_write = Some((actual_right, record.sequence));
                }
            });
        },
    );
}

pub(super) fn render(
    ui: &mut egui::Ui,
    compared: &ComparedRoutes,
    state: &mut ComparisonState,
    access: &NavigationAccess<'_>,
    request: &mut Option<ComparisonSourceRequest>,
) {
    ui.add_space(theme::SPACE_MD);
    ui.heading("变量终值与实际写入");
    ui.label(theme::muted(
        "选择变量查看两侧本段的全局 let / const / set；起点初始化和检查点旧历史不回填",
    ));
    let ids = variables(compared);
    if ids.is_empty() {
        ui.label("没有可展示的变量终值或保留写入；请同时阅读每侧证据状态");
    }
    for variable in ids {
        terminal_card(ui, compared, state, variable, access);
    }
    ui.add_space(theme::SPACE_SM);
    ui.strong(state.selected_variable.as_ref().map_or_else(
        || "实际变量写入 · 尚未选择变量".into(),
        |variable| format!("实际变量写入 · {variable}"),
    ));
    for display_right in [false, true] {
        let side = compared.side(display_right);
        let actual_right = compared.display_index(display_right) == 1;
        ui.add_space(theme::SPACE_SM);
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!(
                    "{} · {}",
                    if display_right { "B" } else { "A" },
                    compared.name(display_right)
                ))
                .strong(),
            )
            .wrap(),
        );
        ui.label(super::view::status_text(side));
        ui.add(egui::Label::new(evidence_summary(side)).wrap());
        if let Some(detail) = &side.detail {
            ui.add(egui::Label::new(theme::muted(detail)).wrap());
        }
        let mut found = false;
        if side.variable_writes.captured {
            let selected = state.selected_variable.clone();
            for record in side
                .variable_writes
                .records
                .iter()
                .filter(|record| selected.as_deref() == Some(record.variable.as_str()))
            {
                found = true;
                write_card(ui, compared, state, actual_right, record, access, request);
            }
        }
        if !found {
            if let Some(variable) = state.selected_variable.as_deref() {
                ui.add(egui::Label::new(theme::muted(empty_selection(side, variable))).wrap());
            }
        }
    }
}

#[cfg(test)]
#[path = "variable_display_tests.rs"]
mod tests;
