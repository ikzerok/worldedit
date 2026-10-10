//! The shared object-reading window has its own bounded keyboard domain.
use super::keyboard;
use crate::app::{writing_workspace::preview_navigation as shared, WorldeditApp};

impl WorldeditApp {
    pub(in crate::app) fn close_reading_from_keyboard(&mut self, ctx: &egui::Context) {
        if self.reading_return.is_some() {
            self.return_to_source_edit(ctx);
        }
        self.close_transient_reading();
    }
    pub(super) fn transient_reading_window(&mut self, ctx: &egui::Context) {
        let Some(target) = self.reading_target.clone() else {
            keyboard::finish_closed(self, ctx);
            return;
        };
        self.sync_edit_layers(ctx);
        let top = self.edit_layer_is_top("object-reading")
            && ctx.memory(|memory| memory.top_modal_layer().is_none());
        let active = top
            && ctx.input(|input| input.focused)
            && !egui::Popup::is_any_open(ctx)
            && !self.auxiliary_ime_active(ctx);
        let mut session = keyboard::Session::begin(self, ctx);
        shared::transient_host(ctx, Some((keyboard::layer(), active)));
        session.advance(ctx, active);
        let mut open = true;
        let mut close = false;
        let mut reader = None;
        let mut content_ids = Vec::new();
        let window = egui::Window::new("Wiki · 注释索引")
            .id(egui::Id::new("object-reading"))
            // A newer explicit auxiliary layer retains its visual/input priority.
            .order(if top {
                egui::Order::Foreground
            } else {
                egui::Order::Background
            })
            .open(&mut open)
            .default_width(720.0)
            .default_height(660.0)
            .max_width((ctx.screen_rect().width() - 24.0).max(240.0))
            .max_height((ctx.screen_rect().height() - 48.0).max(160.0))
            .constrain_to(ctx.screen_rect().shrink(8.0))
            .resizable(true)
            .show(ctx, |ui| {
                if !top {
                    ui.disable();
                }
                let response = ui.add_enabled(
                    active,
                    egui::Button::new(if self.reading_return.is_some() {
                        "返回源码编辑"
                    } else {
                        "关闭资料并返回"
                    }),
                );
                session.initial(&response, active);
                shared::reveal(ui, &response, true, true);
                close = response.clicked();
                let max_height = ui.available_height().max(80.0);
                egui::ScrollArea::vertical()
                    .id_salt("object-reading-content")
                    .max_height(max_height)
                    .animated(false)
                    .show_viewport(ui, |ui, relative| {
                        let outside: Vec<_> = ctx.viewport(|v| {
                            v.this_pass
                                .widgets
                                .get_layer(ui.layer_id())
                                .map(|w| w.id)
                                .collect()
                        });
                        let viewport = shared::viewport(ui, relative).intersect(ui.clip_rect());
                        let readable = self.snapshot.as_ref().is_some_and(|snapshot| {
                            snapshot.result.analysis.catalog.object(&target).is_some()
                        });
                        let mut reader_delta = egui::Vec2::ZERO;
                        if readable {
                            let step =
                                egui::vec2(0.0, ui.text_style_height(&egui::TextStyle::Body) * 3.0);
                            let (response, delta) = shared::reading_deferred(
                                ui,
                                shared::Reader {
                                    identity: egui::Id::new("object-reading-reader"),
                                    label: "资料阅读区 · ↑↓滚动",
                                    viewport,
                                    enabled: active,
                                    horizontal: false,
                                    vertical: true,
                                    step,
                                    escape: true,
                                },
                            );
                            reader = Some(response.id);
                            reader_delta = delta;
                        }
                        if crate::theme::add_enabled(
                            ui,
                            self.reading_panels.ids().len()
                                < super::super::reading_state::PANEL_LIMIT,
                            egui::Button::new("钉住旁查"),
                        )
                        .clicked()
                        {
                            self.selected_reading_panel = self.reading_panels.pin(target.clone());
                            self.personal.settings.references_visible = true;
                            self.close_transient_reading();
                        }
                        if !self.reading_history.is_empty() && ui.button("← 返回上一词条").clicked()
                        {
                            self.reading_target = self.reading_history.pop();
                            self.alias_input.clear();
                        }
                        self.reading_content(ui, target);
                        if top {
                            keyboard::reveal_focused(ui, reader, &outside);
                        }
                        content_ids = ctx.viewport(|v| {
                            v.this_pass
                                .widgets
                                .get_layer(ui.layer_id())
                                .map(|w| w.id)
                                .filter(|id| !outside.contains(id))
                                .collect()
                        });
                        if reader_delta != egui::Vec2::ZERO {
                            ui.scroll_with_delta(reader_delta);
                        }
                    });
            });
        if top {
            if let Some(window) = window.as_ref() {
                shared::outline_shell(ctx, keyboard::layer(), window.response.rect, &content_ids);
            }
        }
        if (close || !open) && active {
            self.close_reading_from_keyboard(ctx);
        }
        session.finish(ctx, active, reader);
        if self.reading_target.is_none() {
            keyboard::finish_closed(self, ctx);
        }
    }
}
