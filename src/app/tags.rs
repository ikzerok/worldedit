use crate::theme;
// 标签选择共用搜索与多选界面，声明和引用仍由 core 管理。
use worldline_core::catalog::Catalog;

fn append_condition(
    existing: &str,
    state: &str,
    tags: &[String],
    absent: bool,
    any: bool,
    alternative: bool,
) -> String {
    let clause = tags
        .iter()
        .map(|tag| format!("{}has({state}, {tag})", if absent { "not " } else { "" }))
        .collect::<Vec<_>>()
        .join(if any { " or " } else { " and " });
    if existing.trim().is_empty() {
        clause
    } else {
        format!(
            "({existing}) {} ({clause})",
            if alternative { "or" } else { "and" }
        )
    }
}

pub(super) fn condition(ui: &mut egui::Ui, catalog: &Catalog, expression: &mut String) {
    egui::CollapsingHeader::new("＋ 用标签设置条件").show(ui, |ui| {
        let key = ui.id().with("tag-condition");
        let (mut state, mut tags, mut absent, mut any, mut alternative) = ui
            .data_mut(|d| d.get_temp::<(String, Vec<String>, bool, bool, bool)>(key))
            .unwrap_or_default();
        if !catalog.states.contains_key(&state) {
            state.clear();
        }
        tags.retain(|id| catalog.tags.get(id).is_some_and(|t| t.declared));
        egui::ComboBox::from_id_salt("condition-state")
            .selected_text(
                catalog
                    .states
                    .get(&state)
                    .map(|s| s.display.as_str())
                    .unwrap_or("选择要判断的状态"),
            )
            .show_ui(ui, |ui| {
                for s in catalog.states.values() {
                    ui.selectable_value(
                        &mut state,
                        s.id.clone(),
                        format!("{} · {}", s.display, s.id),
                    );
                }
            });
        picker(ui, catalog, &mut tags, &[]);
        ui.checkbox(&mut absent, "要求不含所选标签");
        ui.checkbox(&mut any, "所选标签满足任意一项（默认全部满足）");
        ui.checkbox(&mut alternative, "与已有条件满足其一（默认同时满足）");
        let ready = !state.is_empty() && !tags.is_empty();
        if ready {
            ui.label(theme::muted(append_condition(
                expression,
                &state,
                &tags,
                absent,
                any,
                alternative,
            )));
        }
        if ui
            .add_enabled(ready, egui::Button::new("添加到条件"))
            .clicked()
        {
            *expression = append_condition(expression, &state, &tags, absent, any, alternative);
        }
        ui.label(theme::muted(
            "判断的是运行中的状态内容；给对象或标签做分类标记不会改变状态。",
        ));
        if catalog.states.is_empty() {
            ui.label("请先在资料与状态页为对象创建状态。");
        }
        ui.data_mut(|d| d.insert_temp(key, (state, tags, absent, any, alternative)));
    });
}

pub(super) fn picker(
    ui: &mut egui::Ui,
    catalog: &Catalog,
    selected: &mut Vec<String>,
    locked: &[String],
) -> bool {
    let before = selected.clone();
    let key = ui.id().with("tag-search");
    let mut query = ui
        .data_mut(|d| d.get_temp::<String>(key))
        .unwrap_or_default();
    ui.add(
        egui::TextEdit::singleline(&mut query)
            .hint_text("搜索标签名称 / ID")
            .desired_width(f32::INFINITY),
    );
    let needle = query.trim().to_lowercase();
    ui.label(format!("已选 {} 个 · 可连续勾选", selected.len()));
    egui::ScrollArea::vertical()
        .id_salt("tag-results")
        .max_height(180.0)
        .show(ui, |ui| {
            let mut count = 0;
            for tag in catalog.tags.values().filter(|t| t.declared) {
                if !tag.display.to_lowercase().contains(&needle)
                    && !tag.id.to_lowercase().contains(&needle)
                {
                    continue;
                }
                count += 1;
                let mut checked = selected.contains(&tag.id);
                if ui
                    .add_enabled(
                        !locked.contains(&tag.id),
                        egui::Checkbox::new(&mut checked, format!("{} · {}", tag.display, tag.id)),
                    )
                    .on_hover_text(if locked.contains(&tag.id) {
                        "来自正文内联标签，请在源码中修改"
                    } else {
                        &tag.description
                    })
                    .changed()
                {
                    if checked {
                        selected.push(tag.id.clone());
                    } else {
                        selected.retain(|id| id != &tag.id);
                    }
                }
            }
            if count == 0 {
                ui.label("没有匹配的标签");
            }
        });
    ui.data_mut(|d| d.insert_temp(key, query));
    *selected != before
}

pub(super) fn actions(ui: &mut egui::Ui, catalog: &Catalog, actions: &mut String) {
    let states: Vec<_> = catalog
        .objects
        .iter()
        .filter(|object| object.target.kind == "state")
        .collect();
    if !states.is_empty() {
        let key = ui.id().with("become-selection");
        let (mut state, mut tags, mut operation) = ui.data_mut(|data| {
            data.get_temp::<(String, Vec<String>, usize)>(key)
                .unwrap_or_default()
        });
        if !states.iter().any(|object| object.target.id == state) {
            state.clear();
        }
        tags.retain(|id| catalog.tags.get(id).is_some_and(|tag| tag.declared));
        egui::ComboBox::from_id_salt("state")
            .selected_text(if state.is_empty() {
                "选择状态"
            } else {
                &state
            })
            .show_ui(ui, |ui| {
                for object in &states {
                    ui.selectable_value(
                        &mut state,
                        object.target.id.clone(),
                        format!("{} · {}", object.display, object.target.id),
                    );
                }
            });
        picker(ui, catalog, &mut tags, &[]);
        egui::ComboBox::from_id_salt("state-operation")
            .selected_text(["替换全部标签", "增加标签", "移除标签"][operation])
            .show_ui(ui, |ui| {
                for (index, label) in ["替换全部标签", "增加标签", "移除标签"].iter().enumerate()
                {
                    ui.selectable_value(&mut operation, index, *label);
                }
            });
        ui.label(theme::muted(if operation == 0 {
            "完整替换状态；不选标签则清空。"
        } else {
            "仅增减所选标签，保留其他标签。"
        }));
        if ui
            .add_enabled(
                !state.is_empty() && (operation == 0 || !tags.is_empty()),
                egui::Button::new("＋ 添加状态变更"),
            )
            .clicked()
        {
            if !actions.is_empty() && !actions.ends_with('\n') {
                actions.push('\n');
            }
            let values = if tags.is_empty() {
                "[]".into()
            } else {
                tags.join(", ")
            };
            actions.push_str(&format!(
                "become {state} {} {values}\n",
                ["with", "add", "remove"][operation]
            ));
        }
        ui.data_mut(|data| data.insert_temp(key, (state, tags, operation)));
    } else {
        ui.label(theme::muted("创建状态后，可在这里选择状态和标签添加动作。"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn composed_tag_conditions_preserve_grouping_and_compile() {
        let expression = append_condition(
            "true or false",
            "mood",
            &["calm".into(), "alert".into()],
            true,
            true,
            false,
        );
        assert_eq!(
            expression,
            "(true or false) and (not has(mood, calm) or not has(mood, alert))"
        );
        let source = format!("tag calm\ntag alert\ncharacter lin\nstate mood on character lin with calm\nevent start after {expression}\n  -> END\n");
        let result = worldline_core::compile_source("world.wl", &source);
        assert!(!result.has_errors(), "{:?}", result.diagnostics);
    }
}
