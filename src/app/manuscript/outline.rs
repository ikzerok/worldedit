use super::*;
use crate::theme;
use worldline_core::manuscript::ManuscriptIndex;

pub(super) fn draw(
    ui: &mut egui::Ui,
    layout: Layout,
    local: &mut LocalBook,
    index: &ManuscriptIndex,
    status: &mut String,
    pov: &mut String,
    objects: &[CatalogObject],
) {
    ui.heading("章节");
    ui.add(egui::TextEdit::singleline(status).hint_text("筛选状态，如 draft"));
    ui.add(egui::TextEdit::singleline(pov).hint_text("筛选视角：名称或 ID"));
    if (!status.is_empty() || !pov.is_empty()) && ui.button("清除筛选").clicked() {
        status.clear();
        pov.clear();
    }
    let matching: HashSet<_> = local
        .draft
        .entries
        .iter()
        .filter(|entry| {
            entry.kind == ManuscriptEntryKind::Chapter
                && (status.is_empty()
                    || entry
                        .status
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&status.to_lowercase()))
                && (pov.is_empty()
                    || entry.pov.as_ref().is_some_and(|target| {
                        let label = objects
                            .iter()
                            .find(|object| object.target == *target)
                            .map(target_label)
                            .unwrap_or_else(|| target.id.clone());
                        label.to_lowercase().contains(&pov.to_lowercase())
                    }))
        })
        .map(|entry| entry.id.clone())
        .collect();
    let filtering = !status.is_empty() || !pov.is_empty();
    let mut order = Vec::new();
    visit(
        &local.draft.entries,
        None,
        0,
        &mut HashSet::new(),
        &mut order,
    );
    egui::ScrollArea::vertical()
        .id_salt("manuscript-entry-list")
        .max_height(620.0)
        .show(ui, |ui| {
            for (id, depth) in order {
                let Some(entry) = local
                    .draft
                    .entries
                    .iter()
                    .find(|entry| entry.id == id)
                    .cloned()
                else {
                    continue;
                };
                if filtering
                    && !matching.contains(&id)
                    && !local
                        .draft
                        .entry_subtree(&id)
                        .unwrap_or_default()
                        .iter()
                        .any(|child| matching.contains(child))
                {
                    continue;
                }
                if layout != Layout::Tree && entry.kind != ManuscriptEntryKind::Chapter {
                    continue;
                }
                if layout == Layout::Tree
                    && !filtering
                    && hidden_by_parent(&local.draft.entries, &local.collapsed, &entry)
                {
                    continue;
                }
                ui.push_id(&id, |ui| {
                    if layout == Layout::Cards {
                        theme::card().show(ui, |ui| {
                            ui.label(egui::RichText::new(&entry.title).strong());
                            ui.label(theme::muted(format!("编排 ID：{}", entry.id)));
                            metadata(ui, &entry, index, objects);
                            if ui
                                .selectable_label(
                                    local.selected_entry.as_deref() == Some(&id),
                                    "查看章节",
                                )
                                .clicked()
                            {
                                local.selected_entry = Some(id.clone());
                            }
                        });
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            if layout == Layout::Tree {
                                ui.add_space((depth as f32 * 12.0).min(72.0));
                            }
                            if entry.kind == ManuscriptEntryKind::Section {
                                let collapsed = local.collapsed.contains(&id);
                                if ui
                                    .small_button(if collapsed { "▸" } else { "▾" })
                                    .on_hover_text("折叠或展开分节")
                                    .clicked()
                                {
                                    if collapsed {
                                        local.collapsed.remove(&id);
                                    } else {
                                        local.collapsed.insert(id.clone());
                                    }
                                }
                            }
                            if ui
                                .selectable_label(
                                    local.selected_entry.as_deref() == Some(&id),
                                    &entry.title,
                                )
                                .on_hover_text(format!("编排 ID：{}", entry.id))
                                .clicked()
                            {
                                local.selected_entry = Some(id.clone());
                            }
                        });
                        if entry.kind == ManuscriptEntryKind::Chapter {
                            metadata(ui, &entry, index, objects);
                        }
                        ui.separator();
                    }
                });
            }
        });
}

fn metadata(
    ui: &mut egui::Ui,
    entry: &ManuscriptEntryDraft,
    index: &ManuscriptIndex,
    objects: &[CatalogObject],
) {
    if let Some(summary) = &entry.summary {
        ui.label(summary);
    }
    if let Some(pov) = &entry.pov {
        let name = objects
            .iter()
            .find(|object| object.target == *pov)
            .map(|object| object.display.as_str())
            .unwrap_or(&pov.id);
        ui.label(theme::muted(format!("视角：{name} · {}", pov.id)));
    }
    let words = index
        .entries
        .iter()
        .find(|candidate| candidate.id == entry.id)
        .and_then(|candidate| candidate.source.as_ref())
        .and_then(|source| source.stats)
        .map(|stats| format!("{} 词（静态）", stats.words))
        .unwrap_or_else(|| "统计不可用".into());
    ui.label(theme::muted(format!(
        "{} · {words}",
        entry.status.as_deref().unwrap_or("未设状态")
    )));
    if let Some(goal) = &entry.goal {
        ui.label(theme::muted(format!("目标：{goal}")));
    }
    if let Some(target) = &entry.target_ref {
        ui.label(theme::muted(format!("{}:{}", target.kind, target.id)));
    }
}

fn visit(
    entries: &[ManuscriptEntryDraft],
    _parent: Option<&str>,
    _depth: usize,
    visited: &mut HashSet<String>,
    out: &mut Vec<(String, usize)>,
) {
    let mut children: HashMap<Option<&str>, Vec<&ManuscriptEntryDraft>> = HashMap::new();
    for entry in entries {
        children
            .entry(entry.parent_id.as_deref())
            .or_default()
            .push(entry);
    }
    let mut stack: Vec<_> = children
        .get(&None)
        .into_iter()
        .flatten()
        .rev()
        .map(|entry| (*entry, 0usize))
        .collect();
    while let Some((entry, depth)) = stack.pop() {
        if !visited.insert(entry.id.clone()) {
            continue;
        }
        out.push((entry.id.clone(), depth));
        if let Some(next) = children.get(&Some(entry.id.as_str())) {
            stack.extend(
                next.iter()
                    .rev()
                    .map(|child| (*child, depth.saturating_add(1))),
            );
        }
    }
    // 损坏父链仍显示，绝不因为投影不完整而丢弃条目。
    for entry in entries {
        if visited.insert(entry.id.clone()) {
            out.push((entry.id.clone(), 0));
        }
    }
}

fn hidden_by_parent(
    entries: &[ManuscriptEntryDraft],
    collapsed: &HashSet<String>,
    entry: &ManuscriptEntryDraft,
) -> bool {
    let mut parent = entry.parent_id.as_deref();
    for _ in 0..entries.len() {
        let Some(id) = parent else {
            return false;
        };
        if collapsed.contains(id) {
            return true;
        }
        parent = entries
            .iter()
            .find(|entry| entry.id == id)
            .and_then(|entry| entry.parent_id.as_deref());
    }
    false
}
