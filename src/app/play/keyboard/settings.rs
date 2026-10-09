//! Ordinary settings follow real keyboard focus; they never request focus or activate a story.
use super::PlayKeyboard;

#[derive(Clone, Copy)]
pub(in crate::app::play) enum SettingsHost {
    Ordinary,
    Comparison,
}

#[derive(Clone, Copy)]
struct HostReveal {
    context: Option<egui::Id>,
    source: egui::Id,
    frame: u64,
}

impl PlayKeyboard {
    pub(in crate::app::play) fn setting(
        &self,
        ui: &mut egui::Ui,
        role: &str,
        add: impl FnOnce(&mut egui::Ui) -> egui::Response,
    ) -> egui::Response {
        self.setting_in(ui, SettingsHost::Ordinary, role, add)
    }

    pub(in crate::app::play) fn setting_in(
        &self,
        ui: &mut egui::Ui,
        host: SettingsHost,
        role: &str,
        add: impl FnOnce(&mut egui::Ui) -> egui::Response,
    ) -> egui::Response {
        // Isolated renderers may not have prepared a project context; this is not a host test.
        if self.prepared != Some(ui.ctx().cumulative_frame_nr()) {
            return add(ui);
        }
        // play_tab draws shared mode/locale controls in only one host per frame, preserving focus.
        let id = egui::Id::new(("ordinary-play-setting", self.context, role));
        // A semantic child avoids auto-ID changes when locale, notices or budget rows appear.
        let mut builder = egui::UiBuilder::new()
            .layer_id(ui.layer_id())
            .max_rect(ui.available_rect_before_wrap())
            .layout(*ui.layout())
            .style(ui.style().clone());
        if !ui.is_enabled() {
            builder = builder.disabled();
        }
        if !ui.is_visible() {
            builder = builder.invisible();
        }
        let mut child = egui::Ui::new(ui.ctx().clone(), id, builder);
        child.set_clip_rect(ui.clip_rect());
        let response = add(&mut child);
        ui.advance_cursor_after_rect(child.min_rect());
        if matches!(host, SettingsHost::Ordinary) {
            self.reveal_setting(ui, &response);
        }
        response
    }

    pub(in crate::app::play) fn settings_host_focus(
        &self,
        ui: &egui::Ui,
        host: SettingsHost,
        response: &egui::Response,
    ) {
        let key = egui::Id::new("ordinary-play-settings-host-reveal");
        let frame = ui.ctx().cumulative_frame_nr();
        let blocked = self.prepared != Some(frame)
            || self.ime
            || self.rehearsal_preparation
            || ui.input(|input| {
                !input.focused
                    || input.events.iter().any(|event| match event {
                        egui::Event::PointerMoved(_)
                        | egui::Event::PointerButton { .. }
                        | egui::Event::MouseWheel { .. }
                        | egui::Event::Touch { .. }
                        | egui::Event::Ime(_)
                        | egui::Event::Text(_) => true,
                        egui::Event::Key {
                            key,
                            pressed: true,
                            modifiers,
                            ..
                        } => {
                            !matches!(key, egui::Key::Enter | egui::Key::Space)
                                || !modifiers.is_none()
                        }
                        _ => false,
                    })
            });
        match host {
            SettingsHost::Comparison => {
                let fresh_confirm = ui.input(|input| {
                    input.events.iter().any(|event| {
                        matches!(event, egui::Event::Key {
                        key: egui::Key::Enter | egui::Key::Space,
                        pressed: true, repeat: false, modifiers, ..
                    } if modifiers.is_none())
                    })
                });
                if !blocked && fresh_confirm && response.clicked() && response.has_focus() {
                    ui.ctx().data_mut(|data| {
                        data.insert_temp(
                            key,
                            HostReveal {
                                context: self.context,
                                source: response.id,
                                frame,
                            },
                        )
                    });
                }
            }
            SettingsHost::Ordinary => {
                let pending = ui.ctx().data_mut(|data| {
                    let pending = data.get_temp::<HostReveal>(key);
                    data.remove::<HostReveal>(key);
                    pending
                });
                if !blocked
                    && pending.is_some_and(|pending| {
                        pending.context == self.context
                            && pending.source == response.id
                            && pending.frame <= frame
                            && frame - pending.frame <= 1
                    })
                    && response.enabled()
                    && response.has_focus()
                    && !ui.clip_rect().contains_rect(response.rect)
                {
                    // Retain the existing focus; this consumes only the explicit host transition.
                    response.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
                }
            }
        }
    }

    pub(in crate::app::play) fn reveal_setting(&self, ui: &egui::Ui, response: &egui::Response) {
        let frame = ui.ctx().cumulative_frame_nr();
        let pointer_or_ime = ui.input(|input| {
            !input.focused
                || input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::PointerButton { .. }
                            | egui::Event::MouseWheel { .. }
                            | egui::Event::Touch { .. }
                            | egui::Event::Ime(_)
                    )
                })
        });
        if self.prepared == Some(frame)
            && self
                .tab_frame
                .is_some_and(|last| last <= frame && frame - last <= 1)
            && !self.ime
            && !self.rehearsal_preparation
            && !pointer_or_ime
            && response.enabled()
            && response.gained_focus()
            && !ui.clip_rect().contains_rect(response.rect)
        {
            response.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
        }
    }
}
