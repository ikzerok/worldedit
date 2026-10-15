//! 书稿身份只出现一次；低频编排和解释一步展开，待应用状态始终可见。
use super::{workbench::insert_entry, Layout, LocalBook, WorkbenchState};
use crate::chrome::action_layout::{button_width, checkbox_width, record, row_width, text_width};
use crate::theme;
use std::collections::BTreeMap;
use worldline_core::manuscript::{ManuscriptEntryKind, ManuscriptIndex};

#[derive(Default)]
pub(super) struct Actions {
    pub create_book: bool,
    pub create_chapter: bool,
    pub apply: bool,
    pub discard: bool,
    pub rehearse: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BarLayout {
    Full,
    Folded,
    Stacked,
}

struct Budget {
    layout: BarLayout,
    picker: f32,
}
impl Budget {
    fn measure(ui: &egui::Ui, title: &str, changed: bool, focus: bool) -> Self {
        let gap = ui.spacing().item_spacing.x;
        let picker = (text_width(ui, title, egui::TextStyle::Button)
            + ui.spacing().icon_width
            + ui.spacing().icon_spacing
            + ui.spacing().button_padding.x * 2.0)
            .clamp(96.0, (ui.available_width() * 0.25).clamp(96.0, 220.0));
        let mut fixed = vec![picker, button_width(ui, "编排"), button_width(ui, "说明")];
        if !focus {
            fixed.push(button_width(ui, "新建章节"));
        }
        if changed {
            fixed.push(button_width(ui, "应用书稿"));
            fixed.push(text_width(ui, "编排待应用", egui::TextStyle::Body));
        }
        let folded = row_width(&fixed, gap);
        let extras = [
            button_width(ui, "章节树"),
            button_width(ui, "列表"),
            button_width(ui, "卡片"),
            checkbox_width(ui, "阅读预览"),
            button_width(ui, "试演"),
        ];
        let available = ui.available_width() - gap;
        let layout = if !focus && folded + gap + row_width(&extras, gap) <= available {
            BarLayout::Full
        } else if folded <= available {
            BarLayout::Folded
        } else {
            BarLayout::Stacked
        };
        Self { layout, picker }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    ui: &mut egui::Ui,
    state: &mut WorkbenchState,
    local: &mut LocalBook,
    indices: &BTreeMap<String, ManuscriptIndex>,
    index: &ManuscriptIndex,
    book_id: &str,
    error: Option<&str>,
    focus: bool,
    can_rehearse: bool,
) -> Actions {
    let mut actions = Actions::default();
    let title = index.title.as_deref().unwrap_or(book_id);
    let budget = Budget::measure(ui, title, local.changed, focus);
    ui.horizontal_wrapped(|ui| {
        // egui ComboBox::width is a minimum, not a cap. Bound its real Ui so a
        // long selected title cannot consume the measured space for later actions.
        let picker = ui
            .allocate_ui_with_layout(
                egui::vec2(budget.picker, ui.spacing().interact_size.y),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    egui::ComboBox::from_id_salt("manuscript-book-picker")
                        .width(budget.picker)
                        .selected_text(title)
                        .truncate()
                        .show_ui(ui, |ui| {
                            ui.set_max_width((ui.ctx().screen_rect().width() - 48.0).max(160.0));
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                            for (id, book) in indices {
                                ui.selectable_value(
                                    &mut state.selected_book,
                                    Some(id.clone()),
                                    format!("{} · {id}", book.title.as_deref().unwrap_or(id)),
                                );
                            }
                        })
                        .response
                        .on_hover_text(format!("书稿：{title} · {book_id}"))
                },
            )
            .inner;
        record(ui, "manuscript/book", &picker);
        if !focus {
            let create = theme::add_enabled(ui, !index.read_only, theme::primary("新建章节"));
            record(ui, "manuscript/create-chapter", &create);
            actions.create_chapter = create.clicked();
        }
        if budget.layout != BarLayout::Stacked {
            apply_actions(
                ui,
                local.changed,
                !index.read_only && error.is_none(),
                &mut actions,
            );
        }
        let menu = ui
            .push_id("manuscript-arrange", |ui| {
                crate::chrome::quiet_menu(ui, "编排", |ui| {
                    ui.set_max_width((ui.ctx().screen_rect().width() - 48.0).max(160.0));
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    egui::ScrollArea::vertical()
                        .id_salt("manuscript-arrangement-actions")
                        .max_height((ui.ctx().screen_rect().height() - 40.0).max(100.0))
                        .show(ui, |ui| {
                            let create = ui.button("新建书稿");
                            record(ui, "manuscript/create-book", &create);
                            if create.clicked() {
                                actions.create_book = true;
                                ui.close();
                            }
                            for (label, kind) in [
                                ("插入分节", ManuscriptEntryKind::Section),
                                ("插入章节", ManuscriptEntryKind::Chapter),
                            ] {
                                let insert = theme::add_enabled(
                                    ui,
                                    !index.read_only,
                                    egui::Button::new(label),
                                );
                                record(ui, "manuscript/insert", &insert);
                                if insert.clicked() {
                                    insert_entry(local, kind);
                                    ui.close();
                                }
                            }
                            if theme::add_enabled(
                                ui,
                                local.changed,
                                egui::Button::new("恢复书稿草稿"),
                            )
                            .on_hover_text("丢弃未应用编排，保留正文文件草稿")
                            .clicked()
                            {
                                actions.discard = true;
                                ui.close();
                            }
                            if budget.layout != BarLayout::Full {
                                ui.separator();
                                layout_controls(ui, state);
                                ui.separator();
                                actions.rehearse = rehearsal(ui, can_rehearse);
                            }
                        });
                })
            })
            .inner;
        record(ui, "manuscript/arrange", &menu.response);
        if budget.layout == BarLayout::Full {
            layout_controls(ui, state);
            let menu = crate::chrome::quiet_menu(ui, "试演", |ui| {
                actions.rehearse = rehearsal(ui, can_rehearse);
            });
            record(ui, "manuscript/rehearse", &menu.response);
        }
        let help = ui
            .push_id("manuscript-help", |ui| {
                crate::chrome::quiet_menu(ui, "说明", |ui| {
                    ui.set_max_width((ui.ctx().screen_rect().width() - 48.0).clamp(160.0, 420.0));
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    ui.label("书稿只决定阅读顺序，独立于世界时间与事件控制流；正文编辑另行应用。");
                    ui.label(
                        "应用书稿只更新工程内存；之后仍需保存全部。恢复书稿草稿不丢弃正文草稿。",
                    );
                })
            })
            .inner;
        record(ui, "manuscript/help", &help.response);
    });
    if budget.layout == BarLayout::Stacked {
        ui.horizontal_wrapped(|ui| {
            apply_actions(
                ui,
                local.changed,
                !index.read_only && error.is_none(),
                &mut actions,
            );
        });
    }
    if !index.diagnostics.is_empty() {
        ui.colored_label(
            theme::WARNING(),
            format!(
                "编排有 {} 项诊断；在章节导航的快照问题中逐页查看。",
                index.diagnostics.len()
            ),
        );
    }
    if let Some(error) = error {
        ui.colored_label(theme::ERROR(), format!("书稿草稿：{error}"));
    }
    ui.separator();
    actions
}

fn apply_actions(ui: &mut egui::Ui, changed: bool, enabled: bool, actions: &mut Actions) {
    if changed {
        let apply = theme::add_enabled(ui, enabled, egui::Button::new("应用书稿"));
        record(ui, "manuscript/apply", &apply);
        actions.apply = apply.clicked();
        let state = ui.label(theme::muted("编排待应用"));
        record(ui, "manuscript/state", &state);
    }
}

fn layout_controls(ui: &mut egui::Ui, state: &mut WorkbenchState) {
    for (layout, label) in [
        (Layout::Tree, "章节树"),
        (Layout::List, "列表"),
        (Layout::Cards, "卡片"),
    ] {
        let response = ui.selectable_value(&mut state.layout, layout, label);
        record(ui, "manuscript/layout", &response);
    }
    let response = ui.checkbox(&mut state.reader_open, "阅读预览");
    record(ui, "manuscript/reader", &response);
}

fn rehearsal(ui: &mut egui::Ui, enabled: bool) -> bool {
    let response = theme::add_enabled(ui, enabled, egui::Button::new("试演当前正文草稿…"))
        .on_hover_text("明确核对范围后独立运行；不应用、保存或替换普通试玩");
    record(ui, "manuscript/rehearse-draft", &response);
    if response.clicked() {
        ui.close();
    }
    response.clicked()
}
