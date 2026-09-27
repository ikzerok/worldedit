use super::{format_bytes, Action};
use crate::theme;
use egui::{RichText, ScrollArea};
use worldline_core::project::{
    CheckpointFileOperation, CheckpointRestorePlan, CheckpointTextDiff, Project,
};

pub(super) fn draw_plan_summary(
    ui: &mut egui::Ui,
    plan: &CheckpointRestorePlan,
    project: &Project,
) {
    ui.separator();
    ui.heading(format!("恢复预览 · {} 项文件变化", plan.changes.len()));
    ui.label(theme::muted(
        "core 公开 API 提供逐文件操作、对象影响与三方文本投影；base 是检查点创建时捕获的保存基线，不代表共同祖先或自动合并方案。",
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
    if !plan.text_differences.is_empty() || plan.text_differences_truncated {
        ui.separator();
        ui.heading(format!(
            "三方文本差异 · {} 个源码文件",
            plan.text_differences.len()
        ));
        if plan.text_differences_truncated {
            ui.colored_label(
                theme::ERROR,
                "源码文件超过 core 投影上限；其余文本差异未显示。",
            );
        }
        ScrollArea::vertical()
            .id_salt("checkpoint-text-differences")
            .max_height(260.0)
            .show(ui, |ui| {
                for difference in &plan.text_differences {
                    draw_text_difference(ui, difference);
                }
            });
    }
}

fn draw_text_difference(ui: &mut egui::Ui, difference: &CheckpointTextDiff) {
    let title = format!("文本差异：{}", difference.path.display());
    egui::CollapsingHeader::new(title)
        .id_salt(("checkpoint-text-difference", &difference.path))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(theme::muted(format!(
                "{} 项差异 · base {} 行 / {} · current {} 行 / {} · checkpoint {} 行 / {}",
                difference.summary.difference_count,
                optional_count(difference.summary.base_lines),
                optional_bytes(difference.summary.base_bytes),
                optional_count(difference.summary.current_lines),
                optional_bytes(difference.summary.current_bytes),
                optional_count(difference.summary.checkpoint_lines),
                optional_bytes(difference.summary.checkpoint_bytes)
            )));
            if !difference.base_available {
                ui.colored_label(
                    theme::ERROR,
                    "没有可靠的已保存基线；只展示 current/checkpoint 原文，不推断三方差异。",
                );
            }
            if difference.alignment_uncertain {
                ui.colored_label(theme::ERROR, "段落无法可靠对齐；下方内容是有界原文片段。");
            }
            if difference.undecodable {
                ui.colored_label(
                    theme::ERROR,
                    "存在无效 UTF-8；下方以十六进制字节显示，未生成范围。",
                );
            }
            if difference.truncated {
                ui.colored_label(theme::ERROR, "文本投影达到大小或段落上限，显示内容已截断。");
            }

            if difference.alignment_uncertain || difference.undecodable {
                draw_text_triplet(
                    ui,
                    "base",
                    difference.raw.base.as_deref(),
                    "current",
                    difference.raw.current.as_deref(),
                    "checkpoint",
                    difference.raw.checkpoint.as_deref(),
                );
            } else {
                for (index, change) in difference.differences.iter().enumerate() {
                    ui.group(|ui| {
                        ui.label(theme::muted(format!("段落差异 {}", index + 1)));
                        draw_text_triplet(
                            ui,
                            "base",
                            change.base.as_deref(),
                            "current",
                            change.current.as_deref(),
                            "checkpoint",
                            change.checkpoint.as_deref(),
                        );
                    });
                }
            }
        });
}

fn draw_text_triplet(
    ui: &mut egui::Ui,
    base_label: &str,
    base: Option<&str>,
    current_label: &str,
    current: Option<&str>,
    checkpoint_label: &str,
    checkpoint: Option<&str>,
) {
    ui.columns(3, |columns| {
        draw_text_side(&mut columns[0], base_label, base);
        draw_text_side(&mut columns[1], current_label, current);
        draw_text_side(&mut columns[2], checkpoint_label, checkpoint);
    });
}

fn draw_text_side(ui: &mut egui::Ui, label: &str, text: Option<&str>) {
    ui.label(RichText::new(label).strong());
    let Some(text) = text else {
        ui.label(theme::muted("无匹配文本 / 文件侧缺失"));
        return;
    };
    let mut snippet = text.to_owned();
    ui.add(
        egui::TextEdit::multiline(&mut snippet)
            .font(egui::TextStyle::Monospace)
            .desired_rows(3)
            .desired_width(ui.available_width())
            .interactive(false),
    );
}

fn optional_count(value: Option<usize>) -> String {
    value.map_or_else(|| "未知".into(), |value| value.to_string())
}

pub(super) fn draw_restore_confirmation(
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
