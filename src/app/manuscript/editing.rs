use super::*;
use crate::theme;
use worldline_core::manuscript::ManuscriptIndex;

pub(super) fn selected_section(local: &LocalBook) -> Option<String> {
    local.selected_entry.as_deref().and_then(|id| {
        local
            .draft
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .and_then(|entry| {
                if entry.kind == ManuscriptEntryKind::Section {
                    Some(entry.id.clone())
                } else {
                    entry.parent_id.clone()
                }
            })
    })
}

pub(super) fn draw_entry_editor(
    ui: &mut egui::Ui,
    local: &mut LocalBook,
    catalog: &worldline_core::catalog::Catalog,
    index: &ManuscriptIndex,
    pending_remove: &mut Option<String>,
    root: &std::path::Path,
) {
    let Some(id) = local.selected_entry.clone() else {
        ui.label("选择章节或分节以编辑编排。");
        return;
    };
    let Some(position) = local.draft.entries.iter().position(|entry| entry.id == id) else {
        return;
    };
    ui.heading("编排与来源");
    ui.label(theme::muted(format!("稳定 ID：{}", id)));
    let entry = &mut local.draft.entries[position];
    local.changed |= ui.text_edit_singleline(&mut entry.title).changed();
    ui.label("摘要");
    optional_text(ui, &mut entry.summary, &mut local.changed, "章节摘要", true);
    ui.horizontal(|ui| {
        ui.label("状态");
        optional_text(
            ui,
            &mut entry.status,
            &mut local.changed,
            "draft / revised / final",
            false,
        );
    });
    ui.horizontal(|ui| {
        ui.label("字数目标");
        optional_text(
            ui,
            &mut entry.goal,
            &mut local.changed,
            "数字或目标说明",
            false,
        );
    });
    if entry.kind == ManuscriptEntryKind::Chapter {
        ui.label("正文引用（必须明确选择）");
        local.changed |= super::super::object_picker::object_picker(
            ui,
            ("manuscript-target", &id),
            "正文来源",
            &mut entry.target_ref,
            catalog,
            &["event", "scene", "entity", "fragment"],
        );
        ui.label("视角人物");
        local.changed |= super::super::object_picker::object_picker(
            ui,
            ("manuscript-pov", &id),
            "视角人物",
            &mut entry.pov,
            catalog,
            &["character"],
        );
        if let Some(source) = index
            .entries
            .iter()
            .find(|candidate| candidate.id == id)
            .and_then(|entry| entry.source.as_ref())
        {
            ui.label(source_status_text(source.status));
            if let Some(location) = &source.location {
                let relative = theme::relative_source(root, std::path::Path::new(&location.file));
                ui.add(
                    egui::Label::new(theme::muted(format!("来源：{relative}:{}", location.line)))
                        .truncate(),
                )
                .on_hover_text(format!("{}:{}", location.file, location.line));
            }
            if let Some(stats) = source.stats {
                ui.label(format!(
                    "汉字 {} · 词数 {}",
                    stats.han_characters, stats.words
                ));
                if let Some(goal) = entry
                    .goal
                    .as_deref()
                    .and_then(|goal| goal.parse::<u64>().ok())
                    .filter(|goal| *goal > 0)
                {
                    ui.add(
                        egui::ProgressBar::new((stats.words as f32 / goal as f32).clamp(0.0, 1.0))
                            .text(format!("{} / {} 词目标", stats.words, goal)),
                    );
                }
            }
        }
    }
    ui.horizontal(|ui| {
        if ui.button("上移").clicked() {
            local.changed |= move_entry(&mut local.draft.entries, &id, -1);
        }
        if ui.button("下移").clicked() {
            local.changed |= move_entry(&mut local.draft.entries, &id, 1);
        }
        if ui.button("删除编排项").clicked() {
            *pending_remove = Some(id.clone());
        }
    });
    let current_parent = local
        .draft
        .entries
        .iter()
        .find(|entry| entry.id == id)
        .and_then(|entry| entry.parent_id.clone());
    let mut selected_parent = current_parent.clone();
    let forbidden = local.draft.entry_subtree(&id).unwrap_or_default();
    egui::ComboBox::from_id_salt(("manuscript-move", &id))
        .selected_text(
            current_parent
                .as_deref()
                .map(|id| format!("移动到分节：{id}"))
                .unwrap_or_else(|| "移动到分节：根目录".into()),
        )
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut selected_parent, None, "根目录");
            for entry in local.draft.entries.iter().filter(|entry| {
                entry.kind == ManuscriptEntryKind::Section && !forbidden.contains(&entry.id)
            }) {
                ui.selectable_value(
                    &mut selected_parent,
                    Some(entry.id.clone()),
                    format!("{} · {}", entry.title, entry.id),
                );
            }
        });
    if selected_parent != current_parent {
        match local.draft.move_to_section(&id, selected_parent.as_deref()) {
            Ok(changed) => local.changed |= changed,
            Err(error) => {
                ui.colored_label(theme::ERROR(), error);
            }
        }
    }
    if pending_remove.as_deref() == Some(&id) {
        let affected = local.draft.entry_subtree(&id).unwrap_or_default();
        ui.colored_label(
            theme::ERROR(),
            format!(
                "移除 {} 个编排项：{}。不会删除任何源码、人物或事件。",
                affected.len(),
                affected.join("、")
            ),
        );
        ui.horizontal(|ui| {
            if ui.button("确认只删除编排").clicked() {
                if local.draft.remove_entry_subtree(&id).is_ok() {
                    local.changed = true;
                    local.selected_entry =
                        local.draft.entries.first().map(|entry| entry.id.clone());
                }
                *pending_remove = None;
            }
            if ui.button("取消删除").clicked() {
                *pending_remove = None;
            }
        });
    }
}

fn optional_text(
    ui: &mut egui::Ui,
    value: &mut Option<String>,
    changed: &mut bool,
    hint: &str,
    multiline: bool,
) {
    let mut text = value.clone().unwrap_or_default();
    let edit = if multiline {
        egui::TextEdit::multiline(&mut text).desired_rows(2)
    } else {
        egui::TextEdit::singleline(&mut text)
    };
    if ui
        .add(edit.hint_text(hint).desired_width(f32::INFINITY))
        .changed()
    {
        *value = (!text.is_empty()).then_some(text);
        *changed = true;
    }
}
