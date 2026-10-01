//! 试玩与重放集成。
mod bounded;
mod debugger;
mod evidence;
mod evidence_navigation;
mod replay;
mod replay_location;
pub(in crate::app) mod scope;
mod story;

use super::WorldeditApp;

impl WorldeditApp {
    pub(super) fn play_tab(&mut self, ctx: &egui::Context) {
        self.play_tab_inner(ctx);
    }

    pub(super) fn start_play(&mut self) {
        self.request_play();
    }
}
