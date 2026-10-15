//! 无工程的可用工作台：只开放工作区入口与设备外观，不展示临时工程。
use super::{WorkspaceOperation, WorldeditApp};
use crate::app::Pending;
use crate::theme;

impl WorldeditApp {
    pub(in crate::app) fn welcome_frame(&mut self, ctx: &egui::Context) {
        let composing = self.workspace_input_composing(ctx);
        if ctx.input(|input| input.viewport().close_requested()) && !self.allow_close {
            self.request_action(Pending::Close, ctx);
        }
        if self.native_startup.dialog.is_none() && !self.personal.preferences_open && !composing {
            if ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
                self.begin_workspace_dialog(WorkspaceOperation::Open);
            } else if ctx
                .input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::N))
            {
                self.request_action(Pending::New, ctx);
            }
        }
        egui::TopBottomPanel::top("welcome-title")
            .frame(crate::chrome::title_frame(ctx))
            .show(ctx, |ui| {
                crate::chrome::title_drag(ui);
                ui.horizontal(|ui| {
                    if ui.horizontal(crate::chrome::controls).inner {
                        self.request_action(Pending::Close, ctx);
                    }
                    ui.strong("worldedit");
                    ui.label(theme::muted(format!("v{}", env!("CARGO_PKG_VERSION"))));
                });
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("welcome-content")
                .show(ui, |ui| {
                    ui.add_space((ui.available_height() * 0.1).clamp(12.0, 72.0));
                    ui.vertical_centered(|ui| {
                        ui.heading("欢迎使用 worldedit");
                        ui.add_space(8.0);
                        ui.label(theme::muted("从空白开始写作，或继续已有作品"));
                        ui.add_space(24.0);
                        if ui
                            .add_sized([220.0, 44.0], theme::primary("新建作品"))
                            .clicked()
                        {
                            self.request_action(Pending::New, ctx);
                        }
                        ui.add_space(8.0);
                        if ui
                            .add_sized([220.0, 40.0], egui::Button::new("打开工程"))
                            .clicked()
                        {
                            self.begin_workspace_dialog(WorkspaceOperation::Open);
                        }
                        if let Some(path) = self.personal.last_project() {
                            ui.add_space(12.0);
                            if ui
                                .button("重试上次工程")
                                .on_hover_text(path.display().to_string())
                                .clicked()
                            {
                                self.request_action(Pending::Open(path), ctx);
                            }
                        }
                        ui.add_space(16.0);
                        ui.label(theme::muted("尚未打开工程；新建或打开前不会写入作品文件"));
                        ui.add_space(16.0);
                        if ui.button("外观设置").clicked() {
                            self.personal.preferences_open = true;
                        }
                        if ui.button("退出").clicked() {
                            self.request_action(Pending::Close, ctx);
                        }
                    });
                    if let Some(warning) = self.personal.storage_warning() {
                        ui.add_space(16.0);
                        ui.colored_label(theme::WARNING(), warning);
                    }
                    if let Some(error) = self.io_error.clone() {
                        ui.add_space(16.0);
                        egui::Frame::new()
                            .fill(theme::error_background())
                            .inner_margin(12.0)
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new("无法完成操作")
                                        .color(theme::ERROR())
                                        .strong(),
                                );
                                ui.add(egui::Label::new(error).wrap());
                                ui.label("可以重试打开，或选择其他位置新建作品。");
                                if ui.small_button("关闭提示").clicked() {
                                    self.io_error = None;
                                }
                            });
                    }
                });
        });
        self.workspace_dialog(ctx);
        self.preferences_window(ctx);
        crate::chrome::resize_edges(ctx);
        self.sync_native_window_title(ctx);
    }
}
