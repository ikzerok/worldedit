//! 当前报告的一次来源定位；不从选区、消息或同名词推断问题身份。
use super::view::{detail_button, focus_action, severity_color, severity_label};
use crate::{app::WorldeditApp, theme};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use worldline_core::{
    problems::{ProblemCursor, ProblemLocation, ProblemPrecision, ProblemQuery, ProblemsReport},
    Severity,
};

#[derive(Clone, PartialEq, Eq)]
pub(in crate::app) struct SourceProblem {
    report_version: String,
    problem_id: String,
    related: Option<usize>,
    path: PathBuf,
    version: u64,
    generation: u64,
    location: ProblemLocation,
    message: String,
    severity: Severity,
    query: ProblemQuery,
    cursor: Option<ProblemCursor>,
    previous_pages: Vec<Option<ProblemCursor>>,
}

impl WorldeditApp {
    pub(super) fn remember_problem_source(
        &mut self,
        report: &ProblemsReport,
        id: &str,
        related: Option<usize>,
        path: PathBuf,
        location: ProblemLocation,
    ) {
        let Some(entry) = report.entries.iter().find(|entry| entry.id == id) else {
            return;
        };
        self.problems.source = Some(Arc::new(SourceProblem {
            report_version: report.report_version.clone(),
            problem_id: id.into(),
            related,
            path,
            version: self.version,
            generation: self.problems.source_generation,
            location,
            message: entry.message.clone(),
            severity: entry.severity,
            query: self.problems.query.clone(),
            cursor: self.problems.cursor.clone(),
            previous_pages: self.problems.previous_pages.clone(),
        }));
    }

    pub(in crate::app) fn capture_problem_source(&self) -> Option<Arc<SourceProblem>> {
        self.problems
            .source
            .as_ref()
            .filter(|source| source.path == self.active_file && self.problem_source_current(source))
            .cloned()
    }

    pub(in crate::app) fn restore_problem_source(&mut self, source: Option<Arc<SourceProblem>>) {
        self.problems.source = None;
        let Some(source) = source.filter(|source| {
            source.path == self.active_file
                && self.problem_source_current(source)
                && self.problems.query == source.query
        }) else {
            return;
        };
        // 页不是过滤条件；用core重新投影保存的页，绝不在UI重写匹配逻辑。
        let Some(report) = self.problems.report.as_ref() else {
            return;
        };
        match report.query(&source.query, source.cursor.as_ref(), 200) {
            Ok(page)
                if page
                    .entries
                    .iter()
                    .any(|entry| entry.id == source.problem_id) =>
            {
                self.problems.cursor = source.cursor.clone();
                self.problems.previous_pages = source.previous_pages.clone();
                self.problems.page = Some(Arc::new(page));
                self.problems.select(source.problem_id.clone());
                self.problems.source = Some(source);
            }
            _ => {
                self.problems.notice =
                    Some("原问题不在可恢复的报告页中，请重新选择；这不表示已解决".into());
            }
        }
    }

    fn problem_source_current(&self, source: &SourceProblem) -> bool {
        !self.problems.stale(self.version)
            && self.problems.error.is_none()
            && source.version == self.version
            && source.generation == self.problems.source_generation
            && self
                .problems
                .report
                .as_ref()
                .is_some_and(|report| report.report_version == source.report_version)
    }

    pub(in crate::app) fn problem_source_range(
        &self,
        path: &Path,
    ) -> Option<std::ops::Range<usize>> {
        let source = self.problems.source.as_ref()?;
        if source.path != path
            || !self.problem_source_current(source)
            || source.location.precision != ProblemPrecision::Span
            || self.ime_composing
            || self.ime_source_draft.is_some()
        {
            return None;
        }
        source
            .location
            .char_range
            .as_ref()
            .map(|range| range.start..range.end)
    }

    pub(in crate::app) fn problem_source_summary(&mut self, ui: &mut egui::Ui, path: &Path) {
        let Some(source) = self
            .problems
            .source
            .clone()
            .filter(|source| source.path == path)
        else {
            return;
        };
        let current = self.problem_source_current(&source);
        let settings = self.personal.settings.clone();
        let protected = self.ime_composing
            || self.ime_source_draft.is_some()
            || self.command_palette.ime
            || self.command_palette.ime_frame;
        let scope = egui::Id::new((
            "problem-source-summary",
            &source.report_version,
            &source.problem_id,
        ));
        ui.horizontal_wrapped(|ui| {
            if focus_action(detail_button(
                ui,
                scope.with("collapse"),
                match (self.problems.source_collapsed, current) {
                    (true, true) => "▸ 当前问题",
                    (false, true) => "▾ 当前问题",
                    (true, false) => "▸ 上次问题（待重检）",
                    (false, false) => "▾ 上次问题（待重检）",
                },
                !protected,
            )) {
                self.problems.source_collapsed = !self.problems.source_collapsed;
            }
            ui.label(
                egui::RichText::new(severity_label(source.severity))
                    .color(severity_color(source.severity))
                    .size(settings.body_size),
            );
            let position = source.related.map_or_else(
                || "主位置".into(),
                |index| format!("相关位置 {}", index + 1),
            );
            ui.label(theme::muted(format!(
                "{position} · {}",
                super::view::precision_label(&source.location)
            )));
            if focus_action(detail_button(
                ui,
                scope.with("details"),
                "回到问题详情",
                !protected,
            )) {
                self.open_problems(ui.ctx());
                self.problems.focus_list = false;
                self.problems.narrow_detail = true;
                self.problems.focus_detail = true;
            }
            if focus_action(detail_button(
                ui,
                scope.with("copy"),
                "复制原因",
                !protected,
            )) {
                ui.ctx().copy_text(source.message.clone());
            }
        });
        if !self.problems.source_collapsed {
            egui::ScrollArea::vertical()
                .id_salt(scope.with("reason"))
                .max_height((settings.body_size * settings.line_spacing * 2.5).min(120.))
                .show(ui, |ui| {
                    super::view::reading_label(ui, &source.message, &settings, false);
                });
        }
        if !current || self.ime_composing || self.ime_source_draft.is_some() {
            super::view::reading_label(
                ui,
                "来源已变化或仍有输入草稿，位置强调已暂停；请完成输入后重新检查",
                &settings,
                false,
            );
        }
        ui.add_space(4.);
    }
}
