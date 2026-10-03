use crate::theme;
use worldline_core::{
    problems::{ProblemCoverageState, ProblemDomain, ProblemLocation, ProblemPrecision},
    Severity,
};

pub(super) fn domain_label(domain: ProblemDomain) -> &'static str {
    match domain {
        ProblemDomain::Content => "语言内容",
        ProblemDomain::Workspace => "工程注册",
        ProblemDomain::Maps => "地图",
        ProblemDomain::GraphViews => "关系视图",
        ProblemDomain::Presets => "视图预设",
        ProblemDomain::Comments => "审阅批注",
        ProblemDomain::Proposals => "修改提案",
        ProblemDomain::Templates => "工程模板",
        ProblemDomain::SavedQueries => "保存查询",
        ProblemDomain::Manuscripts => "书稿",
        ProblemDomain::ReaderProfiles => "读者配置",
        ProblemDomain::Localizations => "本地化",
    }
}
pub(super) fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "● 错误",
        Severity::Warning => "▲ 提醒",
        Severity::Hint => "◇ 提示",
    }
}
pub(super) fn severity_color(severity: Severity) -> egui::Color32 {
    match severity {
        Severity::Error => theme::ERROR(),
        Severity::Warning => theme::GOLD(),
        Severity::Hint => theme::BLUE(),
    }
}
pub(super) fn coverage_label(state: ProblemCoverageState) -> &'static str {
    match state {
        ProblemCoverageState::Checked => "已检查",
        ProblemCoverageState::Partial => "部分检查",
        ProblemCoverageState::Unavailable => "不可读取",
        ProblemCoverageState::NotApplicable => "不适用",
    }
}
pub(super) fn location_label(location: &ProblemLocation) -> String {
    let path = location.path.as_deref().unwrap_or("来源不可用");
    match location.precision {
        ProblemPrecision::Span => location
            .span
            .map(|s| format!("{path}:{}:{}", s.line, s.column))
            .unwrap_or_else(|| path.into()),
        ProblemPrecision::Document => format!("{path} · 仅文档位置"),
        ProblemPrecision::Unavailable => format!("{path} · 位置不可用"),
    }
}

pub(super) fn location_ui(
    ui: &mut egui::Ui,
    label: &str,
    location: &ProblemLocation,
    current: bool,
) -> bool {
    ui.strong(label);
    let text = location_label(location);
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::Label::new(&text).wrap());
        if ui.small_button("复制位置").clicked() {
            ui.ctx().copy_text(text.clone());
        }
    });
    if let Some(reason) = &location.reason {
        ui.label(theme::muted(reason));
    }
    let enabled =
        current && location.precision != ProblemPrecision::Unavailable && location.path.is_some();
    let clicked = ui
        .add_enabled(
            enabled,
            egui::Button::new(if location.precision == ProblemPrecision::Document {
                "打开文档（无精确选区）"
            } else {
                "定位来源"
            }),
        )
        .clicked();
    if let Some(excerpt) = &location.excerpt {
        ui.add(egui::Label::new(egui::RichText::new(excerpt).monospace().size(13.)).wrap());
    }
    if location.excerpt_truncated {
        ui.label(theme::muted("摘录已限长，完整原文请打开来源"));
    }
    clicked
}
