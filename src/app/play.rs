//! 试玩及运行状态。
use super::{PlayPane, PlayState, ReplayDebugger, SavedReplayPath, WorldeditApp};
use egui::Color32;
use worldline_core::{Analysis, Program};
use worldline_runtime::{
    Output, ReplayBudget, ReplayCancellation, ReplayOrigin, ReplayResult, ReplayStatus,
    ReplayTrace, Story, REPLAY_SCHEMA_VERSION,
};

const MAX_IMPORTED_TRACE_BYTES: usize = 1024 * 1024;
const MAX_IMPORTED_TRACE_STEPS: usize = 20_000;
const MAX_IMPORTED_TRACE_STRING_BYTES: usize = 256 * 1024;
const MAX_SAVED_REPLAY_PATHS: usize = 64;
impl WorldeditApp {
    pub(super) fn play_tab(&mut self, ctx: &egui::Context) {
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
        let mut failure_jump = None;
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
                        if play.paused {
                            if ui.button("▶ 继续").clicked() {
                                play.paused = false;
                            }
                        } else if ui.button("Ⅱ 暂停").clicked() {
                            play.paused = true;
                        }
                        if ui.button("■ 停止").clicked() {
                            play.paused = true;
                            play.ended = true;
                        }
                    });
                    if ui.button("↻ 重新开始(应用最新改动)").clicked() {
                        restart = true;
                    }
                }
                egui::ScrollArea::vertical()
                    .id_salt("play-side-scroll")
                    .show(ui, |ui| {
                        ui.separator();
                        let Some(play) = &mut self.play else { return };
                        if narrow {
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
                        if play.error.is_none() && !play.ended {
                            let choices: Vec<_> = play
                                .story
                                .as_ref()
                                .map(|s| {
                                    s.choices()
                                        .iter()
                                        .map(|c| (c.label.clone(), c.links.clone()))
                                        .collect()
                                })
                                .unwrap_or_default();
                            if choices.is_empty() {
                                ui.label("(推进中…)");
                            } else {
                                ui.label(crate::theme::muted("点击关键词看注释；点击“选择”推进。"));
                            }
                            for (i, (label, links)) in choices.iter().enumerate() {
                                let mut choose = false;
                                ui.push_id(i, |ui| {
                                    ui.group(|ui| {
                                        if let Some(snapshot) = &self.snapshot {
                                            if let Some(target) = super::wiki::keyword_text(
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
                                        choose = ui.button(format!("选择：{label}")).clicked();
                                    });
                                });
                                if choose
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
                        render_debugger_controls(
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
        if let Some((node, line)) = failure_jump {
            let file = self
                .snapshot
                .as_ref()
                .and_then(|snapshot| {
                    snapshot
                        .result
                        .analysis
                        .graph
                        .nodes
                        .iter()
                        .find(|candidate| candidate.name == node)
                        .map(|candidate| std::path::PathBuf::from(candidate.file.clone()))
                })
                .unwrap_or_else(|| self.active_file.clone());
            self.jump_to_file(&file.to_string_lossy(), line, 1);
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(play) = &mut self.play else { return };
            if narrow && self.replay_debugger.pane == PlayPane::Debugger {
                render_debugger_compact(ui, &self.replay_debugger);
            } else if let Some(story) = &mut play.story {
                if play.paused {
                    ui.colored_label(Color32::GRAY, "试玩已暂停；调试重放不会推进当前正文。");
                }
                if !play.paused && !play.ended {
                    match story.continue_story() {
                        Ok(outputs) => {
                            for o in outputs {
                                match o {
                                    Output::Text {
                                        content,
                                        new_line,
                                        links,
                                        ..
                                    } => {
                                        if new_line && !play.transcript.is_empty() {
                                            play.transcript.push('\n');
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
                        Err(e) => play.error = Some(e.to_string()),
                    }
                }
            }
            if !narrow || self.replay_debugger.pane == PlayPane::Story {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(self.play_scroll_bottom)
                    .show(ui, |ui| {
                        if let Some(snapshot) = &self.snapshot {
                            if let Some(target) = super::wiki::keyword_text(
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
                });
            }
        }
    }

    fn begin_replay(&mut self, ctx: &egui::Context) {
        let _ = ctx;
        if self.replay_debugger.job.is_some() {
            return;
        }
        let Some(index) = self.replay_debugger.selected_path else {
            self.replay_debugger.notice = Some("请先选择或导入一条路径".into());
            return;
        };
        let Some(trace) = self
            .replay_debugger
            .saved_paths
            .get(index)
            .map(|path| path.trace.clone())
        else {
            self.replay_debugger.selected_path = None;
            self.replay_debugger.notice = Some("所选路径已失效，请重新选择".into());
            return;
        };
        let Some(snapshot) = self.snapshot.as_ref() else {
            self.replay_debugger.notice = Some("没有可重放的编译快照".into());
            return;
        };
        if snapshot.result.has_errors() {
            self.replay_debugger.notice = Some("当前稿件有编译错误，无法重放".into());
            return;
        }
        let program = snapshot.result.program.clone();
        let analysis = snapshot.result.analysis.clone();
        let max_steps = self.replay_debugger.max_steps;
        #[cfg(target_arch = "wasm32")]
        let max_steps = max_steps.min(100_000);
        let time_budget_ms = self.replay_debugger.time_budget_ms;
        #[cfg(target_arch = "wasm32")]
        let time_budget_ms = time_budget_ms.min(2_000);
        let budget = ReplayBudget::new(max_steps, time_budget_ms);
        let cancellation = ReplayCancellation::new();
        let path_name = self.replay_debugger.saved_paths[index].name.clone();
        self.replay_debugger.result = None;
        self.replay_debugger.result_path_name = Some(path_name);
        self.replay_debugger.result_version = Some(self.version);
        self.replay_debugger.notice = None;
        self.replay_debugger.explanations = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let worker_cancellation = cancellation.clone();
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let result =
                    ReplayTrace::replay(&program, &analysis, &trace, budget, &worker_cancellation)
                        .map_err(|error| error.to_string());
                let _ = sender.send(result);
            });
            self.replay_debugger.job = Some(super::ReplayJob {
                cancellation,
                receiver,
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        #[cfg(target_arch = "wasm32")]
        {
            match ReplayTrace::replay(&program, &analysis, &trace, budget, &cancellation) {
                Ok(result) => self.replay_debugger.result = Some(result),
                Err(error) => self.replay_debugger.notice = Some(error.to_string()),
            }
        }
    }

    fn poll_replay(&mut self, ctx: &egui::Context) {
        let state = self
            .replay_debugger
            .job
            .as_ref()
            .map(|job| job.receiver.try_recv());
        match state {
            Some(Ok(Ok(result))) => {
                self.replay_debugger.job = None;
                self.replay_debugger.result = Some(result);
            }
            Some(Ok(Err(error))) => {
                self.replay_debugger.job = None;
                self.replay_debugger.notice = Some(error);
            }
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                self.replay_debugger.job = None;
                self.replay_debugger.notice = Some("重放任务异常退出".into());
            }
            Some(Err(std::sync::mpsc::TryRecvError::Empty)) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
            }
            None => {}
        }
    }
}

fn render_debugger_controls(
    ui: &mut egui::Ui,
    debugger: &mut ReplayDebugger,
    play: &mut PlayState,
    current_version: u64,
    can_replay: bool,
    replay_request: &mut bool,
    failure_jump: &mut Option<(String, u32)>,
) {
    ui.heading("叙事调试器");
    ui.horizontal(|ui| {
        ui.label("种子");
        ui.add(egui::DragValue::new(&mut debugger.seed));
    });
    if let Some(story) = &play.story {
        let trace = story.replay_trace();
        ui.label(format!("运行 fingerprint：{}", trace.fingerprint));
        ui.label(format!(
            "runtime {} · schema {}",
            trace.runtime_version, trace.schema_version
        ));
        if let Some(seed) = trace_seed(&trace) {
            ui.label(format!("当前运行种子：{seed}"));
        }
        ui.horizontal(|ui| {
            ui.label("路径名");
            ui.add(egui::TextEdit::singleline(&mut debugger.path_name).desired_width(120.0));
        });
        if ui.button("● 保存当前路径").clicked() {
            let name = if debugger.path_name.trim().is_empty() {
                format!("路径 {}", debugger.saved_paths.len() + 1)
            } else {
                debugger.path_name.trim().to_owned()
            };
            let trace_bytes = serde_json::to_vec(&trace).map(|json| json.len());
            if debugger.saved_paths.len() >= MAX_SAVED_REPLAY_PATHS {
                debugger.notice = Some(format!("当前会话最多保留 {MAX_SAVED_REPLAY_PATHS} 条路径"));
            } else if trace.steps.len() > MAX_IMPORTED_TRACE_STEPS
                || trace_bytes.map_or(true, |bytes| bytes > 4 * MAX_IMPORTED_TRACE_BYTES)
            {
                debugger.notice = Some("当前路径超过 20,000 步或 4 MiB 保存边界".into());
            } else {
                debugger.saved_paths.push(SavedReplayPath {
                    name: name.clone(),
                    trace,
                });
                debugger.selected_path = Some(debugger.saved_paths.len() - 1);
                debugger.path_name = format!("路径 {}", debugger.saved_paths.len() + 1);
                debugger.notice = Some(format!("已在本次工作台会话中保存路径“{name}”"));
            }
        }
    } else {
        ui.colored_label(Color32::GRAY, "开始试玩后可录制实际选择路径。");
    }
    if ui.button("解释当前条件（只读）").clicked() {
        debugger.explanations = match play.story.as_ref().map(Story::explain_choices) {
            Some(Ok(explanations)) => Some(explanations),
            Some(Err(error)) => {
                debugger.notice = Some(format!("条件解释失败：{error}"));
                None
            }
            None => Some(Vec::new()),
        };
    }

    let selected_text = debugger
        .selected_path
        .and_then(|index| debugger.saved_paths.get(index))
        .map(|path| path.name.as_str())
        .unwrap_or("选择已录制路径");
    egui::ComboBox::from_id_salt("replay-path-select")
        .selected_text(selected_text)
        .show_ui(ui, |ui| {
            for (index, path) in debugger.saved_paths.iter().enumerate() {
                ui.selectable_value(&mut debugger.selected_path, Some(index), &path.name);
            }
        });
    ui.horizontal(|ui| {
        ui.label("步数上限");
        ui.add(egui::DragValue::new(&mut debugger.max_steps).range(1..=1_000_000_000));
    });
    ui.horizontal(|ui| {
        ui.label("时限 ms");
        ui.add(egui::DragValue::new(&mut debugger.time_budget_ms).range(1..=600_000));
    });
    #[cfg(target_arch = "wasm32")]
    ui.label("浏览器内重放上限为 100,000 步或 2,000 ms；执行期间无法响应取消。");
    let has_path = debugger
        .selected_path
        .is_some_and(|index| debugger.saved_paths.get(index).is_some());
    ui.horizontal(|ui| {
        let replay = ui.add_enabled(
            can_replay && has_path && debugger.job.is_none(),
            egui::Button::new("▶ 重放所选路径"),
        );
        *replay_request |= replay.clicked();
        if let Some(job) = &debugger.job {
            ui.label("重放中…");
            if ui.button("取消重放").clicked() {
                job.cancellation.cancel();
            }
        }
    });
    if let Some(playback) = debugger
        .selected_path
        .and_then(|index| debugger.saved_paths.get(index))
    {
        let trace = &playback.trace;
        ui.label(format!(
            "所选路径：{} · 原 fingerprint {} · {} 步 · {}",
            playback.name,
            trace.fingerprint,
            trace.steps.len(),
            if trace.complete {
                "已完整记录"
            } else {
                "记录未到结尾"
            }
        ));
        if let Some(observation) = &trace.initial_observation {
            egui::CollapsingHeader::new("查看路径起始状态").show(ui, |ui| {
                ui.code(observation.state.to_string());
            });
        }
        match &trace.origin {
            ReplayOrigin::Entry { seed } => {
                ui.label(format!("入口起点 · seed {seed}"));
            }
            ReplayOrigin::Checkpoint { checkpoint } => {
                ui.label(format!(
                    "检查点起点 · seed {} · fingerprint {}",
                    checkpoint.seed, checkpoint.fingerprint
                ));
                egui::CollapsingHeader::new("查看检查点状态").show(ui, |ui| {
                    ui.code(&checkpoint.state);
                });
            }
        };
        egui::CollapsingHeader::new("轨迹逐步状态变化").show(ui, |ui| {
            let mut previous_state = trace
                .initial_observation
                .as_ref()
                .map(|observation| observation.state.clone());
            for (step_index, step) in trace.steps.iter().enumerate() {
                ui.label(format!(
                    "第 {} 步 · {}:{} · {}",
                    step_index + 1,
                    step.choice.node,
                    step.choice.line,
                    step.choice.label
                ));
                if let Some(observation) = &step.observation {
                    let changes = state_changes(previous_state.as_ref(), Some(&observation.state));
                    if changes.is_empty() {
                        ui.label("状态无变化");
                    }
                    for (key, before, after) in changes {
                        ui.label(format!("{key}：{before} → {after}"));
                    }
                    previous_state = Some(observation.state.clone());
                } else {
                    ui.label("此步之后没有记录状态");
                }
            }
        });
        if ui.button("导出所选路径 JSON").clicked() {
            debugger.export_json = serde_json::to_string_pretty(trace)
                .unwrap_or_else(|error| format!("JSON 序列化失败：{error}"));
        }
    }
    egui::CollapsingHeader::new("导入路径 JSON（最多 1 MiB / 20,000 步）")
        .default_open(true)
        .show(ui, |ui| {
            if debugger.import_json.len() > MAX_IMPORTED_TRACE_BYTES {
                ui.colored_label(
                    Color32::LIGHT_RED,
                    "轨迹 JSON 超过 1 MiB 边界；内容已拒绝导入。",
                );
                if ui.button("清除超限输入").clicked() {
                    debugger.import_json.clear();
                }
                if ui.button("拒绝超限轨迹").clicked() {
                    debugger.notice = Some("轨迹 JSON 超过 1 MiB 边界".into());
                }
            } else {
                ui.add(
                    egui::TextEdit::multiline(&mut debugger.import_json)
                        .code_editor()
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
                if ui.button("检查并导入路径").clicked() {
                    match validate_imported_trace(&debugger.import_json) {
                        Ok(trace) => {
                            if debugger.saved_paths.len() >= MAX_SAVED_REPLAY_PATHS {
                                debugger.notice = Some(format!(
                                    "当前会话最多保留 {MAX_SAVED_REPLAY_PATHS} 条路径"
                                ));
                            } else {
                                let name = format!("导入路径 {}", debugger.saved_paths.len() + 1);
                                debugger.saved_paths.push(SavedReplayPath { name, trace });
                                debugger.selected_path = Some(debugger.saved_paths.len() - 1);
                                debugger.notice =
                                    Some("路径格式通过检查，已加入当前调试会话".into());
                            }
                        }
                        Err(error) => debugger.notice = Some(error),
                    }
                }
            }
        });
    if !debugger.export_json.is_empty() {
        ui.add(
            egui::TextEdit::multiline(&mut debugger.export_json)
                .code_editor()
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );
        if ui.button("复制 JSON").clicked() {
            ui.ctx().copy_text(debugger.export_json.clone());
        }
    }
    if let Some(explanations) = &debugger.explanations {
        egui::CollapsingHeader::new(format!("条件说明 · {} 项", explanations.len()))
            .default_open(true)
            .show(ui, |ui| {
                for explanation in explanations {
                    ui.label(format!(
                        "{} · {}",
                        explanation.choice.node, explanation.choice.label
                    ));
                    if let Some(condition) = &explanation.condition {
                        ui.monospace(&condition.expression);
                        ui.label(format!(
                            "结果：{}",
                            condition
                                .result
                                .map_or_else(|| "错误".into(), |value| value.to_string())
                        ));
                        if let Some(error) = &condition.error {
                            ui.colored_label(Color32::LIGHT_RED, error);
                        }
                    }
                    if let Some(reason) = &explanation.unavailable_reason {
                        ui.colored_label(Color32::from_rgb(240, 180, 100), reason);
                    } else if explanation.available {
                        ui.label("可选择");
                    }
                }
            });
    }
    if let Some(result) = &debugger.result {
        ui.separator();
        ui.heading(format!(
            "重放结果 · {}",
            debugger.result_path_name.as_deref().unwrap_or("路径")
        ));
        ui.label(replay_status_text(&result.status));
        if let Some(version) = debugger.result_version {
            ui.label(format!("本结果使用编辑版本 #{version}"));
            if version != current_version {
                ui.colored_label(
                    Color32::from_rgb(240, 200, 100),
                    "当前编辑稿已再次变化；此结果属于旧编译快照。请显式重新重放。",
                );
            }
        }
        ui.label(format!(
            "fingerprint：{} → {} · 完成选择 {} · 执行语句 {} · 当前节点 {}",
            result.original_fingerprint,
            result.source_fingerprint,
            result.completed_choices,
            result.executed_steps,
            result.current_node.as_deref().unwrap_or("无")
        ));
        let selected_trace = debugger
            .selected_path
            .and_then(|index| debugger.saved_paths.get(index))
            .map(|path| &path.trace);
        if let Some((node, line)) = failure_location(result, selected_trace) {
            ui.label(format!("失败位置：{node} · 第 {line} 行"));
            if ui.button("跳转到失败位置").clicked() {
                *failure_jump = Some((node, line));
            }
        }
        egui::CollapsingHeader::new("步骤前后状态差异")
            .default_open(true)
            .show(ui, |ui| {
                if result.state_diff.is_empty() {
                    ui.label("状态没有差异");
                }
                for (key, value) in &result.state_diff {
                    ui.horizontal(|ui| {
                        ui.monospace(key);
                        ui.label(value.to_string());
                    });
                }
            });
        ui.label("本次访问覆盖（未列出的节点表示未测试，不表示不可达）");
        for (node, count) in &result.coverage.visited_nodes {
            ui.label(format!("访问 ×{count} · {node}"));
        }
        for choice in &result.coverage.selected_choices {
            ui.label(format!(
                "选择 ×{} · {} · {}",
                choice.count, choice.node, choice.label
            ));
        }
    }
    if let Some(notice) = &debugger.notice {
        ui.colored_label(Color32::from_rgb(240, 200, 100), notice);
    }
    if play.version != current_version {
        ui.label("当前运行快照已过期；重放按钮使用最新编译快照。正文结果不会自动替换。");
    }
}

fn render_debugger_compact(ui: &mut egui::Ui, debugger: &ReplayDebugger) {
    ui.heading("叙事调试信息");
    ui.label(format!("已录制路径：{}", debugger.saved_paths.len()));
    if let Some(index) = debugger.selected_path {
        if let Some(path) = debugger.saved_paths.get(index) {
            ui.label(format!(
                "当前路径：{} · {} 步",
                path.name,
                path.trace.steps.len()
            ));
            ui.label(format!("来源 fingerprint：{}", path.trace.fingerprint));
        }
    }
    if debugger.job.is_some() {
        ui.label("重放正在后台运行；可在右侧调试器中取消。");
    }
    if let Some(result) = &debugger.result {
        ui.heading(replay_status_text(&result.status));
        ui.label(format!(
            "当前节点：{}",
            result.current_node.as_deref().unwrap_or("无")
        ));
        ui.label(format!("已执行选择：{}", result.completed_choices));
        for (key, value) in &result.state_diff {
            ui.label(format!("{key}：{value}"));
        }
        for (node, count) in &result.coverage.visited_nodes {
            ui.label(format!("访问 ×{count} · {node}"));
        }
    }
    if let Some(notice) = &debugger.notice {
        ui.colored_label(Color32::from_rgb(240, 200, 100), notice);
    }
}

fn trace_seed(trace: &ReplayTrace) -> Option<u64> {
    match &trace.origin {
        ReplayOrigin::Entry { seed } => Some(*seed),
        ReplayOrigin::Checkpoint { checkpoint } => Some(checkpoint.seed),
    }
}

fn state_changes(
    before: Option<&serde_json::Value>,
    after: Option<&serde_json::Value>,
) -> Vec<(String, String, String)> {
    let before = before.and_then(serde_json::Value::as_object);
    let after = after.and_then(serde_json::Value::as_object);
    let keys = before
        .into_iter()
        .flat_map(|object| object.keys())
        .chain(after.into_iter().flat_map(|object| object.keys()))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    keys.into_iter()
        .filter_map(|key| {
            let old = before.and_then(|object| object.get(&key));
            let new = after.and_then(|object| object.get(&key));
            (old != new).then(|| {
                (
                    key,
                    old.map_or_else(|| "∅".into(), ToString::to_string),
                    new.map_or_else(|| "∅".into(), ToString::to_string),
                )
            })
        })
        .collect()
}

fn validate_imported_trace(json: &str) -> Result<ReplayTrace, String> {
    if json.len() > MAX_IMPORTED_TRACE_BYTES {
        return Err("轨迹 JSON 超过 1 MiB 边界".into());
    }
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|error| format!("JSON 格式错误：{error}"))?;
    let mut pending = vec![&value];
    while let Some(value) = pending.pop() {
        match value {
            serde_json::Value::String(text) if text.len() > MAX_IMPORTED_TRACE_STRING_BYTES => {
                return Err("轨迹字段超过 256 KiB 边界".into());
            }
            serde_json::Value::Array(items) => pending.extend(items),
            serde_json::Value::Object(items) => pending.extend(items.values()),
            _ => {}
        }
    }
    let trace: ReplayTrace =
        serde_json::from_value(value).map_err(|error| format!("轨迹格式不兼容：{error}"))?;
    if trace.schema_version != REPLAY_SCHEMA_VERSION {
        return Err(format!(
            "轨迹 schema_version {} 不受支持",
            trace.schema_version
        ));
    }
    if trace.steps.len() > MAX_IMPORTED_TRACE_STEPS {
        return Err(format!("轨迹步数超过 {MAX_IMPORTED_TRACE_STEPS} 边界"));
    }
    Ok(trace)
}

fn replay_status_text(status: &ReplayStatus) -> String {
    match status {
        ReplayStatus::Replayed { ended, complete } => {
            format!("重放完成 · 故事结束 {ended} · 路径完整 {complete}")
        }
        ReplayStatus::Diverged {
            step_index, reason, ..
        } => format!("路径在第 {} 步分歧：{reason}", step_index + 1),
        ReplayStatus::StepBudgetExceeded => "已达到重放语句步数上限".into(),
        ReplayStatus::TimeBudgetExceeded => "已达到重放时间上限".into(),
        ReplayStatus::Cancelled => "重放已取消；已完成部分记录保留".into(),
        ReplayStatus::IncompleteTrace => "轨迹在故事结束前中断".into(),
        ReplayStatus::StoryFailed { message, .. } => format!("故事运行失败：{message}"),
    }
}

fn failure_location(result: &ReplayResult, trace: Option<&ReplayTrace>) -> Option<(String, u32)> {
    match &result.status {
        ReplayStatus::Diverged {
            expected_choice: Some(choice),
            ..
        } => Some((choice.node.clone(), choice.line)),
        ReplayStatus::Diverged {
            step_index,
            actual_choices,
            ..
        } => actual_choices
            .first()
            .map(|choice| (choice.node.clone(), choice.line))
            .or_else(|| {
                trace
                    .and_then(|trace| trace.steps.get(*step_index))
                    .map(|step| (step.choice.node.clone(), step.choice.line))
            }),
        ReplayStatus::StoryFailed {
            node: Some(node),
            line: Some(line),
            ..
        } => Some((node.clone(), *line)),
        _ => None,
    }
}
