//! 试玩与重放集成。
mod bounded;
pub(in crate::app) mod comparison;
mod debugger;
mod evidence;
mod evidence_navigation;
pub(in crate::app) mod inspection;
pub(in crate::app) mod keyboard;
pub(in crate::app) mod localization;
pub(in crate::app) mod rehearsal;
mod replay;
mod replay_location;
pub(in crate::app) mod report;
pub(in crate::app) mod scope;
mod source_guard;
mod start;
mod story;

use super::WorldeditApp;
use keyboard::SettingsHost;

impl WorldeditApp {
    pub(super) fn play_tab(&mut self, ctx: &egui::Context) {
        self.poll_draft_rehearsal(ctx);
        if self.draft_rehearsal.active {
            self.draft_rehearsal_tab(ctx);
            return;
        }
        self.poll_comparison(ctx);
        if self.comparison.active {
            egui::TopBottomPanel::top("play-main-mode").show(ctx, |ui| {
                self.play_mode_switch(ui, SettingsHost::Comparison)
            });
            if !self.comparison.active {
                // Keep the same mode/locale identity, but never render it in two hosts in one frame.
                ctx.request_repaint();
                return;
            }
        }
        if self.comparison.active {
            self.poll_replay(ctx);
            self.comparison_tab(ctx);
        } else {
            self.play_tab_inner(ctx);
        }
    }

    fn play_mode_switch(&mut self, ui: &mut egui::Ui, host: SettingsHost) -> bool {
        let mut entering_comparison = false;
        ui.horizontal_wrapped(|ui| {
            let ordinary = self
                .play_keyboard
                .setting_in(ui, host, "mode-ordinary", |ui| {
                    ui.selectable_value(&mut self.comparison.active, false, "普通试玩")
                });
            self.play_keyboard.settings_host_focus(ui, host, &ordinary);
            let comparison = self
                .play_keyboard
                .setting_in(ui, host, "mode-comparison", |ui| {
                    ui.selectable_value(&mut self.comparison.active, true, "路线对照")
                });
            entering_comparison = comparison.is_pointer_button_down_on() || self.comparison.active;
            ui.label(crate::theme::muted("路径仅保留于当前会话"));
            if self
                .play_keyboard
                .setting_in(ui, host, "mode-draft", |ui| ui.button("试演当前正文草稿…"))
                .clicked()
            {
                self.request_draft_rehearsal(ui.ctx());
            }
            if self.draft_rehearsal.has_session()
                && self
                    .play_keyboard
                    .setting_in(ui, host, "mode-return-draft", |ui| {
                        ui.button("返回隔离试演")
                    })
                    .clicked()
            {
                self.draft_rehearsal.active = true;
                entering_comparison = true;
            }
            if self
                .play_keyboard
                .setting_in(ui, host, "mode-report", |ui| ui.button("试玩路径报告…"))
                .clicked()
            {
                self.playthrough_report.open = true;
                self.playthrough_report.focus_on_open = true;
            }
        });
        localization::controls(
            ui,
            &mut self.replay_debugger.locale,
            &self.localization_ui.target_locale,
            &self.play_keyboard,
            host,
        );
        entering_comparison
    }

    pub(super) fn start_play(&mut self) {
        self.request_play();
    }
}
