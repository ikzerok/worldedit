use crate::theme::{bounded, AppearancePreferences};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub(in crate::app) struct Settings {
    pub appearance: AppearancePreferences,
    pub navigation: bool,
    pub diagnostics: bool,
    pub source_wrap: bool,
    pub focus: bool,
    pub dock_references: bool,
    pub references_visible: bool,
    pub navigation_width: f32,
    pub reference_width: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            appearance: AppearancePreferences::default(),
            navigation: true,
            diagnostics: false,
            source_wrap: false,
            focus: false,
            dock_references: true,
            references_visible: true,
            navigation_width: 212.0,
            reference_width: 330.0,
        }
    }
}
impl Settings {
    pub(super) fn normalize(&mut self) {
        self.appearance.normalize();
        self.navigation_width = bounded(self.navigation_width, 212.0, 180.0, 320.0);
        self.reference_width = bounded(self.reference_width, 330.0, 260.0, 440.0);
    }
}
// Source compatibility for callers editing committed preferences. Rendering must use
// PersonalState::appearance so a preview can never masquerade as committed state.
impl std::ops::Deref for Settings {
    type Target = AppearancePreferences;
    fn deref(&self) -> &Self::Target {
        &self.appearance
    }
}
impl std::ops::DerefMut for Settings {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.appearance
    }
}
