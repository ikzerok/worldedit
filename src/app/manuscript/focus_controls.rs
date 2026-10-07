//! Focus 只收束当前呈现；书稿选择、管理和审稿入口仍明确可达。
use super::{LocalBook, ManuscriptEntryKind, WorkbenchState};
use crate::theme;

pub(super) fn draw(
    ui: &mut egui::Ui,
    state: &mut WorkbenchState,
    local: &mut LocalBook,
    title: &str,
    read_only: bool,
    single_preview: bool,
) -> bool {
    let mut create = false;
    let narrow = ui.ctx().screen_rect().width() < 600.0 || ui.ctx().screen_rect().height() < 420.0;
    ui.horizontal_wrapped(|ui| {
        ui.add_sized(
            [
                if narrow { 72.0 } else { 180.0 },
                ui.spacing().interact_size.y,
            ],
            egui::Label::new(egui::RichText::new(title).strong()).truncate(),
        )
        .on_hover_text(title);
        ui.menu_button("选择章节", |ui| {
            ui.set_max_width((ui.ctx().screen_rect().width() - 32.0).min(360.0));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    for entry in &local.draft.entries {
                        let label = if entry.kind == ManuscriptEntryKind::Section {
                            format!("分节 · {}", entry.title)
                        } else {
                            entry.title.clone()
                        };
                        let chosen = ui
                            .push_id(("focus-chapter", &entry.id), |ui| {
                                ui.selectable_label(
                                    local.selected_entry.as_ref() == Some(&entry.id),
                                    label,
                                )
                                .on_hover_text(format!("编排 ID：{}", entry.id))
                                .clicked()
                            })
                            .inner;
                        if chosen {
                            local.selected_entry = Some(entry.id.clone());
                            state.writing_view.focus_existing_editor();
                            ui.close();
                        }
                    }
                    if local.draft.entries.is_empty() {
                        ui.label(theme::muted("尚无章节，请新建章节开始写作。"));
                    }
                });
        });
        if narrow {
            ui.menu_button("书稿工具", |ui| {
                ui.set_max_width((ui.ctx().screen_rect().width() - 32.0).min(360.0));
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        create = tools(ui, state, read_only, single_preview);
                    });
            });
        } else {
            create = tools(ui, state, read_only, single_preview);
        }
    });
    create
}

fn tools(
    ui: &mut egui::Ui,
    state: &mut WorkbenchState,
    read_only: bool,
    single_preview: bool,
) -> bool {
    let create = crate::theme::add_enabled(ui, !read_only, theme::primary("新建章节")).clicked();
    ui.toggle_value(&mut state.focus_management, "书稿管理");
    ui.checkbox(&mut state.reader_open, "阅读预览");
    if state.reader_open && single_preview {
        let before = state.narrow_preview;
        ui.selectable_value(&mut state.narrow_preview, false, "编辑");
        ui.selectable_value(&mut state.narrow_preview, true, "预览");
        if before != state.narrow_preview {
            state.review_focus = state.narrow_preview;
            if !state.narrow_preview {
                state.writing_view.focus_existing_editor();
            }
        }
    }
    create
}
