use super::PersonalState;
use crate::theme::AppearancePreferences;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PreferencesAction {
    Apply,
    Accept,
    Cancel,
    Escape,
    WindowClose,
}

impl PersonalState {
    pub(in crate::app) fn appearance(&self) -> &AppearancePreferences {
        self.appearance_draft
            .as_ref()
            .filter(|_| self.preferences_open)
            .unwrap_or(&self.settings.appearance)
    }
    pub(super) fn begin_preferences(&mut self) {
        if self.appearance_draft.is_none() {
            self.appearance_draft = Some(self.settings.appearance);
        }
        self.preferences_open = true;
    }
    pub(super) fn preferences_action(&mut self, action: PreferencesAction) {
        if matches!(action, PreferencesAction::Apply | PreferencesAction::Accept) {
            if let Some(mut draft) = self.appearance_draft {
                draft.normalize();
                self.settings.appearance = draft;
                self.appearance_draft = Some(draft);
            }
        }
        if action != PreferencesAction::Apply {
            self.appearance_draft = None;
            self.preferences_open = false;
        }
    }
    pub(super) fn reset_appearance_preview(&mut self) {
        self.appearance_draft = Some(AppearancePreferences::default());
    }
    /// Same source of truth as the UI slider; a shortcut while previewing is reversible.
    pub(in crate::app) fn appearance_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, Modifiers};
        let (reset, larger, smaller) = ctx.input_mut(|input| {
            (
                input.consume_key(Modifiers::COMMAND, Key::Num0),
                input.consume_key(Modifiers::COMMAND, Key::Plus)
                    || input.consume_key(Modifiers::COMMAND, Key::Equals),
                input.consume_key(Modifiers::COMMAND, Key::Minus),
            )
        });
        if reset || larger || smaller {
            let appearance = if self.preferences_open {
                self.appearance_draft
                    .get_or_insert(self.settings.appearance)
            } else {
                &mut self.settings.appearance
            };
            appearance.ui_scale = if reset {
                1.0
            } else {
                ((appearance.ui_scale + if larger { 0.1 } else { -0.1 }) * 100.0).round() / 100.0
            };
            appearance.normalize();
            ctx.request_repaint();
        }
    }
}
