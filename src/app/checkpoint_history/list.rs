use super::recovery;
use super::{format_bytes, format_time, Action, HistoryState};
use crate::theme;
use egui::ScrollArea;
use worldline_core::project::{CheckpointSummary, Project};
pub(super) fn draw_history_list(
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
                if crate::theme::add_enabled(ui, record.available, theme::primary("预览恢复…"))
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

pub(super) fn draw_selected_record(
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
            theme::ERROR(),
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
        recovery::draw_plan_summary(ui, plan, project);
        if state.restore_confirmation {
            recovery::draw_restore_confirmation(ui, project, plan, action);
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
