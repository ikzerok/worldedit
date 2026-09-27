use super::super::WorldeditApp;
use super::*;
use crate::theme;
use egui::RichText;
use worldline_core::manuscript::ManuscriptIndex;
pub(super) fn selected_section(local: &LocalBook) -> Option<String> {
    local.selected_entry.as_deref().and_then(|id| {
        local
            .draft
            .entries
            .iter()
            .find(|entry| entry.id == id && entry.kind == ManuscriptEntryKind::Section)
            .map(|entry| entry.id.clone())
    })
}

pub(super) fn draw_entry_list(
    ui: &mut egui::Ui,
    layout: Layout,
    local: &mut LocalBook,
    entries: &[(String, ManuscriptEntryKind, String, Option<String>, usize)],
) {
    ui.heading("章节");
    egui::ScrollArea::vertical()
        .id_salt("manuscript-entry-list")
        .max_height(480.0)
        .show(ui, |ui| match layout {
            Layout::Tree => {
                for (id, kind, title, parent, depth) in entries {
                    let prefix = if *kind == ManuscriptEntryKind::Section {
                        "▾ "
                    } else {
                        ""
                    };
                    let relation = parent
                        .as_deref()
                        .map(|parent| format!(" · {parent}"))
                        .unwrap_or_default();
                    let label = format!("{}{}{}{}", "  ".repeat(*depth), prefix, title, relation);
                    if ui
                        .selectable_label(local.selected_entry.as_deref() == Some(id), label)
                        .clicked()
                    {
                        local.selected_entry = Some(id.clone());
                    }
                }
            }
            Layout::List => {
                for (id, kind, title, parent, _) in entries {
                    if *kind != ManuscriptEntryKind::Chapter {
                        continue;
                    }
                    let section = parent
                        .as_deref()
                        .map(|parent| format!("{parent} / "))
                        .unwrap_or_default();
                    if ui
                        .selectable_label(
                            local.selected_entry.as_deref() == Some(id),
                            format!("{section}{title}"),
                        )
                        .clicked()
                    {
                        local.selected_entry = Some(id.clone());
                    }
                }
            }
            Layout::Cards => {
                for (id, kind, title, parent, _) in entries {
                    if *kind != ManuscriptEntryKind::Chapter {
                        continue;
                    }
                    let section = parent
                        .as_deref()
                        .map(|parent| format!("分节：{parent}"))
                        .unwrap_or_else(|| "根章节".into());
                    theme::card().show(ui, |ui| {
                        ui.label(RichText::new(title).strong());
                        ui.label(theme::muted(section));
                        if ui
                            .selectable_label(
                                local.selected_entry.as_deref() == Some(id),
                                "查看章节",
                            )
                            .clicked()
                        {
                            local.selected_entry = Some(id.clone());
                        }
                    });
                }
            }
        });
}

#[derive(Clone, Copy)]
pub(super) struct EntryEditorContext<'a> {
    pub(super) book_id: &'a str,
    pub(super) objects: &'a [CatalogObject],
    pub(super) preview_index: &'a ManuscriptIndex,
    pub(super) read_only: bool,
}

pub(super) fn draw_entry_editor(
    app: &mut WorldeditApp,
    ui: &mut egui::Ui,
    local: &mut LocalBook,
    context: &EntryEditorContext<'_>,
    load_body_for: &mut Option<(String, String)>,
    apply_body_for: &mut Option<(String, String)>,
) {
    let EntryEditorContext {
        book_id,
        objects,
        preview_index,
        read_only,
    } = *context;
    ui.heading("编排与来源");
    let selected_id = local.selected_entry.clone();
    let selected_index = selected_id
        .as_ref()
        .and_then(|id| local.draft.entries.iter().position(|entry| &entry.id == id));
    if let Some(position) = selected_index {
        let selected_entry_id = local.draft.entries[position].id.clone();
        let selected_kind = local.draft.entries[position].kind;
        ui.label(theme::muted(format!(
            "稳定 ID：{} · {:?}",
            selected_entry_id, selected_kind
        )));
        {
            let entry = &mut local.draft.entries[position];
            if ui.text_edit_singleline(&mut entry.title).changed() {
                local.changed = true;
            }
            ui.label("摘要");
            let mut summary = entry.summary.clone().unwrap_or_default();
            if ui
                .add(egui::TextEdit::multiline(&mut summary).desired_rows(3))
                .changed()
            {
                entry.summary = (!summary.is_empty()).then_some(summary);
                local.changed = true;
            }
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!read_only, egui::Button::new("上移"))
                .clicked()
                && move_entry(&mut local.draft.entries, &selected_entry_id, -1)
            {
                local.changed = true;
            }
            if ui
                .add_enabled(!read_only, egui::Button::new("下移"))
                .clicked()
                && move_entry(&mut local.draft.entries, &selected_entry_id, 1)
            {
                local.changed = true;
            }
        });
        draw_status_goal(ui, &mut local.draft.entries[position], &mut local.changed);
        if selected_kind == ManuscriptEntryKind::Chapter {
            let entry = &mut local.draft.entries[position];
            draw_metadata_target(ui, entry, objects, &mut local.changed);
            if let Some(index_entry) = preview_index
                .entries
                .iter()
                .find(|candidate| candidate.id == entry.id)
            {
                if let Some(source) = &index_entry.source {
                    ui.label(source_status_text(source.status));
                    if let Some(location) = &source.location {
                        ui.label(theme::muted(format!(
                            "来源：{}:{}",
                            location.file, location.line
                        )));
                    }
                    if let Some(stats) = source.stats {
                        ui.label(format!(
                            "汉字 {} · 词数 {}",
                            stats.han_characters, stats.words
                        ));
                        if let Some(goal) = entry
                            .goal
                            .as_deref()
                            .and_then(|goal| goal.trim().parse::<u64>().ok())
                        {
                            let ratio = if goal == 0 {
                                1.0
                            } else {
                                (stats.words as f32 / goal as f32).clamp(0.0, 1.0)
                            };
                            ui.add(
                                egui::ProgressBar::new(ratio)
                                    .text(format!("{} / {} 词目标", stats.words, goal)),
                            );
                        } else if entry
                            .goal
                            .as_deref()
                            .is_some_and(|goal| !goal.trim().is_empty())
                        {
                            ui.label(theme::muted("目标说明不是数字；输入数字可显示词数进度。"));
                        }
                    }
                }
            }
            let body_key = (book_id.to_owned(), selected_entry_id.clone());
            if let Some(body) = app.manuscript.body_drafts.get_mut(&body_key) {
                ui_body_draft(ui, body, apply_body_for, book_id, &selected_entry_id);
            } else {
                let has_source = preview_index
                    .entries
                    .iter()
                    .find(|candidate| candidate.id == selected_entry_id)
                    .and_then(|candidate| candidate.source.as_ref())
                    .and_then(|source| source.location.as_ref())
                    .is_some();
                if ui
                    .add_enabled(!read_only && has_source, egui::Button::new("编辑来源文件"))
                    .clicked()
                {
                    *load_body_for = Some((book_id.to_owned(), selected_entry_id.clone()));
                }
                if !has_source {
                    ui.label(theme::muted(
                        "来源缺失时先选择一个可确认的事件、场景或实体。",
                    ));
                }
            }
        }
    } else {
        ui.label("选择章节或分节以编辑编排。");
    }
}

fn draw_metadata_target(
    ui: &mut egui::Ui,
    entry: &mut ManuscriptEntryDraft,
    objects: &[CatalogObject],
    changed: &mut bool,
) {
    ui.separator();
    ui.label(RichText::new("正文引用").strong());
    let valid_targets: Vec<_> = objects
        .iter()
        .filter(|object| matches!(object.target.kind.as_str(), "event" | "scene" | "entity"))
        .collect();
    let selected = entry
        .target_ref
        .as_ref()
        .map(|target| format!("{}:{}", target.kind, target.id))
        .unwrap_or_else(|| "未选择来源".into());
    egui::ComboBox::from_id_salt(("manuscript-target", &entry.id))
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for object in &valid_targets {
                if ui
                    .selectable_value(
                        &mut entry.target_ref,
                        Some(object.target.clone()),
                        target_label(object),
                    )
                    .changed()
                {
                    *changed = true;
                }
            }
        });
    if let Some(target) = &entry.target_ref {
        if !valid_targets.iter().any(|object| object.target == *target) {
            ui.colored_label(
                theme::ERROR,
                format!(
                    "引用 {}/{} 已失效；从候选列表选择来源可修复。",
                    target.kind, target.id
                ),
            );
        }
    }

    ui.label("视角人物");
    let selected_pov = entry
        .pov
        .as_ref()
        .map(|target| format!("{}:{}", target.kind, target.id))
        .unwrap_or_else(|| "不指定".into());
    egui::ComboBox::from_id_salt(("manuscript-pov", &entry.id))
        .selected_text(selected_pov)
        .show_ui(ui, |ui| {
            if ui
                .selectable_value(&mut entry.pov, None, "不指定")
                .changed()
            {
                *changed = true;
            }
            for object in objects
                .iter()
                .filter(|object| object.target.kind == "character")
            {
                if ui
                    .selectable_value(
                        &mut entry.pov,
                        Some(object.target.clone()),
                        target_label(object),
                    )
                    .changed()
                {
                    *changed = true;
                }
            }
        });
}

fn draw_status_goal(ui: &mut egui::Ui, entry: &mut ManuscriptEntryDraft, changed: &mut bool) {
    ui.horizontal(|ui| {
        ui.label("状态");
        let mut status = entry.status.clone().unwrap_or_default();
        if ui
            .add(egui::TextEdit::singleline(&mut status).hint_text("draft / revised / final"))
            .changed()
        {
            entry.status = (!status.is_empty()).then_some(status);
            *changed = true;
        }
    });
    ui.horizontal(|ui| {
        ui.label("字数目标");
        let mut goal = entry.goal.clone().unwrap_or_default();
        if ui
            .add(egui::TextEdit::singleline(&mut goal).hint_text("输入数字显示进度，也可写说明"))
            .changed()
        {
            entry.goal = (!goal.is_empty()).then_some(goal);
            *changed = true;
        }
    });
}

fn ui_body_draft(
    ui: &mut egui::Ui,
    body: &mut BodyDraft,
    apply_body_for: &mut Option<(String, String)>,
    book: &str,
    chapter: &str,
) {
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new("来源文件草稿").strong());
        if ui
            .button(if body.open {
                "收起编辑器"
            } else {
                "继续编辑正文草稿"
            })
            .clicked()
        {
            body.open = !body.open;
        }
    });
    ui.label(theme::muted(body.path.display().to_string()));
    if body.open {
        ui.add(
            egui::TextEdit::multiline(&mut body.text)
                .id_salt(("manuscript-source", book, chapter))
                .desired_rows(15)
                .code_editor(),
        );
        ui.label(theme::muted(
            "这是引用的源码文件缓冲；未应用的文字会随章节切换保留。",
        ));
        if ui
            .add_enabled(
                body.text != body.original,
                egui::Button::new("应用正文草稿"),
            )
            .clicked()
        {
            *apply_body_for = Some((book.into(), chapter.into()));
        }
    }
}
