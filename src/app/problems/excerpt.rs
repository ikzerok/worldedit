//! 命中摘录只使用core有界切片与局部范围；省略说明绝不混进原文字节。
use super::view::{detail_button, focus_action, reading_job, reading_label, role_label};
use crate::{app::personal::Settings, theme};
use worldline_core::problems::{ProblemContextVisibility, ProblemLocation};

pub(super) fn source_excerpt(
    ui: &mut egui::Ui,
    scope: egui::Id,
    location: &ProblemLocation,
    settings: &Settings,
) {
    let Some(context) = location.context.as_ref().filter(|context| context.version == 1) else {
        if let Some(excerpt) = &location.excerpt {
            reading_label(ui, excerpt, settings, true);
        }
        reading_label(ui, "旧版或未知上下文仅供阅读；请刷新检查后定位", settings, false);
        if location.excerpt_truncated {
            reading_label(ui, "摘录已限长，完整原文请打开来源", settings, false);
        }
        return;
    };
    ui.label(theme::muted(format!("来源证据 · {}", role_label(context.role))));
    if context.prefix_clipped {
        reading_label(ui, "⋯ 前文已省略", settings, false);
    }
    if let Some(text) = &context.text {
        let mut job = reading_job(text, settings, true);
        if let Some(hit) = &context.hit_byte_range {
            if hit.start <= hit.end && text.get(hit.start..hit.end).is_some() {
                let format = job.sections[0].format.clone();
                job.sections.clear();
                for (range, marked) in [(0..hit.start, false), (hit.start..hit.end, true), (hit.end..text.len(), false)] {
                    if range.is_empty() { continue; }
                    let mut format = format.clone();
                    if marked {
                        format.background = theme::problem_source_background();
                        format.underline = egui::Stroke::new(1_f32, theme::ACCENT());
                    }
                    job.sections.push(egui::text::LayoutSection { leading_space: 0., byte_range: range, format });
                }
            }
        }
        ui.add(egui::Label::new(job).wrap());
        if focus_action(detail_button(ui, scope.with("copy-excerpt"), "复制原文摘录", true)) {
            ui.ctx().copy_text(text.clone());
        }
    }
    if context.suffix_clipped {
        reading_label(ui, "后文已省略 ⋯", settings, false);
    }
    match context.visibility {
        ProblemContextVisibility::Partial => reading_label(ui, "命中或文档超过摘录预算，当前仅显示可见部分", settings, false),
        ProblemContextVisibility::NoText => reading_label(ui, "当前预算或来源状态没有可显示摘录；位置角色仍保留", settings, false),
        ProblemContextVisibility::Full => {}
    }
}
