//! 未应用输入与工程落盘是独立真值，不能用一个“脏”点或互斥分支表示。
use super::*;

const SAVE_SCOPE: &str = "保存全部只保存已应用的工程内容，不自动应用输入，也不改变试玩结果。";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SaveState {
    pub drafts: bool,
    pub project: bool,
}

impl SaveState {
    pub(super) fn label(self) -> &'static str {
        match (self.drafts, self.project) {
            (false, false) => "工程已保存",
            (true, false) => "输入待应用 · 工程已保存",
            (false, true) => "工程待保存",
            (true, true) => "输入待应用 · 工程待保存",
        }
    }

    pub(super) fn stacked_label(self) -> &'static str {
        match (self.drafts, self.project) {
            (true, true) => "输入待应用\n工程待保存",
            (true, false) => "输入待应用\n工程已保存",
            _ => self.label(),
        }
    }

    pub(super) fn footer(self, compact: bool) -> &'static str {
        match (self.drafts, self.project, compact) {
            (false, false, false) => "全部文件已保存",
            (false, false, true) => "已保存",
            (true, false, true) => "待应用 · 已保存",
            (false, true, true) => "待保存",
            (true, true, true) => "待应用 · 待保存",
            _ => self.label(),
        }
    }
}

impl WorldeditApp {
    pub(super) fn save_state(&self) -> SaveState {
        SaveState {
            drafts: self.has_open_authoring_form(),
            project: self.project.is_dirty(),
        }
    }

    pub(super) fn save_state_menu(&self, ui: &mut egui::Ui, stacked: bool) {
        let state = self.save_state();
        let response = ui
            .push_id("global-save-state", |ui| {
                crate::chrome::quiet_menu(
                    ui,
                    if stacked {
                        state.stacked_label()
                    } else {
                        state.label()
                    },
                    |ui| {
                        ui.set_max_width((ui.ctx().screen_rect().width() - 48.0).max(160.0));
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                        ui.strong("输入与工程保存");
                        egui::ScrollArea::vertical()
                            .id_salt("global-save-state-details")
                            .max_height((ui.ctx().screen_rect().height() - 80.0).max(100.0))
                            .show(ui, |ui| {
                                ui.label(if state.project {
                                    "工程：有已应用修改待保存"
                                } else {
                                    "工程：已应用内容已保存"
                                });
                                let drafts = self.dirty_draft_names();
                                if drafts.is_empty() {
                                    ui.label("输入：没有待应用草稿");
                                } else {
                                    ui.label("输入：以下草稿待应用");
                                    for name in drafts {
                                        ui.label(format!("• {name}"));
                                    }
                                }
                                ui.separator();
                                ui.label(SAVE_SCOPE);
                                if let Some(error) = &self.io_error {
                                    ui.colored_label(
                                        theme::ERROR(),
                                        format!("保存或操作未完成：{error}"),
                                    );
                                }
                            });
                    },
                )
                .response
            })
            .inner;
        crate::chrome::action_layout::record(ui, "global/save-state", &response);
    }
}
