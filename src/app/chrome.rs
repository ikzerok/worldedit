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
                            .color(TEXT),
                    );
                    ui.add_space(12.0);
                    crate::chrome::subtitle(ui, "世界创作工作台");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("发布给读者").clicked() {
                            self.open_reader_publish();
                        }
                        #[cfg(target_arch = "wasm32")]
                        if (self.browser_pending_save || self.io_error.is_some())
                            && ui.button("导出恢复副本").clicked()
                        {
                            self.export_browser_recovery_copy();
                        }
                        if ui.add(theme::primary("导出工程  ↗")).clicked() {
                            self.directory_dialog(true);
                        }
                        if ui.button("保存全部").clicked() {
                            self.save();
                        }
                        if ui
                            .button("搜索")
                            .on_hover_text("搜索所有文件 · Ctrl+Shift+F")
                            .clicked()
                        {
                            self.search_open = true;
                            self.search_focus = true;
                        }
                        if ui.button("▶ 试玩").clicked() {
                            self.tab = Tab::Play;
                        }
                        ui.menu_button("工程", |ui| {
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
                            #[cfg(not(target_arch = "wasm32"))]
                            if ui.button("导出 ZIP 工程包…").clicked() {
                                ui.close();
                                self.export_package();
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
                    });
                });
            });
        if let Some(error) = self.io_error.clone() {
            egui::TopBottomPanel::top("error")
                .frame(theme::panel().fill(egui::Color32::from_rgb(65, 36, 44)))
                .show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.colored_label(ERROR, error);
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
    pub(super) fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .corner_radius(egui::CornerRadius {
                        nw: 0,
                        ne: 0,
                        sw: 12,
                        se: 12,
                    })
                    .inner_margin(egui::Margin::symmetric(18, 8)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
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
                    ui.colored_label(
                        if errors > 0 { ERROR } else { ACCENT },
                        if errors > 0 {
                            format!("● {errors} 个错误")
                        } else {
                            "● 编译通过".into()
                        },
                    );
                    if warnings > 0 {
                        ui.label(theme::muted(format!("{warnings} 个提醒")));
                    }
                    ui.separator();
                    ui.label(theme::muted(if self.project.is_dirty() {
                        "有未保存修改"
                    } else {
                        "全部文件已保存"
                    }));
                    if let Some(message) = &self.message {
                        ui.label(theme::muted(message));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(theme::muted(format!(
                            "WORLDLINE {}  ·  UTF-8",
                            self.project.language_version()
                        )));
                        if ui
                            .add_enabled(!self.redo.is_empty(), egui::Button::new("重做").small())
                            .clicked()
                        {
                            self.undo(true);
                        }
                        if ui
                            .add_enabled(
                                !self.history.is_empty(),
                                egui::Button::new("撤销").small(),
                            )
                            .clicked()
                        {
                            self.undo(false);
                        }
                    });
                });
            });
    }
    pub(super) fn page_heading(&self, ui: &mut egui::Ui, title: &str, subtitle: &str) {
        ui.heading(title);
        ui.label(theme::muted(subtitle));
        ui.add_space(12.0);
    }
}
