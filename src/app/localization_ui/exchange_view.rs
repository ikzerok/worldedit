//! Display budgets never change the exchange or the core plan used for application/export.
use super::*;
use worldline_core::localization::LocalizationExchangeEntry;

const PAGE_SIZE: usize = 40;

#[derive(Default)]
pub(super) struct View {
    digest: String,
    offset: usize,
    selected: Option<usize>,
    source_start: usize,
    translation_start: usize,
}

impl View {
    fn bind(&mut self, digest: &str, count: usize) {
        if self.digest != digest {
            *self = Self {
                digest: digest.into(),
                ..Default::default()
            };
        }
        self.offset = self
            .offset
            .min(count.saturating_sub(1) / PAGE_SIZE * PAGE_SIZE);
    }

    fn select(&mut self, selected: Option<usize>) {
        self.selected = selected;
        self.source_start = 0;
        self.translation_start = 0;
    }
}

pub(super) fn show(
    ui: &mut Ui,
    entries: &[LocalizationExchangeEntry],
    digest: &str,
    view: &mut View,
    navigation: &mut Option<navigation::Request>,
    request: impl Fn(&LocalizationExchangeEntry) -> navigation::Request,
) {
    view.bind(digest, entries.len());
    ui.push_id(("localization-exchange-results", digest), |ui| {
        ui.horizontal_wrapped(|ui| {
            if crate::theme::add_enabled(ui, view.offset > 0, egui::Button::new("结果上一页"))
                .clicked()
            {
                view.offset = view.offset.saturating_sub(PAGE_SIZE);
                view.select(None);
            }
            if crate::theme::add_enabled(
                ui,
                view.offset + PAGE_SIZE < entries.len(),
                egui::Button::new("结果下一页"),
            )
            .clicked()
            {
                view.offset += PAGE_SIZE;
                view.select(None);
            }
            ui.label(format!(
                "显示 {}–{} / {} 项 · 每页 {} 项",
                if entries.is_empty() {
                    0
                } else {
                    view.offset + 1
                },
                (view.offset + PAGE_SIZE).min(entries.len()),
                entries.len(),
                PAGE_SIZE
            ));
        });
        for (index, entry) in entries.iter().enumerate().skip(view.offset).take(PAGE_SIZE) {
            ui.push_id(index, |ui| {
                ui.group(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(&entry.id);
                        if ui
                            .link(format!(
                                "定位来源 {}:{} · {}",
                                entry.source.file, entry.source.line, entry.source.kind
                            ))
                            .clicked()
                        {
                            *navigation = Some(request(entry));
                        }
                        ui.label(if entry.translation_parts.is_some() {
                            "译文已提供"
                        } else {
                            "缺译文"
                        });
                        let selected = view.selected == Some(index);
                        if ui
                            .selectable_label(
                                selected,
                                if selected {
                                    "收起此项详情"
                                } else {
                                    "查看此项详情"
                                },
                            )
                            .clicked()
                        {
                            view.select((!selected).then_some(index));
                        }
                    });
                    ui.label(format!("源文摘要：{}", summary(&entry.source_parts)));
                    if let Some(parts) = &entry.translation_parts {
                        ui.label(format!("译文摘要：{}", summary(parts)));
                    }
                    if view.selected == Some(index) {
                        parts(ui, "源文详情", &entry.source_parts, &mut view.source_start);
                        if let Some(parts_value) = &entry.translation_parts {
                            parts(ui, "译文详情", parts_value, &mut view.translation_start);
                        }
                    }
                });
            });
        }
    });
}

fn fragments(parts: &[LocalizationPart]) -> impl Iterator<Item = &str> {
    parts.iter().flat_map(|part| match part {
        LocalizationPart::Text { text } => ["文字：", text.as_str(), "\n", "", "", ""],
        LocalizationPart::Placeholder { token } => ["占位符：", token.as_str(), "\n", "", "", ""],
        LocalizationPart::Link { token, label } => [
            "链接标签：",
            label.as_str(),
            "\n链接身份：",
            token.as_str(),
            "\n",
            "",
        ],
    })
}

fn summary(parts: &[LocalizationPart]) -> String {
    let mut characters = fragments(parts).flat_map(str::chars).map(|character| {
        if character.is_whitespace() {
            ' '
        } else {
            character
        }
    });
    let mut summary: String = characters.by_ref().take(88).collect();
    if characters.next().is_some() {
        summary.push('…');
    }
    summary
}

struct Slice {
    start: usize,
    end: usize,
    total: usize,
    text: String,
}

fn display_boundary(parts: &[LocalizationPart], requested: usize, round_up: bool) -> usize {
    let mut offset = 0;
    for fragment in fragments(parts) {
        let end = offset + fragment.len();
        if requested < end {
            let local = requested.saturating_sub(offset);
            return offset
                + if round_up {
                    json_input::ceil_boundary(fragment, local)
                } else {
                    json_input::boundary(fragment, local)
                };
        }
        offset = end;
    }
    offset
}

fn slice(parts: &[LocalizationPart], requested: usize) -> Slice {
    let total = fragments(parts).map(str::len).sum();
    let start = display_boundary(parts, requested, false);
    let mut text = String::new();
    let mut offset = 0;
    for fragment in fragments(parts) {
        let local = start.saturating_sub(offset).min(fragment.len());
        offset += fragment.len();
        if offset <= start {
            continue;
        }
        let available = json_input::EDIT_BYTES - text.len();
        let end = json_input::boundary(fragment, local.saturating_add(available));
        text.push_str(&fragment[local..end]);
        if end < fragment.len() || text.len() == json_input::EDIT_BYTES {
            break;
        }
    }
    Slice {
        start,
        end: start + text.len(),
        total,
        text,
    }
}

fn parts(ui: &mut Ui, label: &str, value: &[LocalizationPart], start: &mut usize) {
    ui.push_id(label, |ui| {
        let current = slice(value, *start);
        *start = current.start;
        ui.strong(label);
        ui.horizontal_wrapped(|ui| {
            if ui.button("详情首段").clicked() {
                *start = 0;
            }
            if crate::theme::add_enabled(ui, current.start > 0, egui::Button::new("详情上一段"))
                .clicked()
            {
                *start = current.start.saturating_sub(json_input::EDIT_BYTES);
            }
            if crate::theme::add_enabled(
                ui,
                current.end < current.total,
                egui::Button::new("详情下一段"),
            )
            .clicked()
            {
                *start = current.end;
            }
            if ui.button("详情末段").clicked() {
                *start = display_boundary(
                    value,
                    current.total.saturating_sub(json_input::EDIT_BYTES),
                    true,
                );
            }
        });
        let current = if *start == current.start {
            current
        } else {
            slice(value, *start)
        };
        *start = current.start;
        ui.small(format!(
            "显示字节 {}–{} / {} · 每段最多 16 KiB；仅显示，不改交换包",
            if current.total == 0 {
                0
            } else {
                current.start + 1
            },
            current.end,
            current.total
        ));
        ui.add(egui::Label::new(current.text).wrap());
    });
}

#[cfg(test)]
mod tests;
