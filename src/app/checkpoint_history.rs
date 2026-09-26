//! 工程检查点历史；写入与恢复均通过 Project 的公开 API。
use super::WorldeditApp;
use crate::theme;
use egui::{RichText, ScrollArea};
use worldline_core::project::{
    CheckpointFileOperation, CheckpointRestorePlan, CheckpointSummary, Project,
};

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

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("检查点历史");
            ui.label(theme::muted(format!(
                "{} 条记录 · {} · 恢复会写入工程目录",
                records.len(),
                format_bytes(records.iter().map(|record| record.payload_bytes).sum())
            )));
            #[cfg(not(target_arch = "wasm32"))]
            ui.label(theme::muted(
                "桌面端记录保存在工程的 .world/.checkpoints/v1；不进入工程导出包。",
            ));
            #[cfg(target_arch = "wasm32")]
            ui.label(theme::muted(
                "浏览器记录只在当前模块运行期间保留；刷新页面后不保证仍可用。",
            ));
            ui.label(theme::muted(
                "默认上限：20 条、单条 64 MiB、历史 256 MiB；不会自动删除旧记录。",
            ));

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("新检查点标签");
                ui.add(
                    egui::TextEdit::singleline(&mut self.checkpoint_history.label)
                        .hint_text("可选，最多 120 字"),
                );
                if ui.button("创建检查点").clicked() {
                    action = Some(Action::Create(self.checkpoint_history.label.clone()));
                }
            });
            ui.horizontal(|ui| {
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
                    draw_history_list(
                        &mut columns[0],
                        &records,
                        &mut self.checkpoint_history,
                        &mut action,
                    );
                    draw_selected_record(
                        &mut columns[1],
                        &records,
                        &self.checkpoint_history,
                        &self.project,
                        &mut action,
                    );
                });
            } else {
                ScrollArea::vertical()
                    .id_salt("checkpoint-history-narrow")
                    .show(ui, |ui| {
                        draw_history_list(ui, &records, &mut self.checkpoint_history, &mut action);
                        ui.separator();
                        draw_selected_record(
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
                        self.checkpoint_history.notice =
                            Some("检查点已创建；此操作没有保存或修改当前正文。".into());
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
            }
            Err(error) => self.checkpoint_history.error = Some(error),
        }
    }
}

fn draw_history_list(
    ui: &mut egui::Ui,
    records: &[CheckpointSummary],
    state: &mut HistoryState,
    action: &mut Option<Action>,
) {
    ui.heading("记录");
    ScrollArea::vertical()
        .id_salt("checkpoint-history-list")
        .max_height(420.0)
        .show(ui, |ui| {
            let query = state.query.trim().to_lowercase();
            let mut visible = 0;
            for record in records.iter().filter(|record| {
                query.is_empty()
                    || record.id.to_lowercase().contains(&query)
                    || record
                        .label
                        .as_deref()
                        .is_some_and(|label| label.to_lowercase().contains(&query))
            }) {
                visible += 1;
                let title = record.label.as_deref().unwrap_or("未命名检查点");
                let status = if record.available {
                    "可恢复"
                } else {
                    "不可用"
                };
                let row = format!(
                    "{title} · {} · {} 个文件 · {}",
                    format_time(record.created_at_unix_ms),
                    record.file_count,
                    status
                );
                ui.push_id(&record.id, |ui| {
                    let selected = state.selected_id.as_deref() == Some(&record.id);
                    if ui
                        .add_sized(
                            [ui.available_width(), 42.0],
                            egui::Button::selectable(selected, row),
                        )
                        .clicked()
                    {
                        state.selected_id = Some(record.id.clone());
                        state.preview = None;
                        state.restore_confirmation = false;
                    }
                });
            }
            if visible == 0 {
                ui.label(theme::muted(if records.is_empty() {
                    "尚无检查点；创建操作会捕获当前缓冲和工作区文件。"
                } else {
                    "没有符合筛选条件的记录。"
                }));
            }
        });

    if let Some(id) = &state.selected_id {
        if let Some(record) = records.iter().find(|record| &record.id == id) {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(record.available, theme::primary("预览恢复…"))
                    .clicked()
                {
                    *action = Some(Action::Preview(record.id.clone()));
                }
                if ui.button("删除历史记录…").clicked() {
                    *action = Some(Action::Delete(record.id.clone()));
                }
            });
        }
    }
}

fn draw_selected_record(
    ui: &mut egui::Ui,
    records: &[CheckpointSummary],
    state: &HistoryState,
    project: &Project,
    action: &mut Option<Action>,
) {
    ui.heading("所选记录");
    let Some(record) = state
        .selected_id
        .as_deref()
        .and_then(|id| records.iter().find(|record| record.id == id))
    else {
        ui.label(theme::muted("选择一条记录查看详情。"));
        return;
    };
    ui.label(format!(
        "标签：{}",
        record.label.as_deref().unwrap_or("未命名")
    ));
    ui.label(format!("时间：{}", format_time(record.created_at_unix_ms)));
    ui.label(format!("ID：{}", record.id));
    ui.label(format!(
        "{} 个文件 · {}",
        record.file_count,
        format_bytes(record.payload_bytes)
    ));
    if !record.available {
        ui.colored_label(
            theme::ERROR,
            format!(
                "此记录不可恢复：{}",
                record.unavailable_reason.as_deref().unwrap_or("原因未知")
            ),
        );
    } else {
        ui.label(theme::muted("预览与筛选只读；只有确认恢复才会写入工作区。"));
    }
    if let Some(plan) = state
        .preview
        .as_ref()
        .filter(|plan| plan.checkpoint_id == record.id)
    {
        draw_plan_summary(ui, plan, project);
        if state.restore_confirmation {
            draw_restore_confirmation(ui, project, plan, action);
        } else if ui.button("打开恢复确认…").clicked() {
            *action = Some(Action::OpenRestoreConfirmation);
        }
    }
    if state.pending_delete.as_deref() == Some(record.id.as_str()) {
        ui.group(|ui| {
            ui.heading("确认删除检查点记录？");
            ui.label(format!(
                "将删除“{}”及其历史快照。",
                record.label.as_deref().unwrap_or("未命名检查点")
            ));
            ui.label(theme::muted("这不会删除或更改当前工程文件。"));
            ui.horizontal(|ui| {
                if ui.button("确认删除历史记录").clicked() {
                    *action = Some(Action::ConfirmDelete);
                }
                if ui.button("取消删除").clicked() {
                    *action = Some(Action::CancelDelete);
                }
            });
        });
    }
}

fn draw_plan_summary(ui: &mut egui::Ui, plan: &CheckpointRestorePlan, project: &Project) {
    ui.separator();
    ui.heading(format!("恢复预览 · {} 项文件变化", plan.changes.len()));
    ui.label(theme::muted(
        "差异粒度：core 公开 API 提供逐文件操作、字节数和受影响对象；此处不提供逐行文本差异。",
    ));
    if project.is_dirty() {
        ui.colored_label(theme::ERROR, "当前有未保存草稿；确认后会被检查点内容替换。");
    }
    ScrollArea::vertical()
        .id_salt("checkpoint-change-list")
        .max_height(300.0)
        .show(ui, |ui| {
            if plan.changes.is_empty() {
                ui.label("检查点与当前工作区内容相同。");
            }
            for change in &plan.changes {
                let operation = match change.operation {
                    CheckpointFileOperation::Added => "新增",
                    CheckpointFileOperation::Modified => "修改",
                    CheckpointFileOperation::Deleted => "删除",
                };
                ui.group(|ui| {
                    ui.label(
                        RichText::new(format!("{operation} · {}", change.path.display())).strong(),
                    );
                    ui.label(format!(
                        "当前 {} → 检查点 {}",
                        optional_bytes(change.current_bytes),
                        optional_bytes(change.checkpoint_bytes)
                    ));
                    if change.affected_objects.is_empty() {
                        ui.label(theme::muted("未报告关联对象。"));
                    } else {
                        let objects = change
                            .affected_objects
                            .iter()
                            .map(|target| format!("{}:{}", target.kind, target.id))
                            .collect::<Vec<_>>()
                            .join("、");
                        ui.label(format!("关联对象：{objects}"));
                    }
                    if !change.objects_complete {
                        ui.colored_label(
                            theme::ERROR,
                            "对象影响分析不完整；恢复不会因此自动删除或改写对象引用。",
                        );
                    }
                });
            }
        });
}

fn draw_restore_confirmation(
    ui: &mut egui::Ui,
    project: &Project,
    plan: &CheckpointRestorePlan,
    action: &mut Option<Action>,
) {
    let baseline_current = project.content_baseline() == plan.expected_content_baseline;
    ui.group(|ui| {
        ui.heading("确认恢复检查点？");
        ui.label(format!(
            "检查点 {} 将应用 {} 项文件变化。",
            plan.checkpoint_id,
            plan.changes.len()
        ));
        ui.label(theme::muted(
            "确认后会写入工程目录；恢复预览只显示文件级差异和对象影响。",
        ));
        if !baseline_current {
            ui.colored_label(
                theme::ERROR,
                "此预览已过期：工程缓冲发生变化。请取消并重新预览。",
            );
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(baseline_current, theme::primary("确认恢复此工程检查点"))
                .clicked()
            {
                *action = Some(Action::ConfirmRestore);
            }
            if ui.button("取消恢复").clicked() {
                *action = Some(Action::CancelRestore);
            }
        });
    });
}

fn optional_bytes(bytes: Option<u64>) -> String {
    bytes.map(format_bytes).unwrap_or_else(|| "无".into())
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
