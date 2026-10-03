use super::*;
use crate::theme::{self, *};
use worldline_core::Severity;

impl WorldeditApp {
    pub(super) fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top")
            .frame(crate::chrome::title_frame(ctx))
            .show(ctx, |ui| {
                crate::chrome::title_drag(ui);
                ui.horizontal(|ui| {
                    let close = ui.horizontal(crate::chrome::controls).inner;
                    if close {
                        self.request_action(Pending::Close, ctx);
                    }
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new("worldedit")
                            .strong()
                            .size(18.0)
                            .color(TEXT()),
                    );
                    ui.add_space(12.0);
                    if ui.available_width() > 950.0 {
                        crate::chrome::subtitle(ui, "世界创作工作台");
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        self.edit_menu(ui);
                        self.workspace_view_menu(ui);
                        self.compact_navigation_menu(ui);
                        ui.menu_button("工程", |ui| {
                            if ui.button("语言与资料能力…").clicked() {
                                self.open_capabilities();
                                ui.close();
                            }
                            if ui.button("管理工程模板").clicked() {
                                self.tab = Tab::Templates;
                                ui.close();
                            }
                            if ui.button("本地化工作台").clicked() {
                                self.tab = Tab::Localization;
                                ui.close();
                            }
                            if ui.button("检查点历史…").clicked() {
                                self.tab = Tab::CheckpointHistory;
                                ui.close();
                            }
                            if ui.button("新建世界").clicked() {
                                self.request_action(Pending::New, ctx);
                                ui.close();
                            }
                            if ui.button("打开文件夹…").clicked() {
                                self.open_dialog(ctx, true);
                                ui.close();
                            }
                            if ui.button("选择工作区…  Ctrl+O").clicked() {
                                self.open_dialog(ctx, false);
                                ui.close();
                            }
                            if ui.button("另存工程…").clicked() {
                                self.directory_dialog(false);
                                ui.close();
                            }
                            if ui.button("导入 Markdown…").clicked() {
                                ui.close();
                                self.markdown_import_wizard =
                                    Some(markdown_import_ui::Wizard::default());
                            }
                            if ui.button("从磁盘重新载入").clicked() {
                                self.request_action(Pending::Open(self.project.entry.clone()), ctx);
                                ui.close();
                            }
                            #[cfg(not(target_arch = "wasm32"))]
                            if ui.button("查看冲突差异").clicked() {
                                self.conflict_view =
                                    conflicts::ConflictView::capture(&self.project);
                                ui.close();
                            }
                        });
                        self.export_publish_menu(ui);
                        if ui
                            .add(theme::primary("保存全部"))
                            .on_hover_text("Ctrl+S · 保存工程中的全部修改")
                            .clicked()
                        {
                            self.save();
                        }
                        if ui
                            .button("搜索")
                            .on_hover_text("搜索所有文件 · Ctrl+Shift+F")
                            .clicked()
                        {
                            self.open_search(ctx, true, false);
                        }
                        if ui.button("▶ 试玩").clicked() {
                            self.tab = Tab::Play;
                        }
                    });
                });
            });
        if let Some(error) = self.io_error.clone() {
            egui::TopBottomPanel::top("error")
                .frame(theme::panel().fill(theme::error_background()))
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(ERROR(), error);
                        #[cfg(not(target_arch = "wasm32"))]
                        if ui.small_button("查看冲突差异").clicked() {
                            self.conflict_view = conflicts::ConflictView::capture(&self.project);
                        }
                        if ui.small_button("关闭").clicked() {
                            self.io_error = None;
                        }
                    });
                });
        }
    }
    fn export_publish_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("导出与发布", |ui| {
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
    }
    pub(super) fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(BG())
                    .corner_radius(egui::CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: 12,
                        se: 12,
                    })
                    .inner_margin(egui::Margin::symmetric(18, 8)),
            )
            .show(ctx, |ui| {
                // 先给右侧固定操作真实空间，再把剩余宽度交给左侧状态与长回执。
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(theme::muted(format!(
                        "WORLDLINE {}  ·  UTF-8",
                        self.project.language_version()
                    )));
                    if ui
                        .add_enabled(
                            !self.redo.is_empty() || !self.search_state.redo.is_empty(),
                            egui::Button::new("重做").small(),
                        )
                        .clicked()
                    {
                        self.edit_undo(true);
                    }
                    if ui
                        .add_enabled(
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
        if ui
            .small_button(if errors > 0 {
                format!("内容 {errors} 错误 · {warnings} 提醒")
            } else {
                format!("内容编译通过 · {warnings} 提醒")
            })
            .clicked()
        {
            self.open_problems(ui.ctx());
        }
        self.problems_status(ui);
        ui.separator();
        ui.label(theme::muted(if self.has_open_authoring_form() {
            "有未应用输入（尚未保存）"
        } else if self.project.is_dirty() {
            "有未保存修改"
        } else {
            "全部文件已保存"
        }));
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
mod tests;
