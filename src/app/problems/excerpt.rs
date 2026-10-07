//! 命中摘录只使用core有界切片与局部范围；省略说明绝不混进原文字节。
use super::view::{detail_button, focus_action, reading_job, reading_label, role_label};
use crate::theme::{self, AppearancePreferences};
use worldline_core::problems::{ProblemContextVisibility, ProblemLocation};

pub(super) fn source_excerpt(
    ui: &mut egui::Ui,
    scope: egui::Id,
    location: &ProblemLocation,
    settings: &AppearancePreferences,
) {
    let Some(context) = location
        .context
        .as_ref()
        .filter(|context| context.version == 1)
    else {
        if let Some(excerpt) = &location.excerpt {
            reading_label(ui, excerpt, settings, true);
        }
        reading_label(
            ui,
            "旧版或未知上下文仅供阅读；请刷新检查后定位",
            settings,
            false,
        );
        if location.excerpt_truncated {
            reading_label(ui, "摘录已限长，完整原文请打开来源", settings, false);
        }
        if let Some(excerpt) = &location.excerpt {
            if focus_action(detail_button(
                ui,
                scope.with("copy-excerpt"),
                "复制原文摘录",
                true,
            )) {
                ui.ctx().copy_text(excerpt.clone());
            }
        }
        return;
    };
    ui.label(theme::muted(format!(
        "来源证据 · {}",
        role_label(context.role)
    )));
    if context.prefix_clipped {
        reading_label(ui, "⋯ 前文已省略", settings, false);
    }
    if let Some(text) = &context.text {
        let mut job = reading_job(text, settings, true);
        if let Some(hit) = &context.hit_byte_range {
            if !text.is_empty() && hit.start <= hit.end && text.get(hit.start..hit.end).is_some() {
                let format = job.sections[0].format.clone();
                job.sections.clear();
                for (range, marked) in [
                    (0..hit.start, false),
                    (hit.start..hit.end, true),
                    (hit.end..text.len(), false),
                ] {
                    if range.is_empty() {
                        continue;
                    }
                    let mut format = format.clone();
                    if marked {
                        format.background = theme::problem_source_background();
                        format.underline = egui::Stroke::new(1_f32, theme::ACCENT());
                    }
                    job.sections.push(egui::text::LayoutSection {
                        leading_space: 0.,
                        byte_range: range,
                        format,
                    });
                }
            }
        }
        ui.add(egui::Label::new(job).wrap());
        if focus_action(detail_button(
            ui,
            scope.with("copy-excerpt"),
            "复制原文摘录",
            true,
        )) {
            ui.ctx().copy_text(text.clone());
        }
    }
    if context.suffix_clipped {
        reading_label(ui, "后文已省略 ⋯", settings, false);
    }
    match context.visibility {
        ProblemContextVisibility::Partial => reading_label(
            ui,
            if location.precision == worldline_core::problems::ProblemPrecision::Document {
                "文档超过摘录预算，当前仅显示部分原文"
            } else {
                "命中超过摘录预算，当前仅显示可见交集"
            },
            settings,
            false,
        ),
        ProblemContextVisibility::NoText => reading_label(
            ui,
            "当前预算或来源状态没有可显示摘录；位置角色仍保留",
            settings,
            false,
        ),
        ProblemContextVisibility::Full => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldline_core::problems::{
        ProblemPrecision, ProblemRange, ProblemSourceContext, ProblemSourceRole,
    };

    fn location() -> ProblemLocation {
        let text = "同名 missing / 真因 missing / 同名 missing".to_owned();
        let start = text.find("真因").unwrap();
        let end = start + "真因 missing".len();
        ProblemLocation {
            path: Some("来源.wl".into()),
            precision: ProblemPrecision::Span,
            span: None,
            byte_range: None,
            char_range: None,
            excerpt: Some(text.clone()),
            excerpt_truncated: true,
            reason: None,
            context: Some(ProblemSourceContext {
                version: 1,
                role: ProblemSourceRole::Expression,
                text: Some(text.clone()),
                slice_byte_range: None,
                slice_char_range: None,
                hit_byte_range: Some(ProblemRange { start, end }),
                hit_char_range: None,
                visibility: ProblemContextVisibility::Partial,
                prefix_clipped: true,
                suffix_clipped: true,
            }),
        }
    }

    fn paint(location: &ProblemLocation, settings: &AppearancePreferences) -> egui::FullOutput {
        let ctx = egui::Context::default();
        ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                source_excerpt(ui, egui::Id::new("excerpt-test"), location, settings);
            });
        })
    }

    #[test]
    fn excerpt_uses_exact_local_hit_and_reading_size_without_inserting_ellipsis() {
        let location = location();
        let settings = AppearancePreferences {
            body_size: 28.,
            source_size: 28.,
            line_spacing: 1.8,
            ..Default::default()
        };
        let output = paint(&location, &settings);
        let text = location.context.as_ref().unwrap().text.as_ref().unwrap();
        let job = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(shape) if shape.galley.text() == text => Some(&shape.galley.job),
                _ => None,
            })
            .unwrap();
        let marked: Vec<_> = job
            .sections
            .iter()
            .filter(|section| section.format.background != egui::Color32::TRANSPARENT)
            .collect();
        assert_eq!(marked.len(), 1);
        assert_eq!(&text[marked[0].byte_range.clone()], "真因 missing");
        for section in &job.sections {
            assert_eq!(section.format.font_id.size, 28.);
            assert_eq!(section.format.line_height, Some(28. * 1.8));
        }
        assert_eq!(&job.text, text);
    }

    #[test]
    fn old_unknown_and_metadata_only_context_have_no_invented_local_highlight() {
        for version in [None, Some(99), Some(1)] {
            let mut location = location();
            if let Some(version) = version {
                let context = location.context.as_mut().unwrap();
                context.version = version;
                if version == 1 {
                    context.text = None;
                    context.hit_byte_range = None;
                    context.visibility = ProblemContextVisibility::NoText;
                    location.excerpt = None;
                }
            } else {
                location.context = None;
            }
            let output = paint(&location, &AppearancePreferences::default());
            for shape in &output.shapes {
                if let egui::Shape::Text(shape) = &shape.shape {
                    assert!(shape
                        .galley
                        .job
                        .sections
                        .iter()
                        .all(|section| section.format.background == egui::Color32::TRANSPARENT));
                }
            }
        }
    }
}
