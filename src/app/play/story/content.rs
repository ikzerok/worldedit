//! 运行范围和阅读内容共享；短窗正文直接进入主滚动，避免被固定页头扣至零高度。
use super::{Requests, WorldeditApp};
use crate::app::{
    export_scope::UnappliedInput,
    play::{debugger, localization, scope},
    PlayPane,
};
use crate::theme;

impl WorldeditApp {
    pub(super) fn play_content(
        &mut self,
        ui: &mut egui::Ui,
        current_inputs: &[UnappliedInput],
        requests: &mut Requests,
        compact: bool,
    ) {
        if compact && self.replay_debugger.pane == PlayPane::Debugger {
            debugger::render_debugger_compact(ui, &self.replay_debugger);
            return;
        }
        if compact {
            let scope = egui::CollapsingHeader::new("实际运行范围与草稿")
                .id_salt("compact-play-scope")
                .show(ui, |ui| self.play_source_scope(ui, current_inputs));
            self.play_keyboard
                .reveal_setting(ui, &scope.header_response);
        } else {
            self.play_source_scope(ui, current_inputs);
        }
        if let Some(play) = &self.play {
            if play.paused {
                ui.colored_label(
                    theme::MUTED(),
                    if play.stopped {
                        "试玩已停止；可重新开始。"
                    } else {
                        "试玩已暂停；调试重放不会推进当前正文。"
                    },
                );
            }
        }
        self.advance_ordinary_play(ui.ctx());
        let Some(play) = &self.play else { return };
        let identity = play
            .story
            .as_ref()
            .and_then(|story| story.presentation_identity());
        localization::identity_label_with_settings(
            ui,
            identity,
            &mut self.replay_debugger.locale.parallel,
            &self.play_keyboard,
        );
        if compact {
            self.play_transcript(ui, requests);
        } else {
            let transcript_height =
                (ui.available_height() - if play.ended { 44.0 } else { 0.0 }).max(0.0);
            egui::ScrollArea::vertical()
                .max_height(transcript_height)
                .auto_shrink([false, false])
                .stick_to_bottom(self.play_scroll_bottom)
                .show(ui, |ui| self.play_transcript(ui, requests));
        }
        self.play_scroll_bottom = false;
        if self.play.as_ref().is_some_and(|play| play.ended) {
            ui.separator();
            if compact {
                ui.colored_label(theme::SUCCESS(), "—— 世界线收束,故事结束 ——");
            } else {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(theme::SUCCESS(), "—— 世界线收束,故事结束 ——");
                });
            }
        }
    }

    fn play_source_scope(&self, ui: &mut egui::Ui, current_inputs: &[UnappliedInput]) {
        let Some(play) = &self.play else { return };
        scope::render_scope(ui, &play.scope);
        if current_inputs != play.scope.excluded_inputs.as_slice() && !current_inputs.is_empty() {
            ui.colored_label(
                theme::WARNING(),
                "当前另有未应用输入；不会改变此次运行的快照。",
            );
            egui::CollapsingHeader::new("查看当前未应用输入")
                .id_salt("current-unapplied-play-inputs")
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(120.0)
                        .show(ui, |ui| {
                            for input in current_inputs {
                                ui.label(format!("{} · {}", input.kind, input.source));
                            }
                        });
                });
        }
    }

    fn play_transcript(&mut self, ui: &mut egui::Ui, requests: &mut Requests) {
        let Some(play) = &self.play else { return };
        if !play.localized_outputs.is_empty() {
            if let Some(request) = localization::outputs(
                ui,
                &play.localized_outputs,
                self.replay_debugger.locale.parallel,
            ) {
                requests.localization = Some(request);
            }
        } else if let Some(target) = crate::app::wiki::keyword_text(
            ui,
            &play.transcript,
            &play.source_wiki,
            &play.source_catalog,
            &play.transcript_links,
            16.0,
        ) {
            requests.reading = Some(target);
        }
    }
}
