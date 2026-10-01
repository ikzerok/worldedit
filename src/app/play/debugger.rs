use super::super::{PlayState, ReplayDebugger, SavedReplayPath};
use super::replay_location::failure_location;
use egui::Color32;
use worldline_runtime::{ReplayOrigin, ReplayStatus, ReplayTrace, REPLAY_SCHEMA_VERSION};

const MAX_IMPORTED_TRACE_BYTES: usize = 1024 * 1024;
const MAX_IMPORTED_TRACE_STEPS: usize = 20_000;
const MAX_IMPORTED_TRACE_STRING_BYTES: usize = 256 * 1024;
const MAX_SAVED_REPLAY_PATHS: usize = 64;
pub(super) fn render_debugger_controls(
    ui: &mut egui::Ui,
    debugger: &mut ReplayDebugger,
    play: &mut PlayState,
    current_version: u64,
    can_replay: bool,
    replay_request: &mut bool,
    failure_jump: &mut bool,
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
    let actual_evidence = play
        .story
        .as_ref()
        .and_then(|story| story.choice_evidence());
    if debugger
        .explanations
        .as_deref()
        .is_some_and(|shown| Some(shown) != actual_evidence)
    {
        debugger.explanations = None;
    }
    if ui
        .add_enabled(
            actual_evidence.is_some(),
            egui::Button::new("解释当前条件（只读）"),
        )
        .clicked()
    {
        debugger.explanations = actual_evidence.map(<[_]>::to_vec);
    }
    if let Some(explanations) = &debugger.explanations {
        super::evidence::render(ui, explanations, play.version, current_version);
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
    ui.label("浏览器每帧最多执行 512 个解释器步骤或 4 ms；单步与状态恢复不可中断，总上限 100,000 步或 2,000 ms。");
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
        .default_open(false)
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
                egui::ScrollArea::vertical()
                    .id_salt("debugger-import-json")
                    .max_height(120.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut debugger.import_json)
                                .code_editor()
                                .desired_rows(3)
                                .desired_width(f32::INFINITY),
                        );
                    });
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
        egui::ScrollArea::vertical()
            .id_salt("debugger-export-json")
            .max_height(120.0)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut debugger.export_json)
                        .code_editor()
                        .desired_rows(4)
                        .desired_width(f32::INFINITY),
                );
            });
        if ui.button("复制 JSON").clicked() {
            ui.ctx().copy_text(debugger.export_json.clone());
        }
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
        if let Some(location) = failure_location(result) {
            ui.label(format!(
                "失败位置：{} · 第 {} 行",
                location.node, location.line
            ));
            *failure_jump |= ui
                .add_enabled(
                    debugger.result_version == Some(current_version),
                    egui::Button::new("跳转到失败位置"),
                )
                .on_disabled_hover_text("编辑稿已变化，请重新重放后定位")
                .clicked();
        } else if matches!(
            result.status,
            ReplayStatus::Diverged { .. } | ReplayStatus::StoryFailed { .. }
        ) {
            ui.label("当前停止位置不可定位，原记录仅供对比");
        }
        egui::CollapsingHeader::new("步骤前后状态差异")
            .default_open(true)
            .show(ui, |ui| {
                if result.state_diff.is_empty() {
                    ui.label("状态没有差异");
                }
                for (key, value) in &result.state_diff {
                    ui.monospace(key);
                    ui.add(egui::Label::new(value.to_string()).wrap());
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

pub(super) fn render_debugger_compact(ui: &mut egui::Ui, debugger: &ReplayDebugger) {
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
