//! 试玩及运行状态。
use super::super::{PlayPane, PlayState, WorldeditApp};
use super::debugger;
use egui::Color32;
use worldline_core::{Analysis, Program};
use worldline_runtime::{ContinuationOutcome, Output, ReplayBudget, ReplayCancellation, Story};
impl WorldeditApp {
    pub(super) fn play_tab_inner(&mut self, ctx: &egui::Context) {
        self.poll_replay(ctx);
        // 无故事 / 编译有错误时的引导
        let has_story = self.play.is_some();
        let errors = self
            .snapshot
            .as_ref()
            .map(|s| s.result.has_errors())
            .unwrap_or(true);
        if !has_story {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        if errors {
                            ui.colored_label(
                                Color32::from_rgb(240, 110, 110),
                                "故事存在错误,修复后才能试玩(见编辑视图诊断面板)",
                            );
                            return;
                        }
                        ui.horizontal(|ui| {
                            ui.label("重放种子");
                            ui.add(egui::DragValue::new(&mut self.replay_debugger.seed));
                        });
                        super::bounded::render_live_budget(ui, &mut self.replay_debugger);
                        if ui
                            .button(egui::RichText::new("▶ 开始试玩").size(20.0))
                            .clicked()
                        {
                            self.start_play();
                        }
                    });
                });
            });
            return;
        }
        let mut restart = false;
        let mut reading_request = None;
        let mut replay_request = false;
        let mut failure_jump = false;
        let cur_version = self.version;
        let narrow = ctx.screen_rect().width() < 900.0;
        let can_replay = self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| !snapshot.result.has_errors());
        egui::SidePanel::right("play-side")
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("选择");
                if let Some(play) = &mut self.play {
                    ui.horizontal(|ui| {
                        if play.error.is_some() {
                            ui.label("错误已暂停；重新开始可重试。 ");
                        } else if play.stopped {
                            ui.label("试玩已停止；可重新开始。");
                        } else if play.ended {
                            ui.label("故事已正常结束");
                        } else if play.paused {
                            if ui.button("▶ 继续").clicked() {
                                play.paused = false;
                                play.interruption = None;
                            }
                        } else if ui.button("Ⅱ 暂停").clicked() {
                            play.paused = true;
                        }
                        if ui
                            .add_enabled(!play.ended && !play.stopped, egui::Button::new("■ 停止"))
                            .clicked()
                        {
                            play.paused = true;
                            play.stopped = true;
                            play.interruption = Some(ContinuationOutcome::Cancelled);
                        }
                    });
                    if ui.button("↻ 重新开始(应用最新改动)").clicked() {
                        restart = true;
                    }
                }
                super::bounded::render_live_budget(ui, &mut self.replay_debugger);
                if let Some(outcome) = self.play.as_ref().and_then(|play| play.interruption) {
                    ui.label(super::bounded::interruption_text(outcome));
                }
                if narrow && self.play.is_some() {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut self.replay_debugger.pane,
                            PlayPane::Story,
                            "正文",
                        );
                        ui.selectable_value(
                            &mut self.replay_debugger.pane,
                            PlayPane::Debugger,
                            "调试信息",
                        );
                    });
                }
                egui::ScrollArea::vertical()
                    .id_salt("play-side-scroll")
                    .show(ui, |ui| {
                        if !narrow {
                            ui.separator();
                        }
                        let Some(play) = &mut self.play else { return };
                        for diagnostic in &play.entry_diagnostics {
                            ui.label(format!(
                                "{} · {}:{} · {}",
                                diagnostic.code,
                                diagnostic.file,
                                diagnostic.span.line,
                                diagnostic.message
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
                            for (i, (label, links, enabled, reason)) in choices.iter().enumerate() {
                                let mut choose = false;
                                ui.push_id(i, |ui| {
                                    ui.group(|ui| {
                                        if let Some(snapshot) = &self.snapshot {
                                            if let Some(target) = super::super::wiki::keyword_text(
                                                ui,
                                                label,
                                                &snapshot.wiki,
                                                &snapshot.result.analysis.catalog,
                                                links,
                                                15.0,
                                            ) {
                                                reading_request = Some(target);
                                            }
                                        }
                                        choose = ui
                                            .add_enabled(
                                                *enabled && !play.paused,
                                                egui::Button::new(format!("选择：{label}")),
                                            )
                                            .clicked();
                                        if let Some(reason) = reason {
                                            ui.label(crate::theme::muted(format!(
                                                "暂不可选：{reason}"
                                            )));
                                        }
                                    });
                                });
                                if choose
                                    && play
                                        .story
                                        .as_mut()
                                        .map(|s| s.choose_presentation(i))
                                        .is_some_and(|r| r.is_ok())
                                {
                                    self.replay_debugger.explanations = None;
                                    self.play_scroll_bottom = true;
                                }
                            }
                        }
                        if play.version != cur_version {
                            ui.colored_label(
                                Color32::from_rgb(240, 200, 100),
                                "故事已修改,当前试玩仍是旧版本;点上方按钮应用改动。",
                            );
                        }
                        if let Some(err) = &play.error {
                            ui.colored_label(
                                Color32::from_rgb(240, 110, 110),
                                format!("运行错误:{err}"),
                            );
                        }
                        ui.separator();
                        debugger::render_debugger_controls(
                            ui,
                            &mut self.replay_debugger,
                            play,
                            cur_version,
                            can_replay,
                            &mut replay_request,
                            &mut failure_jump,
                        );
                        ui.separator();
                        ui.heading("状态");
                        let Some(story) = &play.story else {
                            return;
                        };
                        let vars = story.vars();
                        let mut rows: Vec<_> = vars.iter().collect();
                        rows.sort_by(|a, b| a.0.cmp(b.0));
                        egui::Grid::new("vars").num_columns(2).show(ui, |ui| {
                            for (k, v) in rows {
                                ui.label(k.clone());
                                ui.monospace(v.display());
                                ui.end_row();
                            }
                            ui.label("回合");
                            ui.monospace(story.turns().to_string());
                            ui.end_row();
                            ui.label("故事线");
                            ui.monospace(story.storyline().to_string());
                            ui.end_row();
                            if let Some(node) = story.current_node() {
                                ui.label("节点");
                                ui.monospace(node);
                                ui.end_row();
                            }
                        });
                        let met = story.met_list();
                        ui.label(format!(
                            "在场:{}",
                            if met.is_empty() {
                                "无".into()
                            } else {
                                met.join(", ")
                            }
                        ));
                        if !story.states().is_empty() {
                            ui.separator();
                            ui.heading("当前状态");
                            for (id, tags) in story.states() {
                                ui.label(format!(
                                    "{id}：{}",
                                    if tags.is_empty() {
                                        "空".into()
                                    } else {
                                        tags.join(" · ")
                                    }
                                ));
                            }
                            egui::CollapsingHeader::new(format!(
                                "状态变更记录 · {}",
                                story.state_history().len()
                            ))
                            .show(ui, |ui| {
                                egui::ScrollArea::vertical()
                                    .id_salt("runtime-state-history")
                                    .max_height(200.0)
                                    .show(ui, |ui| {
                                        for record in story.state_history().iter().rev() {
                                            ui.label(format!(
                                                "{}：{} → {}",
                                                record.state,
                                                record.before.join(", "),
                                                record.after.join(", ")
                                            ));
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{} · 轮次 {}",
                                                    record.node.as_deref().unwrap_or(""),
                                                    record.turn
                                                ))
                                                .small()
                                                .color(Color32::GRAY),
                                            );
                                            if let Some(note) = &record.note {
                                                ui.label(note);
                                            }
                                        }
                                    });
                            });
                        }
                        // 锚点记录(按发生序)
                        ui.separator();
                        ui.heading("锚点记录");
                        let anchors: Vec<worldline_runtime::AnchorRecord> =
                            story.anchors().to_vec();
                        if anchors.is_empty() {
                            ui.colored_label(
                                Color32::GRAY,
                                "(暂无;记录由 anchor 语句与漂流、叙事身份、人物变动产生)",
                            );
                        }
                        egui::ScrollArea::vertical()
                            .max_height(220.0)
                            .show(ui, |ui| {
                                for a in anchors.iter().rev() {
                                    let mut line = format!("◆ [{}] {}", a.kind.label(), a.name);
                                    if let Some(d) = &a.detail {
                                        line.push_str(&format!(" → {d}"));
                                    }
                                    ui.colored_label(Color32::from_rgb(120, 220, 190), line);
                                    if let Some(n) = &a.note {
                                        ui.indent("anchor-note", |ui| {
                                            ui.colored_label(Color32::GRAY, format!("↳ {n}"));
                                        });
                                    }
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "   @{} · 故事线 {} · 回合 {}",
                                            a.node.as_deref().unwrap_or("?"),
                                            a.storyline,
                                            a.turn
                                        ))
                                        .size(10.0)
                                        .color(Color32::from_rgb(120, 130, 150)),
                                    );
                                }
                            });
                        ui.separator();
                    });
            });
        if restart {
            self.start_play();
        }
        if replay_request {
            self.begin_replay(ctx);
        }
        if failure_jump {
            self.jump_to_replay_failure();
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(play) = &mut self.play else { return };
            if narrow && self.replay_debugger.pane == PlayPane::Debugger {
                debugger::render_debugger_compact(ui, &self.replay_debugger);
            } else if let Some(story) = &mut play.story {
                if play.paused {
                    ui.colored_label(
                        Color32::GRAY,
                        if play.stopped {
                            "试玩已停止；可重新开始。"
                        } else {
                            "试玩已暂停；调试重放不会推进当前正文。"
                        },
                    );
                }
                if !play.paused && !play.ended && !play.stopped && play.error.is_none() {
                    let budget = ReplayBudget::new(
                        self.replay_debugger.live_max_steps,
                        self.replay_debugger.live_time_budget_ms,
                    );
                    match story.continue_story_bounded(budget, &ReplayCancellation::new()) {
                        Ok(continuation) => {
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
                                        ..
                                    } => {
                                        if new_line && !play.transcript.is_empty() {
                                            play.transcript.push('\n');
                                        }
                                        if let Some(speaker) = speaker {
                                            let name = self
                                                .snapshot
                                                .as_ref()
                                                .and_then(|snapshot| {
                                                    snapshot
                                                        .result
                                                        .analysis
                                                        .catalog
                                                        .object(&speaker)
                                                })
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
                                        play.transcript_links.extend(links.into_iter().map(
                                            |mut link| {
                                                link.start += offset;
                                                link.end += offset;
                                                link
                                            },
                                        ));
                                        play.transcript.push_str(&content);
                                        self.play_scroll_bottom = true;
                                    }
                                    Output::Ended => play.ended = true,
                                }
                            }
                        }
                        Err(e) => {
                            play.error = Some(e.to_string());
                            play.paused = true;
                        }
                    }
                }
            }
            if !narrow || self.replay_debugger.pane == PlayPane::Story {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(self.play_scroll_bottom)
                    .show(ui, |ui| {
                        if let Some(snapshot) = &self.snapshot {
                            if let Some(target) = super::super::wiki::keyword_text(
                                ui,
                                &play.transcript,
                                &snapshot.wiki,
                                &snapshot.result.analysis.catalog,
                                &play.transcript_links,
                                16.0,
                            ) {
                                reading_request = Some(target);
                            }
                        }
                    });
                self.play_scroll_bottom = false;
                if play.ended {
                    ui.separator();
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(
                            Color32::from_rgb(130, 220, 130),
                            "—— 世界线收束,故事结束 ——",
                        );
                    });
                }
            }
        });
        if let Some(target) = reading_request {
            self.open_reading(target);
        }
    }

    pub(super) fn start_play_inner(&mut self) {
        let Some(snap) = &self.snapshot else { return };
        if snap.result.has_errors() {
            return;
        }
        self.replay_debugger.explanations = None;
        let entry_diagnostics = worldline_core::analysis::execution_diagnostics(
            &snap.result.program,
            &snap.result.analysis,
        );
        // 泄漏快照换取 'static 生命周期(点击级频率;见 PlayState 注释)
        let leaked: &'static (Program, Analysis) = Box::leak(Box::new((
            snap.result.program.clone(),
            snap.result.analysis.clone(),
        )));
        match Story::new_with_seed(&leaked.0, &leaked.1, self.replay_debugger.seed) {
            Ok(story) => {
                self.play = Some(PlayState {
                    story: Some(story),
                    transcript: String::new(),
                    transcript_links: Vec::new(),
                    ended: false,
                    error: None,
                    version: self.version,
                    paused: false,
                    stopped: false,
                    interruption: None,
                    entry_diagnostics,
                });
                self.play_scroll_bottom = true;
            }
            Err(e) => {
                self.play = Some(PlayState {
                    story: None,
                    transcript: String::new(),
                    transcript_links: Vec::new(),
                    ended: true,
                    error: Some(e.to_string()),
                    version: self.version,
                    paused: true,
                    stopped: false,
                    interruption: None,
                    entry_diagnostics,
                });
            }
        }
    }
}
