//! 普通试玩共享交互与执行；紧凑版面只改变容器，不建立第二份运行。
mod content;
mod controls;
mod details;
mod progression;
use super::super::WorldeditApp;
use super::{
    evidence_navigation::EvidenceNavigationRequest,
    keyboard::{Activation, SettingsHost},
    localization,
};
use crate::theme;

#[derive(Default)]
struct Requests {
    restart: Option<Activation>,
    reading: Option<worldline_core::TargetRef>,
    localization: Option<localization::Navigation>,
    replay: bool,
    failure: bool,
    evidence: Option<EvidenceNavigationRequest>,
}

impl WorldeditApp {
    pub(super) fn play_tab_inner(&mut self, ctx: &egui::Context) {
        self.prepare_play_keyboard(ctx);
        self.poll_replay(ctx);
        let available = ctx.available_rect();
        let compact = available.width() < 900.0 || available.height() < 360.0;
        if self.play.is_none() {
            egui::CentralPanel::default().show(ctx, |ui| {
                if compact {
                    Self::compact_play_scroll().show(ui, |ui| self.play_start_content(ui, false));
                } else {
                    self.play_start_content(ui, true);
                }
            });
            return;
        }
        let current_inputs = self.unapplied_play_inputs();
        let locale_name = self
            .play
            .as_ref()
            .and_then(|play| play.story.as_ref())
            .and_then(|story| story.presentation_identity())
            .map(|identity| identity.request.target_locale.clone())
            .unwrap_or_default();
        let evidence_access = self.evidence_navigation_access();
        let mut requests = Requests::default();
        if compact {
            egui::CentralPanel::default().show(ctx, |ui| {
                Self::compact_play_scroll().show(ui, |ui| {
                    if self.play_mode_switch(ui, SettingsHost::Ordinary) {
                        return;
                    }
                    self.play_side_header(ui, &mut requests, true);
                    self.play_choices(ui, &mut requests);
                    self.apply_play_side_requests(ctx, &mut requests);
                    ui.separator();
                    self.play_content(ui, &current_inputs, &mut requests, true);
                    let mut details =
                        egui::CollapsingHeader::new("调试与路径").id_salt("compact-play-debugger");
                    if self.play_keyboard.wants_record_focus() {
                        details = details.open(Some(true));
                    }
                    let shown = details.show(ui, |ui| {
                        self.play_debugger_details(ui, &mut requests, &evidence_access);
                    });
                    self.play_keyboard
                        .reveal_setting(ui, &shown.header_response);
                    // The same explicit debugger actions work even when their group is last.
                    self.apply_play_side_requests(ctx, &mut requests);
                });
            });
        } else {
            egui::SidePanel::right("play-side")
                .default_width(300.0)
                .show(ctx, |ui| {
                    self.play_side_header(ui, &mut requests, false);
                    egui::ScrollArea::vertical()
                        .animated(false)
                        .id_salt("play-side-scroll")
                        .show(ui, |ui| {
                            ui.separator();
                            self.play_choices(ui, &mut requests);
                            self.play_debugger_details(ui, &mut requests, &evidence_access);
                        });
                });
            self.apply_play_side_requests(ctx, &mut requests);
            egui::CentralPanel::default().show(ctx, |ui| {
                if !self.play_mode_switch(ui, SettingsHost::Ordinary) {
                    self.play_content(ui, &current_inputs, &mut requests, false);
                }
            });
        }
        if let Some(navigation) = requests.localization {
            self.open_localized_item(ctx, navigation, &locale_name, false);
        }
        self.render_state_inspection(ctx);
        if let Some(target) = requests.reading {
            self.play_keyboard.cancel();
            self.open_reading(target);
        }
        self.prepare_play_keyboard(ctx);
    }

    fn compact_play_scroll() -> egui::ScrollArea {
        egui::ScrollArea::vertical()
            .id_salt("ordinary-play-compact-scroll")
            .animated(false)
            .min_scrolled_height(0.0)
            .auto_shrink([false, false])
    }

    fn play_start_content(&mut self, ui: &mut egui::Ui, centered: bool) {
        if self.play_mode_switch(ui, SettingsHost::Ordinary) {
            return;
        }
        if centered {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| self.play_start_controls(ui));
            });
        } else {
            self.play_start_controls(ui);
        }
    }

    fn play_start_controls(&mut self, ui: &mut egui::Ui) {
        if self
            .snapshot
            .as_ref()
            .is_none_or(|snapshot| snapshot.result.has_errors())
        {
            ui.colored_label(
                theme::ERROR(),
                "故事存在错误,修复后才能试玩(见编辑视图诊断面板)",
            );
            return;
        }
        ui.label("将运行已应用工程稿；已应用但尚未保存的修改也会参与。");
        if let Some(notice) = &self.replay_debugger.notice {
            ui.colored_label(theme::WARNING(), notice);
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("重放种子");
            self.play_keyboard.setting(ui, "start-seed", |ui| {
                ui.add(egui::DragValue::new(&mut self.replay_debugger.seed))
            });
        });
        super::bounded::render_live_budget(ui, &mut self.replay_debugger, &self.play_keyboard);
        let start = self.play_keyboard.button(
            ui,
            "start",
            None,
            true,
            egui::RichText::new("▶ 开始试玩").size(20.0),
        );
        if let Some(activation) = self.play_keyboard.activation(&start) {
            self.start_play_activated(ui.ctx(), activation);
        }
    }

    fn apply_play_side_requests(&mut self, ctx: &egui::Context, requests: &mut Requests) {
        if let Some(activation) = requests.restart.take() {
            self.start_play_activated(ctx, activation);
        }
        if std::mem::take(&mut requests.replay) {
            self.begin_replay(ctx);
        }
        if std::mem::take(&mut requests.failure) {
            self.jump_to_replay_failure();
        }
        if let Some(source) = requests.evidence.take() {
            self.jump_to_evidence_source(ctx, &source);
        }
    }
}
