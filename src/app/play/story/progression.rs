//! 普通试玩唯一推进入口；两种版面共用同一次有界求值与输出累积。
use super::WorldeditApp;
use worldline_runtime::{ContinuationOutcome, Output, ReplayBudget, ReplayCancellation};

impl WorldeditApp {
    pub(super) fn advance_ordinary_play(&mut self, ctx: &egui::Context) {
        if self.play_confirmation.is_some()
            || self.playthrough_report.open
            || self.replay_debugger.inspection.open
        {
            return;
        }
        let Some(play) = &mut self.play else { return };
        if play.paused || play.ended || play.stopped || play.error.is_some() {
            return;
        }
        let Some(story) = &mut play.story else { return };
        let budget = ReplayBudget::new(
            self.replay_debugger.live_max_steps,
            self.replay_debugger.live_time_budget_ms,
        );
        match story.continue_story_bounded(budget, &ReplayCancellation::new()) {
            Ok(continuation) => {
                self.play_keyboard.settled(
                    ctx,
                    continuation.outcome,
                    story
                        .choice_presentations()
                        .iter()
                        .position(|choice| choice.enabled),
                );
                match continuation.outcome {
                    ContinuationOutcome::Choice | ContinuationOutcome::Ended => {
                        play.interruption = None
                    }
                    outcome => {
                        play.interruption = Some(outcome);
                        play.paused = true;
                    }
                }
                for o in continuation.outputs {
                    match o {
                        Output::Text {
                            content,
                            new_line,
                            links,
                            speaker,
                            localization,
                            ..
                        } => {
                            if localization.is_some() {
                                play.localized_outputs.push(
                                    crate::draft_rehearsal_worker::DisplayOutput {
                                        content: content.clone(),
                                        links: links.clone(),
                                        localization: localization.map(|metadata| *metadata),
                                        new_line,
                                        speaker: speaker.as_ref().map(|target| {
                                            play.source_catalog
                                                .object(target)
                                                .map(|o| o.display.clone())
                                                .unwrap_or_else(|| target.id.clone())
                                        }),
                                    },
                                );
                            }
                            if new_line && !play.transcript.is_empty() {
                                play.transcript.push('\n');
                            }
                            if let Some(speaker) = speaker {
                                let name = play
                                    .source_catalog
                                    .object(&speaker)
                                    .map(|object| object.display.clone())
                                    .unwrap_or_else(|| speaker.id.clone());
                                let start = play.transcript.len();
                                play.transcript.push_str(&name);
                                let end = play.transcript.len();
                                play.transcript_links.push(
                                    worldline_core::navigation::RenderedLink {
                                        target: speaker,
                                        start,
                                        end,
                                    },
                                );
                                play.transcript.push('：');
                            }
                            let offset = play.transcript.len();
                            play.transcript_links
                                .extend(links.into_iter().map(|mut link| {
                                    link.start += offset;
                                    link.end += offset;
                                    link
                                }));
                            play.transcript.push_str(&content);
                            self.play_scroll_bottom = true;
                        }
                        Output::Ended => play.ended = true,
                    }
                }
            }
            Err(e) => {
                self.play_keyboard.cancel();
                play.error = Some(e.to_string());
                play.paused = true;
            }
        }
    }
}
