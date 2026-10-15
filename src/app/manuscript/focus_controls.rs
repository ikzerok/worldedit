//! Focus 只收束当前呈现；书稿选择、管理和审稿入口仍明确可达。
use super::{LocalBook, WorkbenchState};
use crate::chrome::action_layout::{button_width, checkbox_width, record, row_width, text_width};
use crate::theme;
use std::sync::Arc;
use worldline_core::manuscript::ManuscriptQuerySnapshot;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    ui: &mut egui::Ui,
    state: &mut WorkbenchState,
    local: &mut LocalBook,
    title: &str,
    read_only: bool,
    single_preview: bool,
    snapshot: &Result<Arc<ManuscriptQuerySnapshot>, String>,
    root: &std::path::Path,
    can_rehearse: bool,
) -> (bool, bool) {
    let mut create = false;
    let mut rehearse = false;
    let gap = ui.spacing().item_spacing.x;
    let title_width = text_width(ui, title, egui::TextStyle::Button)
        .clamp(72.0, (ui.available_width() * 0.24).clamp(72.0, 180.0));
    let identity = if state.focus_management {
        0.0
    } else {
        title_width + gap
    };
    let mut widths = vec![
        button_width(ui, "选择章节"),
        button_width(ui, "新建章节"),
        button_width(ui, "书稿管理"),
        checkbox_width(ui, "阅读预览"),
        button_width(ui, "试演"),
    ];
    if state.reader_open && single_preview {
        widths.extend([button_width(ui, "编辑"), button_width(ui, "预览")]);
    }
    let narrow = identity + row_width(&widths, gap) > ui.available_width() - gap;
    ui.horizontal_wrapped(|ui| {
        ui.push_id("manuscript-title-slot", |ui| {
            if !state.focus_management {
                ui.add_sized(
                    [title_width, ui.spacing().interact_size.y],
                    egui::Label::new(egui::RichText::new(title).strong()).truncate(),
                )
                .on_hover_text(title);
            }
        });
        let chapters = ui
            .push_id("manuscript-chapter-menu", |ui| {
                egui::containers::menu::MenuButton::new("选择章节")
                    .config(
                        egui::containers::menu::MenuConfig::new()
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside),
                    )
                    .ui(ui, |ui| {
                        ui.set_max_width((ui.ctx().screen_rect().width() - 32.0).min(720.0));
                        ui.set_max_height((ui.ctx().screen_rect().height() - 80.0).max(160.0));
                        super::outline::draw(
                            ui,
                            state.layout,
                            local,
                            snapshot,
                            &mut state.navigation,
                            root,
                            true,
                        );
                        if state.navigation.enter_editor {
                            state.narrow_preview = false;
                            state.writing_view.focus_existing_editor();
                            ui.close();
                        }
                    })
            })
            .inner;
        record(ui, "manuscript/chapters", &chapters.0);
        if narrow {
            let tools = ui.menu_button("书稿工具", |ui| {
                ui.set_max_width((ui.ctx().screen_rect().width() - 32.0).min(360.0));
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        create = crate::theme::add_enabled_ui(
                            ui,
                            !state.navigation.input.blocked(),
                            |ui| tools(ui, state, read_only, single_preview),
                        )
                        .inner;
                        ui.separator();
                        rehearse = rehearsal_button(ui, can_rehearse);
                    });
            });
            record(ui, "manuscript/tools", &tools.response);
        } else {
            create = crate::theme::add_enabled_ui(ui, !state.navigation.input.blocked(), |ui| {
                tools(ui, state, read_only, single_preview)
            })
            .inner;
            ui.menu_button("试演", |ui| {
                rehearse = rehearsal_button(ui, can_rehearse);
            });
        }
    });
    (create, rehearse)
}

fn rehearsal_button(ui: &mut egui::Ui, enabled: bool) -> bool {
    let clicked = theme::add_enabled(ui, enabled, egui::Button::new("试演当前正文草稿…"))
        .on_hover_text("明确核对范围后独立运行；不应用、保存或替换普通试玩")
        .clicked();
    if clicked {
        ui.close();
    }
    clicked
}

fn tools(
    ui: &mut egui::Ui,
    state: &mut WorkbenchState,
    read_only: bool,
    single_preview: bool,
) -> bool {
    let create = crate::theme::add_enabled(ui, !read_only, theme::primary("新建章节"));
    record(ui, "manuscript/create-chapter", &create);
    let management = ui.toggle_value(&mut state.focus_management, "书稿管理");
    record(ui, "manuscript/management", &management);
    let reader = ui.checkbox(&mut state.reader_open, "阅读预览");
    record(ui, "manuscript/reader", &reader);
    if state.reader_open && single_preview {
        let before = state.narrow_preview;
        let edit = ui.selectable_value(&mut state.narrow_preview, false, "编辑");
        record(ui, "manuscript/edit", &edit);
        let preview = ui.selectable_value(&mut state.narrow_preview, true, "预览");
        record(ui, "manuscript/preview", &preview);
        if before != state.narrow_preview {
            state.review_focus = state.narrow_preview;
            if !state.narrow_preview {
                state.writing_view.focus_existing_editor();
            }
        }
    }
    create.clicked()
}
