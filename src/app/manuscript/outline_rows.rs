//! 页面内的紧凑大纲与卡片；只负责呈现 core 的真实字段。
use super::{navigation::Columns, *};
use crate::theme;
use worldline_core::manuscript::ManuscriptQueryRow;

const TITLE: f32 = 210.0;
const ID: f32 = 140.0;
const SUMMARY: f32 = 190.0;
const POV: f32 = 150.0;
const STATUS: f32 = 90.0;
const STATS: f32 = 105.0;
const GOAL: f32 = 160.0;
const SOURCE: f32 = 210.0;

pub(super) fn headers(ui: &mut egui::Ui, columns: &Columns) {
    ui.horizontal(|ui| {
        pinned_cell(ui, |ui| cell(ui, ui.available_width(), "章节 / 分节", true));
        columns_clip(ui, |ui| {
            if columns.identity {
                cell(ui, ID, "编排 ID", true);
            }
            if columns.summary {
                cell(ui, SUMMARY, "摘要", true);
            }
            if columns.perspective {
                cell(ui, POV, "POV · 身份", true);
            }
            if columns.status {
                cell(ui, STATUS, "状态", true);
            }
            if columns.statistics {
                cell(ui, STATS, "静态统计", true);
            }
            if columns.goal {
                cell(ui, GOAL, "写作目标", true);
            }
            if columns.source {
                cell(ui, SOURCE, "正文来源", true);
            }
        });
    });
    ui.separator();
}

pub(super) fn draw(
    ui: &mut egui::Ui,
    layout: Layout,
    row: &ManuscriptQueryRow,
    local: &mut LocalBook,
    columns: &Columns,
) -> egui::Response {
    let book_id = local.draft.id.clone();
    ui.push_id((&book_id, row.ordinal, &row.entry.id), |ui| {
        if layout == Layout::Cards {
            theme::card()
                .show(ui, |ui| {
                    ui.set_width(ui.available_width().clamp(180.0, 420.0));
                    let response = title(ui, layout, row, local);
                    cell(ui, ui.available_width(), &path(row), false);
                    metadata(ui, row, columns);
                    let open = crate::theme::add_enabled(
                        ui,
                        !row.identity_ambiguous,
                        egui::Button::selectable(
                            local.selected_entry.as_deref() == Some(&row.entry.id),
                            "查看章节",
                        ),
                    );
                    response.union(open)
                })
                .inner
        } else if layout == Layout::Tree {
            let response = ui.horizontal(|ui| title(ui, layout, row, local)).inner;
            if row.entry.kind == ManuscriptEntryKind::Chapter {
                metadata(ui, row, columns);
            }
            ui.separator();
            response
        } else {
            ui.horizontal(|ui| {
                let response = pinned_cell(ui, |ui| title(ui, layout, row, local));
                columns_clip(ui, |ui| {
                    if columns.identity {
                        cell(ui, ID, &row.entry.id, false);
                    }
                    if columns.summary {
                        cell(
                            ui,
                            SUMMARY,
                            row.entry.summary.as_deref().unwrap_or("—"),
                            false,
                        );
                    }
                    if columns.perspective {
                        cell(ui, POV, &perspective(row), false);
                    }
                    if columns.status {
                        cell(
                            ui,
                            STATUS,
                            row.entry.status.as_deref().unwrap_or("未设"),
                            false,
                        );
                    }
                    if columns.statistics {
                        cell(ui, STATS, &statistics(row), false);
                    }
                    if columns.goal {
                        cell(ui, GOAL, row.entry.goal.as_deref().unwrap_or("—"), false);
                    }
                    if columns.source {
                        cell(ui, SOURCE, &source(row), false);
                    }
                });
                response
            })
            .inner
        }
    })
    .inner
}

fn pinned_width(ui: &egui::Ui) -> f32 {
    (ui.clip_rect().width() * 0.52).clamp(96.0, TITLE)
}
fn pinned_cell<R>(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let viewport = ui.clip_rect();
    let width = pinned_width(ui);
    let height =
        ui.spacing().interact_size.y.max(
            ui.text_style_height(&egui::TextStyle::Button) + 2.0 * ui.spacing().button_padding.y,
        );
    let (_, slot) = ui.allocate_space(egui::vec2(width, height));
    let rect = egui::Rect::from_min_size(
        egui::pos2(viewport.left(), slot.top()),
        egui::vec2(width, height),
    );
    let mut pinned = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("pinned-title-cell")
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    pinned.set_clip_rect(rect.intersect(viewport));
    contents(&mut pinned)
}
fn columns_clip<R>(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let mut clip = ui.clip_rect();
    clip.min.x += pinned_width(ui) + ui.spacing().item_spacing.x;
    ui.scope(|ui| {
        ui.set_clip_rect(clip);
        contents(ui)
    })
    .inner
}

fn metadata(ui: &mut egui::Ui, row: &ManuscriptQueryRow, columns: &Columns) {
    if columns.identity {
        ui.label(theme::muted(format!("编排 ID：{}", row.entry.id)));
    }
    if columns.summary {
        if let Some(summary) = &row.entry.summary {
            ui.add(egui::Label::new(summary).wrap());
        }
    }
    if columns.perspective && row.entry.perspective.is_some() {
        ui.add(egui::Label::new(theme::muted(perspective(row))).wrap());
    }
    let mut status = Vec::new();
    if columns.status {
        status.push(row.entry.status.as_deref().unwrap_or("未设状态").to_owned());
    }
    if columns.statistics {
        status.push(statistics(row));
    }
    if !status.is_empty() {
        ui.add(egui::Label::new(theme::muted(status.join(" · "))).wrap());
    }
    if columns.goal {
        if let Some(goal) = &row.entry.goal {
            ui.add(egui::Label::new(theme::muted(format!("目标：{goal}"))).wrap());
        }
    }
    if columns.source {
        ui.add(egui::Label::new(theme::muted(source(row))).wrap());
    }
}

fn title(
    ui: &mut egui::Ui,
    layout: Layout,
    row: &ManuscriptQueryRow,
    local: &mut LocalBook,
) -> egui::Response {
    let entry = &row.entry;
    if layout == Layout::Tree {
        ui.add_space((row.section_path.len() as f32 * 10.0).min(50.0));
        if entry.kind == ManuscriptEntryKind::Section {
            let collapsed = local.collapsed.contains(&entry.id);
            if crate::theme::add_enabled(
                ui,
                !row.identity_ambiguous,
                egui::Button::new(if collapsed { "▸" } else { "▾" }).small(),
            )
            .on_hover_text("折叠或展开分节；筛选时显示命中祖先")
            .clicked()
            {
                if collapsed {
                    local.collapsed.remove(&entry.id);
                } else {
                    local.collapsed.insert(entry.id.clone());
                }
            }
        }
    }
    let prefix = if row.identity_ambiguous {
        "身份重复 · "
    } else if row.context_only {
        "分节上下文 · "
    } else if entry.kind == ManuscriptEntryKind::Section {
        "分节 · "
    } else {
        ""
    };
    let text = egui::RichText::new(format!("{prefix}{}", entry.title));
    let response = crate::theme::add_enabled(
        ui,
        !row.identity_ambiguous,
        egui::Button::selectable(local.selected_entry.as_deref() == Some(&entry.id), text)
            .truncate(),
    );
    #[cfg(test)]
    {
        let record = (
            ui.ctx().cumulative_frame_nr(),
            response.id,
            response.rect,
            ui.clip_rect(),
        );
        ui.ctx().data_mut(|data| {
            data.insert_temp(
                egui::Id::new(("manuscript-row-geometry", &local.draft.id, &entry.id)),
                record,
            )
        });
    }
    response.on_hover_text(format!(
        "{}\n编排 ID：{}\n{}\n{}",
        entry.title,
        entry.id,
        path(row),
        source(row)
    ))
}

fn cell(ui: &mut egui::Ui, width: f32, text: &str, heading: bool) {
    let text_style = if heading {
        egui::RichText::new(text).strong()
    } else {
        theme::muted(text)
    };
    ui.add_sized(
        [width, ui.spacing().interact_size.y],
        egui::Label::new(text_style).truncate(),
    )
    .on_hover_text(text);
}

pub(super) fn path(row: &ManuscriptQueryRow) -> String {
    let mut parts: Vec<_> = row
        .section_path
        .iter()
        .map(|part| format!("{} [{}]", part.title, part.id))
        .collect();
    parts.push(format!("{} [{}]", row.entry.title, row.entry.id));
    format!(
        "{}{}",
        if row.path_complete {
            "路径："
        } else {
            "恢复路径（未完整确认）："
        },
        parts.join(" / ")
    )
}

fn perspective(row: &ManuscriptQueryRow) -> String {
    row.entry.perspective.as_ref().map_or_else(
        || "未设 POV".into(),
        |target| {
            format!(
                "{} · {}:{}",
                row.perspective_display.as_deref().unwrap_or("未确认名称"),
                target.kind,
                target.id
            )
        },
    )
}

fn statistics(row: &ManuscriptQueryRow) -> String {
    if row.entry.kind == ManuscriptEntryKind::Section {
        return "分节".into();
    }
    row.entry
        .source
        .as_ref()
        .and_then(|source| source.stats)
        .map(|stats| format!("{} 词（静态） · {} 汉字", stats.words, stats.han_characters))
        .unwrap_or_else(|| "统计不可用".into())
}

fn source(row: &ManuscriptQueryRow) -> String {
    let identity = row
        .entry
        .target_ref
        .as_ref()
        .map(|target| format!("{}:{}", target.kind, target.id));
    match (&row.entry.source, identity) {
        (Some(source), Some(identity)) => format!(
            "{} · {}{}",
            identity,
            source_status_text(source.status),
            source
                .location
                .as_ref()
                .map(|location| format!(" · {}:{}", location.file, location.line))
                .unwrap_or_default()
        ),
        (_, Some(identity)) => format!("{identity} · 来源未确认"),
        _ if row.entry.kind == ManuscriptEntryKind::Section => "分节不含正文".into(),
        _ => "尚未选择正文来源".into(),
    }
}
