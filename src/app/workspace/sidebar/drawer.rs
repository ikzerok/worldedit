//! 复用完整导航和文件能力，临时展开不覆盖持久布局偏好。
use crate::app::WorldeditApp;

fn drawer_id() -> egui::Id {
    egui::Id::new("workbench-navigation-drawer")
}

impl WorldeditApp {
    pub(in crate::app) fn auxiliary_ime_active(&self, ctx: &egui::Context) -> bool {
        self.ime_composing
            || self.command_palette.ime
            || self.command_palette.ime_frame
            || ctx.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Ime(_)))
            })
    }

    pub(in crate::app) fn navigation_drawer_open(&self, ctx: &egui::Context) -> bool {
        ctx.data(|data| data.get_temp::<bool>(drawer_id())) == Some(true)
    }

    pub(in crate::app) fn open_navigation_drawer(&mut self, ctx: &egui::Context) {
        if self.auxiliary_ime_active(ctx) {
            self.message = Some("请先完成输入法组合，再打开导航；当前输入已保留".into());
            return;
        }
        ctx.data_mut(|data| data.insert_temp(drawer_id(), true));
        self.sync_edit_layers(ctx);
        // 先让取消层记录真实返回目标，再让幕后 TextEdit 失去键盘所有权。
        if let Some(id) = ctx.memory(|memory| memory.focused()) {
            ctx.memory_mut(|memory| memory.surrender_focus(id));
        }
        ctx.data_mut(|data| data.insert_temp(drawer_id().with("focus"), true));
    }

    pub(in crate::app) fn close_navigation_drawer(&mut self, ctx: &egui::Context) {
        if self.auxiliary_ime_active(ctx) {
            return;
        }
        ctx.data_mut(|data| {
            data.remove::<bool>(drawer_id());
            data.remove::<bool>(drawer_id().with("focus"));
        });
    }

    pub(in crate::app) fn navigation_drawer_shortcut(&mut self, ctx: &egui::Context) {
        if self.auxiliary_ime_active(ctx) {
            return;
        }
        if ctx.input_mut(|input| {
            input.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::E,
            )
        }) {
            if self.navigation_drawer_open(ctx) {
                self.close_navigation_drawer(ctx);
            } else {
                self.open_navigation_drawer(ctx);
            }
        }
    }

    pub(in crate::app) fn navigation_drawer_window(&mut self, ctx: &egui::Context) {
        if !self.navigation_drawer_open(ctx) {
            return;
        }
        self.sync_edit_layers(ctx);
        let screen = ctx.screen_rect();
        let width = (screen.width() - 32.0).clamp(220.0, 340.0);
        let mut open = true;
        let mut close = false;
        let previous = (self.tab, self.active_file.clone());
        let input_ready = !self.auxiliary_ime_active(ctx);
        egui::Window::new("导航与文件")
            .id(drawer_id().with("window"))
            .open(&mut open)
            .collapsible(false)
            .default_width(width)
            .max_width(width)
            .max_height((screen.height() - 48.0).max(140.0))
            .constrain_to(screen.shrink(8.0))
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                let response =
                    crate::theme::add_enabled(ui, input_ready, egui::Button::new("返回正文"));
                if response.enabled()
                    && ui.is_rect_visible(response.rect)
                    && ctx.data(|data| data.get_temp::<bool>(drawer_id().with("focus")))
                        == Some(true)
                {
                    response.request_focus();
                    ctx.data_mut(|data| data.remove::<bool>(drawer_id().with("focus")));
                }
                close = response.clicked();
                ui.label(crate::theme::muted("Esc 或关闭会保留布局、文件和当前输入"));
                ui.separator();
                crate::theme::add_enabled_ui(ui, input_ready, |ui| self.sidebar_content(ui));
            });
        if !open || close || previous != (self.tab, self.active_file.clone()) {
            self.close_navigation_drawer(ctx);
        }
    }
}
