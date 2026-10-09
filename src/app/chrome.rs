use super::*;
use crate::theme::{self, *};
use worldline_core::Severity;

mod menus;

impl WorldeditApp {
    pub(super) fn top_bar(&mut self, ctx: &egui::Context) {
        self.navigation_drawer_shortcut(ctx);
        egui::TopBottomPanel::top("top")
            .frame(crate::chrome::title_frame(ctx))
            .show(ctx, |ui| {
                crate::chrome::title_drag(ui);
                ui.horizontal(|ui| {
                    if ui.horizontal(crate::chrome::controls).inner {
                        self.request_action(Pending::Close, ctx);
                    }
                    ui.label(egui::RichText::new("worldedit").size(13.0).color(MUTED()));
                    ui.add_space(10.0);
                    let search_width = if ui.available_width() > 620.0 {
                        230.0
                    } else {
                        94.0
                    };
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_sized(
                                [search_width, theme::metrics().control_height],
                                egui::Button::new(if search_width > 100.0 {
                                    "查找对象 / 命令    Ctrl/Cmd+P"
                                } else {
                                    "查找对象"
                                })
                                .fill(theme::DOCUMENT())
                                .stroke(egui::Stroke::new(1.0_f32, theme::CONTROL_BORDER())),
                            )
                            .on_hover_text(
                                "按名称、类型、别名或来源寻找对象；任务命令可在面板内切换",
                            )
                            .clicked()
                        {
                            self.open_commands(ctx, false);
                        }
                        let available = egui::vec2(
                            ui.available_width().max(0.0),
                            theme::metrics().control_height,
                        );
                        ui.allocate_ui_with_layout(
                            available,
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_clip_rect(ui.clip_rect().intersect(ui.max_rect()));
                                let title = self
                                    .snapshot
                                    .as_ref()
                                    .and_then(|snapshot| snapshot.result.analysis.world.as_ref())
                                    .map(|world| world.display.clone())
                                    .or_else(|| {
                                        self.project
                                            .root
                                            .file_name()
                                            .map(|name| name.to_string_lossy().into_owned())
                                    })
                                    .unwrap_or_else(|| "未命名作品".into());
                                let dirty =
                                    self.has_open_authoring_form() || self.project.is_dirty();
                                let caption =
                                    format!("{}{}", if dirty { "●  " } else { "" }, title);
                                ui.add_sized(
                                    [
                                        ui.available_width().max(0.0),
                                        theme::metrics().control_height,
                                    ],
                                    egui::Label::new(
                                        egui::RichText::new(caption).strong().size(16.0),
                                    )
                                    .truncate(),
                                )
                                .on_hover_text(format!(
                                    "{}\n{}",
                                    title,
                                    self.project.root.display()
                                ));
                            },
                        );
                    });
                });
            });
        let technical = theme::style_preset() == theme::StylePreset::Technical;
        egui::TopBottomPanel::top("workbench-actions")
            .frame(
                egui::Frame::new()
                    .fill(theme::CHROME())
                    .inner_margin(egui::Margin::symmetric(14, if technical { 3 } else { 6 })),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(7.0, 5.0);
                    self.compact_navigation_menu(ui);
                    if ui
                        .add(theme::primary("保存全部"))
                        .on_hover_text("Ctrl/Cmd+S · 保存工程中全部已应用修改；不自动应用草稿")
                        .clicked()
                    {
                        self.save();
                    }
                    if crate::chrome::quiet_button(ui, "▶ 试玩").clicked() {
                        self.switch_tab(Tab::Play);
                    }
                    if crate::chrome::quiet_button(ui, "搜索")
                        .on_hover_text("查找正文与源文件 · Ctrl/Cmd+Shift+F")
                        .clicked()
                    {
                        self.open_search(ctx, true, false);
                    }
                    ui.separator();
                    self.export_publish_menu(ui);
                    self.project_menu(ui);
                    self.edit_menu(ui);
                    self.workspace_view_menu(ui);
                    if crate::chrome::quiet_button(ui, "外观")
                        .on_hover_text("配色、工作台风格、密度与文字；仅此设备")
                        .clicked()
                    {
                        self.personal.preferences_open = true;
                    }
                });
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
        crate::chrome::quiet_menu(ui, "导出与发布", |ui| {
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
        let (short, full) = if self.has_open_authoring_form() {
            ("未应用", "有未应用输入（尚未保存）")
        } else if self.project.is_dirty() {
            ("未保存", "有未保存修改")
        } else {
            ("已保存", "全部文件已保存")
        };
        ui.label(theme::muted(if compact { short } else { full }))
            .on_hover_text(full);
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
