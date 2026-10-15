use super::*;

pub(super) fn show(
    ui: &mut egui::Ui,
    context: &Context<'_>,
    node: &mut ConditionNode,
    navigation: &mut Option<Navigation>,
) {
    let (count, depth) = size(node);
    let can_wrap = count + 2 <= MAX_CONDITION_NODES && depth < MAX_CONDITION_DEPTH;
    ui.label(theme::muted(format!(
        "{count}/{MAX_CONDITION_NODES} 节点 · {depth}/{MAX_CONDITION_DEPTH} 层"
    )));
    node_ui(ui, context, node, 0, can_wrap, navigation);
}
fn node_ui(
    ui: &mut egui::Ui,
    context: &Context<'_>,
    node: &mut ConditionNode,
    depth: usize,
    can_wrap: bool,
    navigation: &mut Option<Navigation>,
) {
    if depth >= MAX_CONDITION_DEPTH {
        ui.label("已达表单深度上限；请先减少分组");
        return;
    }
    let mut replacement = None;
    ui.group(|ui| {
        ui.horizontal_wrapped(|ui| {
            ui.strong(match node {
                ConditionNode::Bool { .. } => "布尔常量",
                ConditionNode::GlobalBool { .. } => "布尔变量",
                ConditionNode::Has { .. } => "状态包含标签",
                ConditionNode::Compare { .. } => "变量比较",
                ConditionNode::Not { .. } => "取反 NOT",
                ConditionNode::And { .. } => "同时满足 AND · 先左后右",
                ConditionNode::Or { .. } => "满足其一 OR · 先左后右",
            });
            theme::add_enabled_ui(ui, !context.composing, |ui| { ui.menu_button("调整子句", |ui| {
                for (label, leaf) in [
                    ("改为布尔常量", ConditionNode::Bool { value: true }),
                    ("改为布尔变量", ConditionNode::GlobalBool { variable: String::new() }),
                    ("改为状态包含标签", ConditionNode::Has { state_id: String::new(), tag_id: String::new() }),
                    ("改为变量与字面量比较", ConditionNode::Compare { variable: String::new(), op: ConditionComparison::Eq, value: ConditionLiteral::Num(0.0), literal_left: false }),
                ] {
                    if ui.button(label).clicked() { replacement = Some(leaf); ui.close(); }
                }
                ui.separator();
                if theme::add_enabled(ui, can_wrap, egui::Button::new("对此子句取反")).clicked() {
                    replacement = Some(ConditionNode::Not { child: Box::new(node.clone()) }); ui.close();
                }
                if theme::add_enabled(ui, can_wrap, egui::Button::new("并且添加后一项")).clicked() {
                    replacement = Some(ConditionNode::And { left: Box::new(node.clone()), right: Box::new(ConditionNode::Bool { value: true }) }); ui.close();
                }
                if theme::add_enabled(ui, can_wrap, egui::Button::new("或者添加后一项")).clicked() {
                    replacement = Some(ConditionNode::Or { left: Box::new(node.clone()), right: Box::new(ConditionNode::Bool { value: true }) }); ui.close();
                }
                if !can_wrap { ui.label("先减少节点或层级，再增加组合"); }
                match node {
                    ConditionNode::Not { child } => {
                        if ui.button("移除取反，保留原项").clicked() { replacement = Some(*child.clone()); ui.close(); }
                    }
                    ConditionNode::And { left, right } | ConditionNode::Or { left, right } => {
                        if ui.button("仅保留左项（删除右项）").clicked() { replacement = Some(*left.clone()); ui.close(); }
                        if ui.button("仅保留右项（删除左项）").clicked() { replacement = Some(*right.clone()); ui.close(); }
                        if ui.button("显式交换前后顺序").clicked() { std::mem::swap(left, right); ui.close(); }
                    }
                    _ => {}
                }
            }); });
        });
        match node {
            ConditionNode::Bool { value } => { theme::add_enabled(ui, !context.composing, egui::Checkbox::new(value, "为真（取消为假）")); }
            ConditionNode::GlobalBool { variable } => { candidates::select(ui, context, ConditionCandidateKind::Variable, variable, true); }
            ConditionNode::Has { state_id, tag_id } => {
                candidates::select(ui, context, ConditionCandidateKind::State, state_id, false);
                candidates::select(ui, context, ConditionCandidateKind::Tag, tag_id, false);
                if let Some(state) = context.compiled.analysis.catalog.states.get(state_id) {
                    ui.horizontal_wrapped(|ui| {
                        let definition = theme::add_enabled(ui, !context.composing, egui::Button::new("定位状态定义"));
                        crate::chrome::action_layout::record(ui, "condition/state-definition", &definition);
                        if definition.clicked() {
                            *navigation = Some(Navigation::Definition(worldline_core::state_inspection_source::DeclarationSource {
                                kind: worldline_core::state_inspection_source::DeclarationKind::State,
                                id: state.id.clone(), file: state.file.clone(), line: state.line,
                            }));
                        }
                        let object = theme::add_enabled(ui, !context.composing, egui::Button::new("旁查所属对象运行状态…"));
                        crate::chrome::action_layout::record(ui, "condition/state-owner", &object);
                        if object.clicked() {
                            *navigation = Some(Navigation::Owner(state.target.clone()));
                        }
                    });
                    ui.label(theme::muted(format!("所属对象 {}:{}", state.target.kind, state.target.id)));
                }
                ui.label(theme::muted("判断运行状态中的真实标签；对象分类不改变状态"));
            }
            ConditionNode::Compare { variable, op, value, literal_left } => {
                if let Some(candidate) = candidates::select(ui, context, ConditionCandidateKind::Variable, variable, false) {
                    match candidate.value_type {
                        Some(ConditionValueType::Num) if !matches!(value, ConditionLiteral::Num(_)) => *value = ConditionLiteral::Num(0.0),
                        Some(ConditionValueType::Str) if !matches!(value, ConditionLiteral::Str(_)) => *value = ConditionLiteral::Str(String::new()),
                        Some(ConditionValueType::Bool) if !matches!(value, ConditionLiteral::Bool(_)) => *value = ConditionLiteral::Bool(false),
                        _ => {}
                    }
                }
                theme::add_enabled_ui(ui, !context.composing, |ui| { egui::ComboBox::from_id_salt("comparison-operation").selected_text(operation(*op)).show_ui(ui, |ui| {
                    for candidate in [ConditionComparison::Eq, ConditionComparison::Neq, ConditionComparison::Lt, ConditionComparison::Le, ConditionComparison::Gt, ConditionComparison::Ge] {
                        let allowed = matches!(candidate, ConditionComparison::Eq | ConditionComparison::Neq) || matches!(value, ConditionLiteral::Num(_));
                        theme::add_enabled_ui(ui, allowed, |ui| { ui.selectable_value(op, candidate, operation(candidate)); });
                    }
                }); });
                match value {
                    ConditionLiteral::Num(number) => { ui.horizontal_wrapped(|ui| {
                        ui.label("数值");
                        let response = ui.add(egui::DragValue::new(number).speed(1.0));
                        #[cfg(test)]
                        record_number(ui, *number, response.id);
                        #[cfg(not(test))]
                        let _ = response;
                    }); }
                    ConditionLiteral::Str(text) => {
                        ui.label("字符串字面量（直接输入文字，转义由 core 处理）");
                        let response = ui.add(egui::TextEdit::multiline(text).id_salt("literal-string").desired_rows(2).desired_width(f32::INFINITY));
                        input::register(&response, &context.project.root);
                        if response.has_focus() && context.composing { ui.label(theme::muted("正在组合输入；先确认候选，再预览或导航")); }
                        #[cfg(test)]
                        ui.ctx().data_mut(|data| data.insert_temp(egui::Id::new("condition-literal-test-focus"), response.id));
                    }
                    ConditionLiteral::Bool(boolean) => { theme::add_enabled(ui, !context.composing, egui::Checkbox::new(boolean, "布尔字面量：真（取消为假）")); }
                }
                theme::add_enabled(ui, !context.composing, egui::Checkbox::new(literal_left, "字面量在左侧（保持比较方向）"));
            }
            ConditionNode::Not { child } => { ui.push_id("not-child", |ui| node_ui(ui, context, child, depth + 1, can_wrap, navigation)); }
            ConditionNode::And { left, right } | ConditionNode::Or { left, right } => {
                ui.label(theme::muted("第 1 项（左）"));
                ui.push_id("left", |ui| node_ui(ui, context, left, depth + 1, can_wrap, navigation));
                ui.label(theme::muted("第 2 项（右）"));
                ui.push_id("right", |ui| node_ui(ui, context, right, depth + 1, can_wrap, navigation));
            }
        }
    });
    if let Some(replacement) = replacement {
        *node = replacement;
    }
}
fn operation(op: ConditionComparison) -> &'static str {
    match op {
        ConditionComparison::Eq => "等于 ==",
        ConditionComparison::Neq => "不等于 !=",
        ConditionComparison::Lt => "小于 <",
        ConditionComparison::Le => "小于或等于 ≤",
        ConditionComparison::Gt => "大于 >",
        ConditionComparison::Ge => "大于或等于 ≥",
    }
}
fn size(node: &ConditionNode) -> (usize, usize) {
    let children = match node {
        ConditionNode::Not { child } => {
            let (count, depth) = size(child);
            (count, depth)
        }
        ConditionNode::And { left, right } | ConditionNode::Or { left, right } => {
            let a = size(left);
            let b = size(right);
            (a.0 + b.0, a.1.max(b.1))
        }
        _ => (0, 0),
    };
    (children.0 + 1, children.1 + 1)
}

#[cfg(test)]
fn record_number(ui: &egui::Ui, value: f64, id: egui::Id) {
    let frame = ui.ctx().cumulative_frame_nr();
    ui.data_mut(|data| {
        let (recorded, fields) = data.get_temp_mut_or_default::<(u64, Vec<(f64, egui::Id)>)>(
            egui::Id::new("condition-number-test-fields"),
        );
        if *recorded != frame {
            *recorded = frame;
            fields.clear();
        }
        fields.push((value, id));
    });
}
