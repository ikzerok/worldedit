//! 选择卡片只编辑核心草稿，源码定位与重写由 core 完成。
use crate::theme;
use worldline_core::authoring::{ChoiceDraft, EventDraft};

pub(super) fn choice_cards(
    ui: &mut egui::Ui,
    draft: &mut EventDraft,
    context: &super::condition_authoring::Context<'_>,
    forms: &mut super::condition_authoring::ConditionForms,
) -> Result<(), String> {
    let graph = Some(&context.compiled.analysis.graph);
    let catalog = Some(&context.compiled.analysis.catalog);
    let symbols = Some(&context.compiled.analysis.symbols);
    let locked_choices = context
        .project
        .language_version_kind()
        .supports_language_112();
    if forms.choice_error.is_some()
        && forms
            .choice_cache
            .as_ref()
            .is_some_and(|(source, _)| source != &draft.body)
    {
        ui.colored_label(
            theme::WARNING(),
            "未完成分支输入的正文已变化；输入保留，不能套到新行号",
        );
        if theme::add_enabled(
            ui,
            !context.composing,
            egui::Button::new("取消未完成分支输入"),
        )
        .clicked()
        {
            forms.choice_cache = None;
            forms.choice_error = None;
        } else {
            return Err("请取消旧分支输入后重新读取当前正文".into());
        }
    }
    let choices = forms
        .choice_cache
        .as_ref()
        .filter(|(source, _)| source == &draft.body)
        .map(|(_, choices)| choices.clone())
        .unwrap_or_else(|| draft.choices());
    let unchanged_choices = choices.clone();
    let original_body = draft.body.clone();
    let mut changed = None;
    let mut remove = None;
    let add = ui
        .horizontal(|ui| {
            ui.heading("分支决策");
            theme::add_enabled(ui, !context.composing, egui::Button::new("＋ 添加选项")).clicked()
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
                    theme::add_enabled(ui, !context.composing, egui::Checkbox::new(&mut choice.once, "只能选择一次"));
                    if theme::add_enabled(ui, !context.composing, egui::Button::new("删除分支").small()).clicked() {
                        remove = Some(choice.line);
                    }
                });
                ui.add(
                    egui::TextEdit::singleline(&mut choice.label)
                        .hint_text("玩家看到的选择文案")
                        .desired_width(f32::INFINITY),
                );
                super::condition_authoring::show(ui, context, forms,
                    super::condition_authoring::owner(&draft.id, Some(choice.line)),
                    worldline_core::condition_authoring::ConditionSlot::ChoiceDisplay,
                    &mut choice.condition);
                egui::CollapsingHeader::new("可见但禁用（语言 1.12）")
                    .default_open(!choice.enable_condition.is_empty())
                    .show(ui, |ui| {
                        if !locked_choices { ui.label(theme::muted("需显式语言 1.12；旧工程不会自动升级。")); }
                        crate::theme::add_enabled_ui(ui, locked_choices, |ui| {
                            super::condition_authoring::show(ui, context, forms,
                                super::condition_authoring::owner(&draft.id, Some(choice.line)),
                                worldline_core::condition_authoring::ConditionSlot::ChoiceEnable,
                                &mut choice.enable_condition);
                            ui.label("禁用说明（读者可见的静态文字）");
                            ui.add(egui::TextEdit::singleline(&mut choice.disabled_reason)
                                .hint_text("例如：还需要银钥匙").desired_width(f32::INFINITY));
                            ui.label(theme::muted("显示条件仍决定隐藏；可选条件只决定是否可点。说明不求值、不泄露调试变量。"));
                        });
                    });
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
                if let Some(catalog) = catalog {
                    theme::add_enabled_ui(ui, !context.composing, |ui| { ui.horizontal_wrapped(|ui| {
                        if ui
                            .selectable_label(choice.target.is_none(), "继续执行后文")
                            .clicked()
                        {
                            choice.target = None;
                        }
                        if ui
                            .selectable_label(choice.target.as_deref() == Some("END"), "结束故事")
                            .clicked()
                        {
                            choice.target = Some("END".into());
                        }
                    }); });
                    let mut target = choice
                        .target
                        .as_deref()
                        .filter(|id| *id != "END")
                        .map(|id| resolved_choice_target(id, &draft.id, symbols));
                    if super::object_picker::object_picker(
                        ui,
                        "choice-destination",
                        &caption,
                        &mut target,
                        catalog,
                        &["event", "scene"],
                    ) && !context.composing {
                        choice.target = target.map(|target| target.id);
                    }
                } else {
                    theme::add_enabled_ui(ui, !context.composing, |ui| { egui::ComboBox::from_id_salt("target")
                        .selected_text(caption)
                        .width(ui.available_width() - 20.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut choice.target,
                                None,
                                "继续执行选择组之后的内容",
                            );
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
                        }); });
                }
                if choice.target != previous_target {
                    choice.drift = choice
                        .target
                        .as_ref()
                        .and_then(|id| graph.and_then(|g| g.nodes.iter().find(|n| &n.name == id)))
                        .is_some_and(|n| n.storyline != draft.storyline);
                }
                if choice.target.as_deref().is_some_and(|t| t != "END") {
                    theme::add_enabled(ui, !context.composing, egui::Checkbox::new(&mut choice.drift, "跨故事线漂流"));
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
                        // 保留标签搜索框的 IME 接收身份；组合期间不把结构动作写入正文。
                        let mut candidate = choice.body.clone();
                        super::tags::actions(ui, catalog, &mut candidate);
                        if !context.composing { choice.body = candidate; }
                    });
                }
            });
        });
        if choice != before {
            changed = Some(choice);
        }
        ui.add_space(6.0);
    }
    let mut result = forms.choice_error.clone().map_or(Ok(()), Err);
    if let Some(line) = remove {
        result = draft.remove_choice(line);
    } else if let Some(choice) = &changed {
        result = draft.write_choice(Some(choice.line), choice);
    }
    if add && result.is_ok() {
        result = draft.write_choice(
            None,
            &ChoiceDraft {
                label: "新的选择".into(),
                target: Some("END".into()),
                ..Default::default()
            },
        );
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
    forms.choice_cache = Some((draft.body.clone(), updated));
    forms.choice_error = result.as_ref().err().cloned();
    if let Some(error) = &forms.choice_error {
        ui.colored_label(
            theme::WARNING(),
            format!("分支输入尚未应用，完整保留：{error}"),
        );
    }
    ui.label(theme::muted(
        "同组选择只执行选中的一项；无可用选项时继续组后内容。复杂分支仍可在完整正文中编辑。",
    ));
    result
}

fn resolved_choice_target(
    raw: &str,
    event: &str,
    symbols: Option<&worldline_core::Symbols>,
) -> worldline_core::TargetRef {
    symbols
        .and_then(|symbols| {
            let path = symbols.resolve_target(raw, Some(event))?;
            let event_name = symbols.event_order.get(path.event)?;
            Some(worldline_core::TargetRef::new(
                if path.scenes.is_empty() {
                    "event"
                } else {
                    "scene"
                },
                &path.full_name(event_name),
            ))
        })
        .unwrap_or_else(|| worldline_core::TargetRef::new("event", raw))
}
#[cfg(test)]
mod target_tests {
    #[test]
    fn v080_short_scene_target_uses_core_resolution_without_rewriting_author_text() {
        let result = worldline_core::compile_source(
            "test.wl",
            "event start\n  scene inner\n    内部。\n    -> END\n  scene other\n    -> inner\n",
        );
        assert!(!result.has_errors());
        let target =
            super::resolved_choice_target("inner", "start", Some(&result.analysis.symbols));
        assert_eq!(
            target,
            worldline_core::TargetRef::new("scene", "start.inner")
        );
        assert_eq!(
            super::resolved_choice_target("start.inner", "start", Some(&result.analysis.symbols)),
            target
        );
    }
}
