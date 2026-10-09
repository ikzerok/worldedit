//! Reveal only this keyboard navigation, never continuously follow a stale focus.
const FRAME_ID: &str = "localization-keyboard-focus-frame";

#[derive(Clone, Copy)]
struct Navigation {
    frame: u64,
    from: Option<egui::Id>,
    requested: Option<egui::Id>,
}

pub(super) fn begin(ctx: &egui::Context, compact: bool) {
    let id = egui::Id::new(FRAME_ID);
    let frame = ctx.cumulative_frame_nr();
    let (tab, newer_intent) = ctx.input(|input| {
        let tab = input.events.iter().any(|event| matches!(event,
            egui::Event::Key { key: egui::Key::Tab, pressed: true, .. }));
        let newer = input.events.iter().any(|event| {
            matches!(event,
                egui::Event::PointerButton { .. } | egui::Event::MouseWheel { .. }
                | egui::Event::Touch { .. } | egui::Event::Ime(_) | egui::Event::Text(_)
                | egui::Event::Paste(_) | egui::Event::WindowFocused(false))
                || matches!(event, egui::Event::Key { key, pressed: true, .. } if *key != egui::Key::Tab)
        });
        (tab, newer)
    });
    if !compact || newer_intent {
        ctx.data_mut(|data| data.remove::<Navigation>(id));
    } else if tab {
        let from = ctx.memory(|memory| memory.focused());
        ctx.data_mut(|data| {
            // An extra layout pass must not replace the original navigation origin.
            if !data
                .get_temp::<Navigation>(id)
                .is_some_and(|last| last.frame == frame)
            {
                data.insert_temp(
                    id,
                    Navigation {
                        frame,
                        from,
                        requested: None,
                    },
                );
            }
        });
    }
}

pub(super) trait RevealFocus {
    fn reveal_focus(self, ui: &egui::Ui) -> Self;
}

impl RevealFocus for egui::Response {
    fn reveal_focus(self, ui: &egui::Ui) -> Self {
        let frame = ui.ctx().cumulative_frame_nr();
        let Some(mut navigation) = ui
            .ctx()
            .data(|data| data.get_temp::<Navigation>(egui::Id::new(FRAME_ID)))
        else {
            return self;
        };
        let reveal = navigation.frame <= frame
            && frame - navigation.frame <= 1
            && navigation.from != Some(self.id)
            && navigation.requested != Some(self.id);
        if reveal && self.has_focus() && !ui.clip_rect().contains_rect(self.rect) {
            // Shift+Tab may deliver focus on the following frame, after gained_focus is false.
            // Tab indentation keeps the same field and its own caret-sized scroll request.
            // ScrollArea applies the target after constructing its next content rect. Repeating
            // that rect's delta on the next frame would add the same scroll twice and overshoot.
            navigation.requested = Some(self.id);
            ui.ctx()
                .data_mut(|data| data.insert_temp(egui::Id::new(FRAME_ID), navigation));
            ui.scroll_to_rect_animation(self.rect, None, egui::style::ScrollAnimation::none());
        }
        self
    }
}

pub(super) fn reveal_opening_combo(ui: &egui::Ui, response: &egui::Response) {
    let id = response.id.with("localization-opening-frame");
    let frame = ui.ctx().cumulative_frame_nr();
    let open = egui::ComboBox::is_open(ui.ctx(), response.id);
    let newer_intent = ui.input(|input| input.events.iter().any(|event| {
        matches!(event, egui::Event::PointerButton { .. } | egui::Event::MouseWheel { .. }
            | egui::Event::Touch { .. } | egui::Event::Ime(_) | egui::Event::Text(_)
            | egui::Event::Paste(_) | egui::Event::WindowFocused(false))
            || matches!(event, egui::Event::Key { key, pressed: true, .. } if *key != egui::Key::Tab)
    }));
    if open && response.clicked() {
        ui.ctx().data_mut(|data| data.insert_temp(id, frame));
    } else if !open || newer_intent {
        ui.ctx().data_mut(|data| data.remove::<u64>(id));
    }
    let recent_open = ui
        .ctx()
        .data(|data| data.get_temp::<u64>(id))
        .is_some_and(|opened| opened <= frame && frame - opened <= 1);
    if recent_open {
        // A coalesced move+press can leave parent drag momentum. An explicit target also
        // clears that momentum on the next pass; neither ongoing dragging nor touch is disabled.
        ui.scroll_to_rect_animation(
            response.rect,
            Some(egui::Align::Center),
            egui::style::ScrollAnimation::none(),
        );
    }
}
