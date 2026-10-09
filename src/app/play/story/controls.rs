//! 两种布局共享同一组运行控制和选择；不重新解释或执行故事。
use super::{Requests, WorldeditApp};
use crate::app::{
    play::{bounded, keyboard::Target},
    PlayPane,
};
use crate::theme;
use worldline_runtime::ContinuationOutcome;

impl WorldeditApp {
    pub(super) fn play_side_header(
        &mut self,
        ui: &mut egui::Ui,
        requests: &mut Requests,
        compact: bool,
    ) {
        ui.heading("选择");
        if self
            .play_keyboard
            .setting(ui, "inspection", |ui| ui.button("⌕ 状态检查：查值与变化…"))
            .clicked()
        {
            self.replay_debugger.inspection.show(ui.ctx());
        }
        if let Some(play) = &mut self.play {
            ui.horizontal_wrapped(|ui| {
                if play.error.is_some() {
                    ui.label("错误已暂停；重新开始可重试。 ");
                } else if play.stopped {
                    ui.label("试玩已停止；可重新开始。");
                } else if play.ended {
                    ui.label("故事已正常结束");
                } else if play.paused {
                    if self
                        .play_keyboard
                        .setting(ui, "continue", |ui| ui.button("▶ 继续"))
                        .clicked()
                    {
                        play.paused = false;
                        play.interruption = None;
                    }
                } else if self
                    .play_keyboard
                    .setting(ui, "pause", |ui| ui.button("Ⅱ 暂停"))
                    .clicked()
                {
                    play.paused = true;
                    self.play_keyboard.cancel();
                }
                if self
                    .play_keyboard
                    .setting(ui, "stop", |ui| {
                        crate::theme::add_enabled(
                            ui,
                            !play.ended && !play.stopped,
                            egui::Button::new("■ 停止"),
                        )
                    })
                    .clicked()
                {
                    play.paused = true;
                    play.stopped = true;
                    play.interruption = Some(ContinuationOutcome::Cancelled);
                    self.play_keyboard.cancel();
                }
            });
            let response =
                self.play_keyboard
                    .button(ui, "restart", None, true, "↻ 重新开始（已应用稿）");
            requests.restart = self.play_keyboard.activation(&response);
        }
        bounded::render_live_budget(ui, &mut self.replay_debugger, &self.play_keyboard);
        if let Some(outcome) = self.play.as_ref().and_then(|play| play.interruption) {
            ui.label(bounded::interruption_text(outcome));
        }
        if compact && self.play.is_some() {
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                self.play_keyboard.setting(ui, "pane-story", |ui| {
                    ui.selectable_value(&mut self.replay_debugger.pane, PlayPane::Story, "正文")
                });
                self.play_keyboard.setting(ui, "pane-debugger", |ui| {
                    ui.selectable_value(
                        &mut self.replay_debugger.pane,
                        PlayPane::Debugger,
                        "调试信息",
                    )
                });
            });
        }
    }

    pub(super) fn play_choices(&mut self, ui: &mut egui::Ui, requests: &mut Requests) {
        let cur_version = self.version;
        let Some(play) = &mut self.play else { return };
        for diagnostic in &play.entry_diagnostics {
            ui.label(format!(
                "{} · {}:{} · {}",
                diagnostic.code, diagnostic.file, diagnostic.span.line, diagnostic.message
            ));
        }
        if play.error.is_none() && !play.ended && !play.stopped {
            let choices: Vec<_> = play
                .story
                .as_ref()
                .map(|s| {
                    s.choice_presentations()
                        .iter()
                        .map(|c| {
                            (
                                c.label.clone(),
                                c.links.clone(),
                                c.enabled,
                                c.disabled_reason.clone(),
                                c.localization.clone(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            if choices.is_empty() {
                ui.label(if play.paused {
                    "当前没有可选项；继续可推进剩余原稿。"
                } else {
                    "(推进中…)"
                });
            } else {
                ui.label(crate::theme::muted("点击关键词看注释；点击“选择”推进。"));
            }
            for (i, (label, links, enabled, reason, localization)) in choices.iter().enumerate() {
                let mut choose = None;
                ui.push_id(i, |ui| {
                    ui.group(|ui| {
                        if localization.is_some() {
                            if let Some(request) = crate::app::play::localization::item(
                                ui,
                                label,
                                links,
                                localization.as_ref(),
                                self.replay_debugger.locale.parallel,
                            ) {
                                requests.localization = Some(request);
                            }
                        } else if let Some(target) = crate::app::wiki::keyword_text(
                            ui,
                            label,
                            &play.source_wiki,
                            &play.source_catalog,
                            links,
                            15.0,
                        ) {
                            requests.reading = Some(target);
                        }
                        let response = self.play_keyboard.button(
                            ui,
                            "choice",
                            Some(Target::Choice(i)),
                            *enabled
                                && !play.paused
                                && self.play_confirmation.is_none()
                                && !self.playthrough_report.open
                                && !self.replay_debugger.inspection.open,
                            format!("选择：{label}"),
                        );
                        choose = self.play_keyboard.activation(&response);
                        if let Some(reason) = reason {
                            ui.label(crate::theme::muted(format!("暂不可选：{reason}")));
                        }
                    });
                });
                if let Some(activation) = choose {
                    if play
                        .story
                        .as_mut()
                        .is_some_and(|story| story.choose_presentation(i).is_ok())
                    {
                        self.play_keyboard.advanced(ui.ctx(), activation);
                        self.replay_debugger.explanations = None;
                        self.play_scroll_bottom = true;
                    } else {
                        self.play_keyboard.cancel();
                    }
                }
            }
        }
        if play.version != cur_version {
            ui.colored_label(
                theme::WARNING(),
                "已应用工程稿已变化，当前结果仍属于旧快照；重新开始可运行新的已应用稿。",
            );
        }
        if let Some(err) = &play.error {
            ui.colored_label(theme::ERROR(), format!("运行错误:{err}"));
        }
    }
}
