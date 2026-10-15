use super::*;
use crate::theme::{self, *};
use worldline_core::Severity;

mod actions;
mod layout;
mod menus;
mod save_state;
mod title;

impl WorldeditApp {
    pub(super) fn top_bar(&mut self, ctx: &egui::Context) {
        self.navigation_drawer_shortcut(ctx);
        #[cfg(test)]
        crate::chrome::action_layout::clear_trace(ctx);
        self.title_bar(ctx);
        let technical = theme::style_preset() == theme::StylePreset::Technical;
        // 矮窗先回收外层留白，保留控件密度和双编辑状态的稳定高度。
        let action_margin = if ctx.screen_rect().height() < 420.0 {
            2
        } else if technical {
            3
        } else {
            6
        };
        egui::TopBottomPanel::top("workbench-actions")
            .frame(
                egui::Frame::new()
                    .fill(theme::CHROME())
                    .inner_margin(egui::Margin::symmetric(14, action_margin)),
            )
            .show(ctx, |ui| {
                self.workbench_actions(ui);
                if technical {
                    ui.painter().hline(
                        ui.max_rect().x_range(),
                        ui.max_rect().bottom(),
                        egui::Stroke::new(1.0_f32, theme::BORDER()),
                    );
                }
            });
        if let Some(warning) = self.personal.storage_warning() {
            egui::TopBottomPanel::top("personal-storage-warning")
                .frame(theme::panel())
                .show(ctx, |ui| {
                    ui.colored_label(theme::WARNING(), warning);
                    ui.label(theme::muted(
                        "本次外观仍可使用；原设置内容已保留，没有被默认值覆盖",
                    ));
                });
        }
        if let Some(error) = self.io_error.clone() {
            egui::TopBottomPanel::top("error")
                .frame(theme::panel().fill(theme::error_background()))
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(ERROR(), format!("无法完成操作 · {error}"));
                        #[cfg(not(target_arch = "wasm32"))]
                        if ui.small_button("查看冲突差异").clicked() {
                            self.conflict_view.open_or_capture(&self.project);
                        }
                        if ui.small_button("关闭").clicked() {
                            self.io_error = None;
                        }
                    });
                });
        }
    }
    fn export_publish_menu(&mut self, ui: &mut egui::Ui) {
        let menu = crate::chrome::quiet_menu(ui, "导出与发布", |ui| {
            if ui.button("发布给读者").clicked() {
                self.open_reader_publish();
                ui.close();
            }
            ui.separator();
            if ui.button("导出工程  ↗").clicked() {
                self.directory_dialog(true);
                ui.close();
            }
            #[cfg(not(target_arch = "wasm32"))]
            if ui.button("导出 ZIP 工程包…").clicked() {
                self.export_package();
                ui.close();
            }
            #[cfg(target_arch = "wasm32")]
            if (self.browser_pending_save || self.io_error.is_some())
                && ui.button("导出恢复副本").clicked()
            {
                self.export_browser_recovery_copy();
                ui.close();
            }
        });
        crate::chrome::action_layout::record(ui, "global/export", &menu.response);
    }
    pub(super) fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(theme::CHROME())
                    .corner_radius(egui::CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: theme::shapes().window,
                        se: theme::shapes().window,
                    })
                    .inner_margin(egui::Margin::symmetric(14, 5)),
            )
            .show(ctx, |ui| {
                // 先给右侧固定操作真实空间，再把剩余宽度交给左侧状态与长回执。
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(theme::muted(format!(
                        "WORLDLINE {}  ·  UTF-8",
                        self.project.language_version()
                    )));
                    if crate::theme::add_enabled(
                        ui,
                        !self.redo.is_empty() || !self.search_state.redo.is_empty(),
                        egui::Button::new("重做").small(),
                    )
                    .clicked()
                    {
                        self.edit_undo(true);
                    }
                    if crate::theme::add_enabled(
                        ui,
                        !self.history.is_empty() || !self.search_state.undo.is_empty(),
                        egui::Button::new("撤销").small(),
                    )
                    .clicked()
                    {
                        self.edit_undo(false);
                    }
                    ui.add_space(8.0);
                    let remaining =
                        egui::vec2(ui.available_width().max(0.0), ui.spacing().interact_size.y);
                    ui.allocate_ui_with_layout(
                        remaining,
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
                            self.status_details(ui);
                        },
                    );
                });
            });
    }

    fn status_details(&mut self, ui: &mut egui::Ui) {
        let errors = self
            .diagnostics()
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count();
        let warnings = self
            .diagnostics()
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count();
        let content = if errors > 0 {
            format!("内容 {errors} 错误 · {warnings} 提醒")
        } else {
            format!("内容编译通过 · {warnings} 提醒")
        };
        let compact = ui.available_width() < 500.;
        self.problems_status(ui, &content);
        ui.separator();
        let state = self.save_state();
        ui.label(theme::muted(state.footer(compact)))
            .on_hover_text(state.label());
        if let Some(message) = &self.message {
            ui.add_sized(
                egui::vec2(ui.available_width().max(0.0), ui.spacing().interact_size.y),
                egui::Label::new(theme::muted(message))
                    .truncate()
                    .show_tooltip_when_elided(false),
            )
            .on_hover_text(message);
        }
    }
    pub(super) fn page_heading(&self, ui: &mut egui::Ui, title: &str, subtitle: &str) {
        theme::page_heading(ui, title, subtitle);
    }
}

#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod tests;
