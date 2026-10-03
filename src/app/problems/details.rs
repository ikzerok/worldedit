//! 完整解释与来源证据沿用一个详情区域，阅读设置覆盖每段主要内容。
use super::view::*;
use crate::{app::WorldeditApp, theme};

impl WorldeditApp {
    pub(super) fn problem_details(&mut self, ui: &mut egui::Ui) {
        let Some(problem) = self
            .problems
            .page
            .as_ref()
            .and_then(|page| {
                page.entries
                    .iter()
                    .find(|entry| Some(&entry.id) == self.problems.selected.as_ref())
            })
            .cloned()
        else {
            ui.label(theme::muted("选择一个问题，查看完整说明、主来源及相关位置"));
            return;
        };
        let current = !self.problems.stale(self.version) && self.problems.error.is_none();
        let scope = egui::Id::new(("problem-detail", &problem.id));
        let settings = self.personal.settings.clone();
        if self.problems.focus_detail {
            ui.memory_mut(|memory| memory.request_focus(scope.with("copy-problem")));
            self.problems.focus_detail = false;
        }
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt("problem-details-scroll")
            .auto_shrink([false, false]);
        if self.problems.detail_reset {
            scroll = scroll.vertical_scroll_offset(0.).animated(false);
            self.problems.detail_reset = false;
        }
        scroll.show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(800.));
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    severity_color(problem.severity),
                    severity_label(problem.severity),
                );
                if focus_action(detail_button(
                    ui,
                    scope.with("copy-problem"),
                    "复制问题",
                    true,
                )) {
                    ui.ctx()
                        .copy_text(serde_json::to_string_pretty(&problem).unwrap_or_default());
                }
            });
            reading_label(ui, &problem.message, &settings, false);
            if let Some(note) = &problem.note {
                reading_label(ui, &format!("说明：{note}"), &settings, false);
            }
            if let Some(suggestion) = &problem.suggestion {
                reading_label(ui, &format!("建议：{suggestion}"), &settings, false);
            }
            if problem.text_truncated {
                ui.colored_label(theme::GOLD(), "问题文本达到上限，当前说明不完整");
            }
            if problem.code == "A213"
                && focus_action(detail_button(
                    ui,
                    scope.with("temporal"),
                    "查看时间环与来源证据",
                    true,
                ))
            {
                self.open_temporal_issues(ui.ctx());
            }
            ui.separator();
            if location_ui(
                ui,
                "主位置",
                scope.with("primary"),
                &problem.primary,
                current,
                &settings,
            ) {
                self.locate_problem(ui.ctx(), None);
            }
            if problem.related_count > 0 {
                ui.separator();
                ui.strong(format!("相关位置 · 共 {} 处", problem.related_count));
                if let Some(related) = self.problems.related.clone() {
                    for (index, location) in related.locations.iter().enumerate() {
                        let index = self.problems.related_offset + index;
                        if location_ui(
                            ui,
                            &format!("相关位置 {}", index + 1),
                            scope.with(("related", index)),
                            location,
                            current,
                            &settings,
                        ) {
                            self.locate_problem(ui.ctx(), Some(index));
                        }
                        ui.separator();
                    }
                    ui.horizontal(|ui| {
                        if focus_action(detail_button(
                            ui,
                            scope.with("related-previous"),
                            "上页相关位置",
                            !self.problems.related_history.is_empty(),
                        )) {
                            self.problem_related_page(true);
                        }
                        if focus_action(detail_button(
                            ui,
                            scope.with("related-next"),
                            "下页相关位置",
                            related.next_cursor.is_some(),
                        )) {
                            self.problem_related_page(false);
                        }
                    });
                    if related.truncated {
                        ui.colored_label(theme::GOLD(), "相关来源已截断，不能视为完整证据");
                    }
                }
            }
            ui.separator();
            ui.label(theme::muted(format!("技术信息：{} · {}", domain_label(problem.domain), problem.code)));
        });
    }

    fn problem_related_page(&mut self, previous: bool) {
        let cursor = if previous {
            let Some(cursor) = self.problems.related_history.pop() else {
                return;
            };
            cursor
        } else {
            let Some(cursor) = self
                .problems
                .related
                .as_ref()
                .and_then(|page| page.next_cursor.clone())
            else {
                return;
            };
            self.problems
                .related_history
                .push(self.problems.related_cursor.clone());
            Some(cursor)
        };
        let Some(report) = &self.problems.report else {
            return;
        };
        let Some(id) = &self.problems.selected else {
            return;
        };
        match report.related_page(id, cursor.as_ref(), 50) {
            Ok(page) => {
                self.problems.related_offset = cursor.as_ref().map_or(0, |cursor| cursor.offset);
                self.problems.related_cursor = cursor;
                self.problems.related = Some(std::sync::Arc::new(page));
                self.problems.detail_reset = true;
            }
            Err(error) => self.problems.error = Some(error.to_string()),
        }
    }

}
