//! 只对本窗口的新键盘导航滚入焦点；不覆盖后来的指针、输入或IME意图。
#[derive(Clone, Copy)]
pub(super) struct FocusReveal {
    enabled: bool,
}
impl FocusReveal {
    pub(super) fn for_frame(ctx: &egui::Context, blocked: bool) -> Self {
        let id = egui::Id::new("playthrough-report-focus-frame");
        let frame = ctx.cumulative_frame_nr();
        let (tab, newer_navigation) = ctx.input(|input| {
            let tab = input.events.iter().any(|event| matches!(event,
                egui::Event::Key { key: egui::Key::Tab, pressed: true, .. }));
            let other = input.events.iter().any(|event| matches!(event,
                egui::Event::PointerButton { .. } | egui::Event::MouseWheel { .. }
                | egui::Event::Touch { .. } | egui::Event::Ime(_) | egui::Event::Text(_)
                | egui::Event::Paste(_) | egui::Event::WindowFocused(false))
                || matches!(event, egui::Event::Key { key, pressed: true, .. } if *key != egui::Key::Tab));
            (tab, other)
        });
        if blocked || newer_navigation {
            ctx.data_mut(|data| data.remove::<u64>(id));
        } else if tab {
            ctx.data_mut(|data| data.insert_temp(id, frame));
        }
        let enabled = ctx
            .data(|data| data.get_temp::<u64>(id))
            .is_some_and(|requested| requested <= frame && frame - requested <= 1);
        Self { enabled }
    }

    pub(super) fn widget(
        self,
        ui: &mut egui::Ui,
        add: impl FnOnce(&mut egui::Ui) -> egui::Response,
    ) -> egui::Response {
        let response = add(ui);
        self.reveal(ui, &response, response.rect);
        response
    }

    pub(super) fn reveal(self, ui: &egui::Ui, response: &egui::Response, rect: egui::Rect) {
        if self.enabled && response.has_focus() && !ui.clip_rect().contains_rect(rect) {
            ui.scroll_to_rect_animation(rect, None, egui::style::ScrollAnimation::none());
        }
    }
}
