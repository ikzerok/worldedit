//! 窄有效视口临时收起固定参考，不改变用户的停靠、宽度或对象选择。
use crate::app::WorldeditApp;
use crate::theme;

fn drawer_id() -> egui::Id {
    egui::Id::new("compact-reference-drawer")
}

impl WorldeditApp {
    pub(in crate::app) fn compact_reference_mode(&self, ctx: &egui::Context) -> bool {
        self.personal.settings.references_visible
            && (self.personal.settings.dock_references || self.focus_style())
            && !self.reading_panels.ids().is_empty()
            && (self.focus_style()
                || ctx.screen_rect().width() - self.personal.settings.reference_width < 450.0)
    }

    pub(in crate::app) fn compact_reference_open(&self, ctx: &egui::Context) -> bool {
        self.compact_reference_mode(ctx)
            && ctx.data(|data| data.get_temp::<bool>(drawer_id())) == Some(true)
    }

    pub(in crate::app) fn close_compact_reference(&mut self, ctx: &egui::Context) {
        if self.auxiliary_ime_active(ctx) {
            return;
        }
        ctx.data_mut(|data| {
            data.remove::<bool>(drawer_id());
            data.remove::<bool>(drawer_id().with("focus"));
        });
    }

    pub(super) fn compact_reference_bar(&mut self, ctx: &egui::Context) {
        let input_ready = !self.auxiliary_ime_active(ctx);
        egui::TopBottomPanel::top("compact-reference-bar")
            .frame(theme::chrome().inner_margin(egui::Margin::symmetric(14, 4)))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(theme::muted("固定参考"));
                    for (index, id) in self.reading_panels.ids().into_iter().enumerate() {
                        if crate::theme::add_enabled(
                            ui,
                            input_ready,
                            egui::Button::new(format!("参考 {}", index + 1)),
                        )
                        .on_hover_text("临时展开固定参考；关闭窗口会保留对象和原停靠选择")
                        .clicked()
                        {
                            self.selected_reading_panel = Some(id);
                            ctx.data_mut(|data| {
                                data.insert_temp(drawer_id(), true);
                                data.insert_temp(drawer_id().with("focus"), true);
                            });
                            self.sync_edit_layers(ctx);
                            if let Some(id) = ctx.memory(|memory| memory.focused()) {
                                ctx.memory_mut(|memory| memory.surrender_focus(id));
                            }
                        }
                    }
                    if crate::theme::add_enabled(
                        ui,
                        input_ready,
                        egui::Button::new("收起参考").small(),
                    )
                    .clicked()
                    {
                        self.personal.settings.references_visible = false;
                        self.close_compact_reference(ctx);
                    }
                });
            });
    }

    pub(super) fn compact_reference_window(&mut self, ctx: &egui::Context) {
        if !self.compact_reference_mode(ctx) {
            self.close_compact_reference(ctx);
            return;
        }
        if !self.compact_reference_open(ctx) {
            return;
        }
        self.sync_edit_layers(ctx);
        let screen = ctx.screen_rect();
        let width = (screen.width() - 24.0).clamp(220.0, 460.0);
        let mut open = true;
        let mut close = false;
        let input_ready = !self.auxiliary_ime_active(ctx);
        egui::Window::new("固定参考 · 临时展开")
            .id(drawer_id().with("window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(width)
            .max_width(width)
            .max_height((screen.height() - 32.0).max(140.0))
            .constrain_to(screen.shrink(8.0))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    crate::theme::add_enabled_ui(ui, input_ready, |ui| {
                        for (index, id) in self.reading_panels.ids().into_iter().enumerate() {
                            ui.selectable_value(
                                &mut self.selected_reading_panel,
                                Some(id),
                                format!("参考 {}", index + 1),
                            );
                        }
                    });
                    let response =
                        crate::theme::add_enabled(ui, input_ready, egui::Button::new("收起窗口"));
                    if response.enabled()
                        && ui.is_rect_visible(response.rect)
                        && ctx.data(|data| data.get_temp::<bool>(drawer_id().with("focus")))
                            == Some(true)
                    {
                        response.request_focus();
                        ctx.data_mut(|data| data.remove::<bool>(drawer_id().with("focus")));
                    }
                    close = response.clicked();
                });
                ui.label(theme::muted("收起窗口会保留参考和停靠设置"));
                ui.separator();
                if let Some(id) = self.selected_reading_panel {
                    self.docked_reading_content(ui, id);
                }
            });
        if !open || close || self.reading_panels.ids().is_empty() {
            self.close_compact_reference(ctx);
        }
    }
}
