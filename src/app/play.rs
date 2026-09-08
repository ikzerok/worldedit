//! 试玩及运行状态。
use super::{PlayState, WorldeditApp};
use egui::{Color32, Vec2};
use worldline_core::{Analysis, Program};
use worldline_runtime::{Output, Story};
impl WorldeditApp {
    pub(super) fn play_tab(&mut self, ctx: &egui::Context) {
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
        let cur_version = self.version;
        egui::SidePanel::right("play-side")
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("选择");
                ui.separator();
                let Some(play) = &mut self.play else { return };
                if play.error.is_none() && !play.ended {
                    let choices: Vec<(String, u32)> = play
                        .story
                        .as_ref()
                        .map(|s| {
                            s.choices()
                                .iter()
                                .map(|c| (c.label.clone(), c.line))
                                .collect()
                        })
                        .unwrap_or_default();
                    if choices.is_empty() {
                        ui.label("(推进中…)");
                    }
                    for (i, (label, _line)) in choices.iter().enumerate() {
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!("  {}  ", label)).size(15.0),
                                )
                                .wrap_mode(egui::TextWrapMode::Wrap)
                                .min_size(Vec2::new(ui.available_width(), 0.0)),
                            )
                            .clicked()
                            && play
                                .story
                                .as_mut()
                                .map(|s| s.choose(i))
                                .is_some_and(|r| r.is_ok())
                        {
                            self.play_scroll_bottom = true;
                        }
                    }
                }
                ui.separator();
                if ui.button("↻ 重新开始(应用最新改动)").clicked() {
                    restart = true;
                }
                if play.version != cur_version {
                    ui.colored_label(
                        Color32::from_rgb(240, 200, 100),
                        "故事已修改,当前试玩仍是旧版本;点上方按钮应用改动。",
                    );
                }
                if let Some(err) = &play.error {
                    ui.colored_label(Color32::from_rgb(240, 110, 110), format!("运行错误:{err}"));
                }
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
                let anchors: Vec<worldline_runtime::AnchorRecord> = story.anchors().to_vec();
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
            });
        if restart {
            self.start_play();
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(play) = &mut self.play else { return };
            if let Some(story) = &mut play.story {
                match story.continue_story() {
                    Ok(outputs) => {
                        for o in outputs {
                            match o {
                                Output::Text {
                                    content, new_line, ..
                                } => {
                                    if new_line && !play.transcript.is_empty() {
                                        play.transcript.push('\n');
                                    }
                                    play.transcript.push_str(&content);
                                    self.play_scroll_bottom = true;
                                }
                                Output::Ended => play.ended = true,
                            }
                        }
                    }
                    Err(e) => play.error = Some(e.to_string()),
                }
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(self.play_scroll_bottom)
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&play.transcript)
                                .size(16.0)
                                .line_height(Some(26.0)),
                        )
                        .wrap_mode(egui::TextWrapMode::Wrap),
                    );
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
        });
    }

    pub(super) fn start_play(&mut self) {
        let Some(snap) = &self.snapshot else { return };
        if snap.result.has_errors() {
            return;
        }
        // 泄漏快照换取 'static 生命周期(点击级频率;见 PlayState 注释)
        let leaked: &'static (Program, Analysis) = Box::leak(Box::new((
            snap.result.program.clone(),
            snap.result.analysis.clone(),
        )));
        match Story::new(&leaked.0, &leaked.1) {
            Ok(story) => {
                self.play = Some(PlayState {
                    story: Some(story),
                    transcript: String::new(),
                    ended: false,
                    error: None,
                    version: self.version,
                });
                self.play_scroll_bottom = true;
            }
            Err(e) => {
                self.play = Some(PlayState {
                    story: None,
                    transcript: String::new(),
                    ended: true,
                    error: Some(e.to_string()),
                    version: self.version,
                });
            }
        }
    }
}
