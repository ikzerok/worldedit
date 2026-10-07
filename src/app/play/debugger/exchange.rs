//! 路径 JSON 只通过 runtime 的有界交换接口；失败不产生半份记录或伪 JSON。
use crate::app::{ReplayDebugger, SavedReplayPath};
use crate::theme;
use worldline_runtime::{
    decode_replay_trace, encode_replay_trace, ReplayTrace, MAX_REPLAY_EXCHANGE_BYTES,
    MAX_REPLAY_EXCHANGE_STEPS,
};

const MAX_SAVED_REPLAY_PATHS: usize = 64;

fn capacity_error(debugger: &ReplayDebugger) -> Option<String> {
    (debugger.saved_paths.len() >= MAX_SAVED_REPLAY_PATHS)
        .then(|| format!("当前会话最多保留 {MAX_SAVED_REPLAY_PATHS} 条路径"))
}

// 焦点交接可跨多个按住按键的帧；这里只做常量时间预检，不序列化整份轨迹。
pub(super) fn record_focus_unavailable(
    debugger: &ReplayDebugger,
    trace: &ReplayTrace,
) -> Option<String> {
    capacity_error(debugger).or_else(|| {
        (trace.steps.len() > MAX_REPLAY_EXCHANGE_STEPS)
            .then(|| "路径保存失败：step_limit：路径交换超过 20,000 步上限".into())
    })
}

fn record_unavailable(debugger: &ReplayDebugger, trace: &ReplayTrace) -> Option<String> {
    capacity_error(debugger).or_else(|| {
        encode_replay_trace(trace)
            .err()
            .map(|error| format!("路径保存失败：{error}"))
    })
}

impl ReplayDebugger {
    pub(in crate::app) fn select_replay_path(&mut self, selected: Option<usize>) {
        if self.selected_path != selected {
            self.export_json.clear();
            self.selected_path = selected;
        }
    }

    pub(in crate::app) fn record_replay_path(&mut self, trace: ReplayTrace) {
        if let Some(error) = record_unavailable(self, &trace) {
            self.notice = Some(error);
            return;
        }
        let name = if self.path_name.trim().is_empty() {
            format!("路径 {}", self.saved_paths.len() + 1)
        } else {
            self.path_name.trim().to_owned()
        };
        self.saved_paths.push(SavedReplayPath {
            name: name.clone(),
            trace,
        });
        self.select_replay_path(Some(self.saved_paths.len() - 1));
        self.path_name = format!("路径 {}", self.saved_paths.len() + 1);
        self.notice = Some(format!("已在本次工作台会话中保存路径“{name}”"));
    }

    pub(in crate::app) fn import_replay_path(&mut self) {
        let imported = capacity_error(self).map_or_else(
            || {
                decode_replay_trace(self.import_json.as_bytes())
                    .map_err(|error| format!("路径导入失败：{error}"))
            },
            Err,
        );
        match imported {
            Ok(trace) => {
                let name = format!("导入路径 {}", self.saved_paths.len() + 1);
                self.saved_paths.push(SavedReplayPath { name, trace });
                self.select_replay_path(Some(self.saved_paths.len() - 1));
                self.notice = Some(
                    "路径格式通过检查，已加入当前调试会话；可只读查看，重放仍须通过版本与指纹校验"
                        .into(),
                );
            }
            Err(error) => self.notice = Some(error),
        }
    }

    pub(in crate::app) fn export_replay_path(&mut self) {
        let encoded = self
            .selected_path
            .and_then(|index| self.saved_paths.get(index))
            .ok_or_else(|| "请先选择一条有效路径".to_owned())
            .and_then(|path| encode_replay_trace(&path.trace).map_err(|error| error.to_string()));
        match encoded {
            Ok(json) => {
                self.export_json = json;
                self.notice =
                    Some("已生成可重新导入的紧凑路径 JSON；复制并另存才会保留交换副本".into());
            }
            Err(error) => {
                self.export_json.clear();
                self.notice = Some(format!("路径导出失败：{error}；原记录保留"));
            }
        }
    }
}

pub(in crate::app::play) fn render_trace_import(ui: &mut egui::Ui, debugger: &mut ReplayDebugger) {
    egui::CollapsingHeader::new("导入路径 JSON（最多 4 MiB / 20,000 步）")
        .default_open(false)
        .show(ui, |ui| {
            if debugger.import_json.len() > MAX_REPLAY_EXCHANGE_BYTES {
                ui.colored_label(
                    theme::ERROR(),
                    "轨迹 JSON 超过 4 MiB UTF-8 字节边界；内容已拒绝导入。",
                );
                if ui.button("清除超限输入").clicked() {
                    debugger.import_json.clear();
                }
                if ui.button("拒绝超限轨迹").clicked() {
                    debugger.import_replay_path();
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
                    debugger.import_replay_path();
                }
            }
        });
}
