//! 工程检查点历史；写入与恢复均通过 Project 的公开 API。
mod list;
mod recovery;

use super::WorldeditApp;
use crate::theme;
use egui::ScrollArea;
use worldline_core::project::CheckpointRestorePlan;

#[derive(Default)]
pub(super) struct HistoryState {
    pub(super) query: String,
    pub(super) label: String,
    pub(super) selected_id: Option<String>,
    pub(super) preview: Option<CheckpointRestorePlan>,
    pub(super) pending_delete: Option<String>,
    pub(super) restore_confirmation: bool,
    pub(super) error: Option<String>,
    pub(super) notice: Option<String>,
}

#[derive(Debug)]
enum Action {
    Create(String),
    Preview(String),
    Delete(String),
    OpenRestoreConfirmation,
    ConfirmRestore,
    CancelRestore,
    ConfirmDelete,
    CancelDelete,
}

impl WorldeditApp {
    pub(super) fn checkpoint_history_tab(&mut self, ctx: &egui::Context) {
        let records = match self.project.list_checkpoints() {
            Ok(records) => records,
            Err(error) => {
                self.checkpoint_history.error = Some(error);
                Vec::new()
            }
        };
        let mut action = None;
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            if self.checkpoint_history.restore_confirmation {
                action = Some(Action::CancelRestore);
            } else if self.checkpoint_history.pending_delete.is_some() {
                action = Some(Action::CancelDelete);
            }
        }

        egui::CentralPanel::default().frame(theme::panel().fill(theme::BG)).show(ctx, |ui| {
            theme::page_heading(ui, "检查点历史", &format!(
                "{} 条记录 · {} · 恢复会写入工程目录",
                records.len(),
                format_bytes(records.iter().map(|record| record.payload_bytes).sum())
            ));
            #[cfg(not(target_arch = "wasm32"))]
            ui.label(theme::muted(
                "桌面端记录保存在工程的 .world/.checkpoints/v1；不进入工程导出包。",
            ));
            #[cfg(target_arch = "wasm32")]
            ui.label(theme::muted(
                "检查点内容与工程快照写入此浏览器的本地存储；若写入失败，当前脏稿仍保留，可导出工程恢复副本。",
            ));
            ui.label(theme::muted(
                "默认上限：20 条、单条 64 MiB、历史 256 MiB；不会自动删除旧记录。",
            ));

            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label("新检查点标签");
                ui.add(
                    egui::TextEdit::singleline(&mut self.checkpoint_history.label)
                        .hint_text("可选，最多 120 字"),
                );
                if ui.button("创建检查点").clicked() {
                    action = Some(Action::Create(self.checkpoint_history.label.clone()));
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("筛选");
                ui.add(
                    egui::TextEdit::singleline(&mut self.checkpoint_history.query)
                        .hint_text("标签或检查点 ID"),
                );
            });

            if let Some(error) = &self.checkpoint_history.error {
                ui.colored_label(theme::ERROR, error);
            }
            if let Some(notice) = &self.checkpoint_history.notice {
                ui.label(theme::muted(notice));
            }

            let available_width = ui.available_width();
            if available_width >= 820.0 {
                ui.columns(2, |columns| {
                    list::draw_history_list(
                        &mut columns[0],
                        &records,
                        &mut self.checkpoint_history,
                        &mut action,
                    );
                    ScrollArea::vertical()
                        .id_salt("checkpoint-history-details")
                        .show(&mut columns[1], |ui| {
                            list::draw_selected_record(
                                ui,
                                &records,
                                &self.checkpoint_history,
                                &self.project,
                                &mut action,
                            );
                        });
                });
            } else {
                ScrollArea::vertical()
                    .id_salt("checkpoint-history-narrow")
                    .show(ui, |ui| {
                        list::draw_history_list(ui, &records, &mut self.checkpoint_history, &mut action);
                        ui.separator();
                        list::draw_selected_record(
                            ui,
                            &records,
                            &self.checkpoint_history,
                            &self.project,
                            &mut action,
                        );
                    });
            }
        });

        match action {
            Some(Action::Create(label)) => {
                let label = (!label.trim().is_empty()).then(|| label.trim().to_owned());
                match self
                    .project
                    .create_checkpoint(label, worldline_core::project::CheckpointLimits::default())
                {
                    Ok(summary) => {
                        self.checkpoint_history.selected_id = Some(summary.id);
                        self.checkpoint_history.label.clear();
                        self.checkpoint_history.preview = None;
                        self.checkpoint_history.error = None;
                        #[cfg(target_arch = "wasm32")]
                        {
                            match self.persist_browser_checkpoint_state() {
                                Ok(()) => {
                                    self.io_error = None;
                                    self.checkpoint_history.notice = Some(
                                        "检查点和当前工程快照已保存到此浏览器；没有请求下载。"
                                            .into(),
                                    );
                                }
                                Err(error) => {
                                    self.browser_pending_save = true;
                                    self.checkpoint_history.error = Some(format!(
                                        "浏览器持久化失败，检查点和脏稿仍保留在本次会话中。可点击“导出工程”下载恢复副本；下载不代表已持久化。{error}"
                                    ));
                                }
                            }
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            self.checkpoint_history.notice =
                                Some("检查点已创建；此操作没有保存或修改当前正文。".into());
                        }
                    }
                    Err(error) => self.checkpoint_history.error = Some(error),
                }
            }
            Some(Action::Preview(id)) => match self.project.preview_checkpoint_restore(&id) {
                Ok(plan) => {
                    self.checkpoint_history.selected_id = Some(id);
                    self.checkpoint_history.preview = Some(plan);
                    self.checkpoint_history.error = None;
                }
                Err(error) => {
                    self.checkpoint_history.preview = None;
                    self.checkpoint_history.error = Some(error);
                }
            },
            Some(Action::Delete(id)) => {
                self.checkpoint_history.pending_delete = Some(id);
            }
            Some(Action::OpenRestoreConfirmation) => {
                self.checkpoint_history.restore_confirmation = true;
            }
            Some(Action::CancelRestore) => {
                self.checkpoint_history.restore_confirmation = false;
                self.checkpoint_history.preview = None;
                self.checkpoint_history.notice = Some("已取消恢复；当前工程草稿保持不变。".into());
            }
            Some(Action::ConfirmRestore) => {
                self.checkpoint_history.restore_confirmation = false;
                self.confirm_restore();
            }
            Some(Action::CancelDelete) => {
                self.checkpoint_history.pending_delete = None;
            }
            Some(Action::ConfirmDelete) => self.confirm_delete(),
            None => {}
        }
    }

    fn confirm_restore(&mut self) {
        let Some(plan) = self.checkpoint_history.preview.clone() else {
            return;
        };
        match self.project.preview_checkpoint_restore(&plan.checkpoint_id) {
            Ok(current) if current == plan => {}
            Ok(_) => {
                self.checkpoint_history.preview = None;
                self.checkpoint_history.error =
                    Some("恢复预览已过期；请重新查看最新变化后再确认。".into());
                return;
            }
            Err(error) => {
                self.checkpoint_history.preview = None;
                self.checkpoint_history.error = Some(format!(
                    "恢复预览已失效：{error}。当前草稿未改变，请重新预览。"
                ));
                return;
            }
        }

        let before = self.project.clone();
        match self.project.restore_checkpoint(&plan) {
            Ok(result) => {
                self.remember(before);
                self.recompile();
                self.checkpoint_history.preview = None;
                self.checkpoint_history.error = None;
                self.checkpoint_history.notice = Some(format!(
                    "已恢复检查点 {} 的 {} 个文件；可用撤销回到恢复前草稿。",
                    plan.checkpoint_id, result.restored_files
                ));
                #[cfg(target_arch = "wasm32")]
                if let Err(error) = self.persist_browser_checkpoint_state() {
                    self.browser_pending_save = true;
                    self.checkpoint_history.error = Some(format!(
                        "检查点已恢复，但浏览器未能持久化恢复后的快照。当前恢复稿仍保留，可点击“导出工程”下载恢复副本；下载不代表已持久化。{error}"
                    ));
                }
            }
            Err(error) => {
                self.checkpoint_history.error = Some(format!("恢复失败，当前草稿已保留：{error}"));
            }
        }
    }

    fn confirm_delete(&mut self) {
        let Some(id) = self.checkpoint_history.pending_delete.take() else {
            return;
        };
        match self.project.delete_checkpoint(&id) {
            Ok(()) => {
                if self.checkpoint_history.selected_id.as_deref() == Some(id.as_str()) {
                    self.checkpoint_history.selected_id = None;
                }
                self.checkpoint_history.restore_confirmation = false;
                if self
                    .checkpoint_history
                    .preview
                    .as_ref()
                    .is_some_and(|plan| plan.checkpoint_id == id)
                {
                    self.checkpoint_history.preview = None;
                }
                self.checkpoint_history.error = None;
                self.checkpoint_history.notice =
                    Some("检查点记录已删除；工程源码和当前草稿没有变化。".into());
                #[cfg(target_arch = "wasm32")]
                if let Err(error) = self.persist_browser_checkpoint_state() {
                    self.browser_pending_save = true;
                    self.checkpoint_history.error = Some(format!(
                        "本次会话已删除此记录，但浏览器历史未能更新；刷新后旧记录可能仍出现。当前稿仍保留。{error}"
                    ));
                }
            }
            Err(error) => self.checkpoint_history.error = Some(error),
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn format_time(unix_ms: u64) -> String {
    let seconds = (unix_ms / 1000).min(i64::MAX as u64) as i64;
    let days = seconds.div_euclid(86_400);
    let day_seconds = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let hour = day_seconds / 3_600;
    let minute = day_seconds % 3_600 / 60;
    let second = day_seconds % 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02} UTC")
}
