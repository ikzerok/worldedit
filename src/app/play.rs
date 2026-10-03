//! 试玩与重放集成。
mod bounded;
pub(in crate::app) mod comparison;
mod debugger;
mod evidence;
mod evidence_navigation;
mod replay;
mod replay_location;
pub(in crate::app) mod scope;
mod source_guard;
mod story;

use super::WorldeditApp;

impl WorldeditApp {
    pub(super) fn play_tab(&mut self, ctx: &egui::Context) {
        self.poll_comparison(ctx);
        egui::TopBottomPanel::top("play-main-mode").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.comparison.active, false, "普通试玩");
                ui.selectable_value(&mut self.comparison.active, true, "路线对照");
                ui.label(crate::theme::muted("路径仅保留于当前会话"));
            });
        });
        if self.comparison.active {
            self.poll_replay(ctx);
            self.comparison_tab(ctx);
        } else {
            self.play_tab_inner(ctx);
        }
    }

    pub(super) fn start_play(&mut self) {
        self.request_play();
    }
}
