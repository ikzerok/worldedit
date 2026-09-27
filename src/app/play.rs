//! 试玩与重放集成。
mod debugger;
mod replay;
mod story;

use super::WorldeditApp;

impl WorldeditApp {
    pub(super) fn play_tab(&mut self, ctx: &egui::Context) {
        self.play_tab_inner(ctx);
    }

    pub(super) fn start_play(&mut self) {
        self.start_play_inner();
    }
}
