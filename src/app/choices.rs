//! 选择卡片只编辑核心草稿，源码定位与重写由 core 完成。
use crate::theme;
use worldline_core::{
    authoring::{ChoiceDraft, EventDraft},
    RelationGraph,
};

pub(super) fn choice_cards(
    ui: &mut egui::Ui,
    draft: &mut EventDraft,
    graph: Option<&RelationGraph>,
    catalog: Option<&worldline_core::catalog::Catalog>,
) -> Result<(), String> {
    let cache_key = ui.id().with(("choice-forms", &draft.id));
    let choices = ui
        .data_mut(|data| data.get_temp::<(String, Vec<ChoiceDraft>)>(cache_key))
        .filter(|(source, _)| source == &draft.body)
        .map(|(_, choices)| choices)
        .unwrap_or_else(|| draft.choices());
    let unchanged_choices = choices.clone();
    let original_body = draft.body.clone();
    let mut changed = None;
    let mut remove = None;
    let add = ui
        .horizontal(|ui| {
            ui.heading("分支决策");
            ui.button("＋ 添加选项").clicked()
        })
        .inner;
    ui.label(theme::muted(
        "编辑选项、显示条件和去向；修改后点击“应用更改”。",
    ));
    for mut choice in choices {
        let before = choice.clone();
        ui.push_id(("choice-card", choice.line), |ui| {
            theme::card().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(if choice.depth == 0 {
                        "选项"
                    } else {
                        "嵌套选项"
                    });
                    ui.checkbox(&mut choice.once, "只能选择一次");
                    if ui.small_button("删除分支").clicked() {
                        remove = Some(choice.line);
                    }
                });
                ui.add(
                    egui::TextEdit::singleline(&mut choice.label)
                        .hint_text("玩家看到的选择文案")
                        .desired_width(f32::INFINITY),
                );
                ui.label(theme::muted("显示条件（可留空）"));
                ui.add(
                    egui::TextEdit::singleline(&mut choice.condition)
                        .hint_text("例如 has(mood, calm)")
                        .desired_width(f32::INFINITY),
                );
                if let Some(catalog) = catalog {
                    super::tags::condition(ui, catalog, &mut choice.condition);
                }
                let previous_target = choice.target.clone();
                let caption = match choice.target.as_deref() {
                    None => "继续执行选择组之后的内容".into(),
                    Some("END") => "结束故事".into(),
                    Some(id) => graph
                        .and_then(|g| g.nodes.iter().find(|n| n.name == id))
                        .map(|n| {
                            format!("{} · {}", n.summary.as_deref().unwrap_or(&n.name), n.name)
                        })
                        .unwrap_or_else(|| id.into()),
                };
                ui.label(theme::muted("分支末尾去向"));
                egui::ComboBox::from_id_salt("target")
                    .selected_text(caption)
                    .width(ui.available_width() - 20.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut choice.target, None, "继续执行选择组之后的内容");
                        ui.selectable_value(&mut choice.target, Some("END".into()), "结束故事");
                        if let Some(graph) = graph {
                            for node in &graph.nodes {
                                ui.selectable_value(
                                    &mut choice.target,
                                    Some(node.name.clone()),
                                    format!(
                                        "{} · {}",
                                        node.summary.as_deref().unwrap_or(&node.name),
                                        node.name
                                    ),
                                );
                            }
                        }
                    });
                if choice.target != previous_target {
                    choice.drift = choice
                        .target
                        .as_ref()
                        .and_then(|id| graph.and_then(|g| g.nodes.iter().find(|n| &n.name == id)))
                        .is_some_and(|n| n.storyline != draft.storyline);
                }
                if choice.target.as_deref().is_some_and(|t| t != "END") {
                    ui.checkbox(&mut choice.drift, "跨故事线漂流");
                }
                if let Some(target) = choice
                    .target
                    .as_ref()
                    .and_then(|id| graph.and_then(|g| g.nodes.iter().find(|n| &n.name == id)))
                {
                    if let Some(requirement) = graph
                        .and_then(|g| {
                            g.edges
                                .iter()
                                .find(|e| g.nodes[e.to as usize].name == target.name)
                        })
                        .and_then(|e| e.target_requirement.as_ref())
                    {
                        ui.label(theme::muted(format!(
                            "目标准入：{requirement}。请让显示条件与其匹配。"
                        )));
                    }
                }
                ui.label(theme::muted("选后正文 / 动作（保留条件和嵌套结构）"));
                ui.add(
                    egui::TextEdit::multiline(&mut choice.body)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY)
                        .hint_text("选择后发生什么…"),
                );
                if let Some(catalog) = catalog {
                    egui::CollapsingHeader::new("＋ 选后改变状态").show(ui, |ui| {
                        super::tags::actions(ui, catalog, &mut choice.body);
                    });
                }
            });
        });
        if choice != before {
            changed = Some(choice);
        }
        ui.add_space(6.0);
    }
    if let Some(line) = remove {
        draft.remove_choice(line)?;
    } else if let Some(choice) = &changed {
        draft.write_choice(Some(choice.line), choice)?;
    }
    if add {
        draft.write_choice(
            None,
            &ChoiceDraft {
                label: "新的选择".into(),
                target: Some("END".into()),
                ..Default::default()
            },
        )?;
    }
    let mut updated = if draft.body == original_body {
        unchanged_choices
    } else {
        draft.choices()
    };
    if let Some(choice) = changed {
        if let Some(form) = updated.iter_mut().find(|c| c.line == choice.line) {
            *form = choice;
        }
    }
    ui.data_mut(|data| data.insert_temp(cache_key, (draft.body.clone(), updated)));
    ui.label(theme::muted(
        "同组选择只执行选中的一项；无可用选项时继续组后内容。复杂分支仍可在完整正文中编辑。",
    ));
    Ok(())
}
