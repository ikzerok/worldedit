use super::view::*;
use crate::{app::WorldeditApp, theme};
use egui::RichText;
use worldline_core::{problems::ProblemDomain, Severity};

impl WorldeditApp {
    pub(in crate::app) fn problems_panel(&mut self, ctx: &egui::Context) {
        if !self.personal.settings.diagnostics || self.personal.settings.focus {
            return;
        }
        let height = ctx.available_rect().height();
        let max_height = (height * 0.55).min((height - 240.).max(180.)).max(180.);
        egui::TopBottomPanel::bottom("project-problems")
            .resizable(true)
            .default_height(280.)
            .min_height(220.)
            .max_height(max_height)
            .frame(theme::index_panel())
            .show(ctx, |ui| {
                let compact_controls = ui.available_height() < 280.;
                self.problems_header(ui);
                if compact_controls {
                    ui.horizontal(|ui| {
                        let count = usize::from(!self.problems.query.severities.is_empty())
                            + usize::from(!self.problems.query.domains.is_empty())
                            + usize::from(self.problems.query.path.is_some())
                            + usize::from(!self.problems.query.text.is_empty());
                        ui.menu_button(
                            if count == 0 {
                                "筛选（全部）".to_owned()
                            } else {
                                format!("筛选（{count} 项）")
                            },
                            |ui| {
                                ui.set_max_width(760.);
                                self.problems_filters(ui);
                            },
                        );
                        ui.label(theme::muted("按级别、域、路径或文字筛选"));
                    });
                } else {
                    self.problems_filters(ui);
                }
                if let Some(error) = &self.problems.error {
                    ui.colored_label(theme::ERROR(), error);
                }
                if let Some(notice) = &self.problems.notice {
                    ui.add(
                        egui::Label::new(theme::muted(notice))
                            .truncate()
                            .show_tooltip_when_elided(true),
                    );
                }
                let narrow = ui.available_width() < 1150.;
                if narrow {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(!self.problems.narrow_detail, "问题列表")
                            .clicked()
                        {
                            self.problems.narrow_detail = false;
                            self.problems.focus_list = true;
                        }
                        if ui
                            .add_enabled(
                                self.problems.selected.is_some(),
                                egui::Button::selectable(self.problems.narrow_detail, "选中详情 →"),
                            )
                            .clicked()
                        {
                            self.problems.narrow_detail = true;
                        }
                    });
                    if self.problems.narrow_detail {
                        self.problem_details(ui);
                    } else {
                        self.problem_list(ui, true);
                    }
                } else {
                    ui.columns(2, |columns| {
                        self.problem_list(&mut columns[0], false);
                        self.problem_details(&mut columns[1]);
                    });
                }
            });
    }

    fn problems_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.strong("工程问题");
            let status = if self.problems.cancelling() {
                "等待旧检查退出"
            } else if self.problems.job.is_some() {
                "刷新中"
            } else if self.problems.error.is_some() {
                "检查未完成"
            } else if self.problems.report.is_none() {
                "报告未就绪"
            } else if self.problems.stale(self.version) {
                "已过期，等待刷新"
            } else if self.problems.report.as_ref().is_some_and(|r| r.truncated) {
                "结果已截断"
            } else if self.problems.report.as_ref().is_some_and(|r| !r.complete) {
                "部分覆盖"
            } else {
                "当前缓冲快照"
            };
            ui.label(theme::muted(status));
            if let Some(page) = &self.problems.page {
                ui.label(format!("总计 {} · 匹配 {}", page.total, page.matched));
            }
            if self.problems.job.is_some() {
                ui.spinner();
                if self.problems.cancelling() {
                    if ui.small_button("排队检查最新稿").clicked() {
                        self.retry_problems(ui.ctx());
                    }
                } else if ui.small_button("取消检查").clicked() {
                    self.cancel_problems();
                }
            } else if ui.small_button("刷新检查").clicked() {
                self.retry_problems(ui.ctx());
            }
            if ui
                .small_button("上一问题")
                .on_hover_text("原生：Shift+F8")
                .clicked()
            {
                self.step_problem(ui.ctx(), true, true);
            }
            if ui
                .small_button("下一问题")
                .on_hover_text("原生：F8")
                .clicked()
            {
                self.step_problem(ui.ctx(), false, true);
            }
            self.problem_coverage_menu(ui);
            if ui.small_button("收起问题").clicked() {
                self.personal.settings.diagnostics = false;
            }
        });
        if let Some(job) = &self.problems.job {
            ui.label(theme::muted(job.status()));
        }
        if self.has_open_authoring_form() || self.ime_composing || self.ime_source_draft.is_some() {
            ui.label(theme::muted(
                "检查仅含已应用缓冲；未应用表单、正文和输入法草稿未包含",
            ));
        }
    }

    fn problems_filters(&mut self, ui: &mut egui::Ui) {
        let previous = self.problems.query.clone();
        let mut all = self.problems.query.severities.is_empty();
        ui.horizontal_wrapped(|ui| {
            if ui.selectable_label(all, "全部级别").clicked() {
                self.problems.query.severities.clear();
                all = true;
            }
            for severity in [Severity::Error, Severity::Warning, Severity::Hint] {
                let selected = self.problems.query.severities.contains(&severity);
                if ui
                    .selectable_label(
                        selected,
                        RichText::new(severity_label(severity)).color(severity_color(severity)),
                    )
                    .clicked()
                {
                    if selected {
                        self.problems.query.severities.retain(|s| *s != severity);
                    } else {
                        if all {
                            self.problems.query.severities.clear();
                        }
                        self.problems.query.severities.push(severity);
                    }
                }
            }
            egui::ComboBox::from_id_salt("problems-domain")
                .selected_text(
                    self.problems
                        .query
                        .domains
                        .first()
                        .copied()
                        .map(domain_label)
                        .unwrap_or("全部检查域"),
                )
                .width(112.)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(self.problems.query.domains.is_empty(), "全部检查域")
                        .clicked()
                    {
                        self.problems.query.domains.clear();
                    }
                    for domain in ProblemDomain::ALL {
                        if ui
                            .selectable_label(
                                self.problems.query.domains.contains(&domain),
                                domain_label(domain),
                            )
                            .clicked()
                        {
                            self.problems.query.domains = vec![domain];
                        }
                    }
                });
            let changed_current = ui
                .checkbox(&mut self.problems.current_file, "当前文件")
                .changed();
            if changed_current {
                self.problems.query.path = if self.problems.current_file {
                    self.active_file
                        .strip_prefix(&self.project.root)
                        .ok()
                        .map(|p| p.to_string_lossy().replace('\\', "/"))
                } else {
                    None
                };
            }
            let mut path = self.problems.query.path.clone().unwrap_or_default();
            if ui
                .add_enabled(
                    !self.problems.current_file,
                    egui::TextEdit::singleline(&mut path)
                        .id_salt("problems-path")
                        .hint_text("完整相对路径")
                        .desired_width(160.),
                )
                .changed()
            {
                self.problems.query.path = (!path.is_empty()).then_some(path);
            }
            ui.add(
                egui::TextEdit::singleline(&mut self.problems.query.text)
                    .id_salt("problems-text")
                    .hint_text("查消息、路径或 code")
                    .desired_width(190.),
            );
            if ui.small_button("清除筛选").clicked() {
                self.problems.query = Default::default();
                self.problems.current_file = false;
            }
        });
        if previous != self.problems.query {
            self.problems.change_filter();
        }
    }

    fn problem_list(&mut self, ui: &mut egui::Ui, narrow: bool) {
        let Some(page) = self.problems.page.clone() else {
            ui.label(theme::muted("尚无可显示报告，检查完成后会在此列出"));
            return;
        };
        let offset = self
            .problems
            .cursor
            .as_ref()
            .map_or(0, |cursor| cursor.offset);
        ui.horizontal(|ui| {
            ui.label(theme::muted(if page.entries.is_empty() {
                "当前筛选无匹配；请查看检查范围与新鲜度".into()
            } else {
                format!(
                    "第 {}–{} / {} 条",
                    offset + 1,
                    offset + page.entries.len(),
                    page.matched
                )
            }));
            if ui
                .add_enabled(
                    !self.problems.previous_pages.is_empty(),
                    egui::Button::new("← 前页"),
                )
                .clicked()
            {
                self.problem_page(true);
            }
            if ui
                .add_enabled(page.next_cursor.is_some(), egui::Button::new("后页 →"))
                .clicked()
            {
                self.problem_page(false);
            }
        });
        let list_id = egui::Id::new("problems-list");
        let entry = ui.interact(
            ui.available_rect_before_wrap(),
            list_id,
            egui::Sense::focusable_noninteractive(),
        );
        if self.problems.focus_list {
            entry.request_focus();
            self.problems.focus_list = false;
        }
        // 聚焦提示始终占相同空间，避免Tab移出时改变面板高度和控件布局而丢焦点。
        ui.label(theme::muted("列表聚焦后：↑↓选择 · Enter定位 · Esc返回"));
        let font = self.personal.settings.body_size;
        let row_height = font * 2.5 + 8.;
        self.problems.rendered_rows = 0;
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt("problems-list-scroll")
            .auto_shrink([false, false]);
        if self.problems.scroll_selected {
            if let Some(index) = self
                .problems
                .selected
                .as_ref()
                .and_then(|id| page.entries.iter().position(|e| &e.id == id))
            {
                scroll = scroll.vertical_scroll_offset(
                    index as f32 * (row_height + ui.spacing().item_spacing.y),
                );
            }
            self.problems.scroll_selected = false;
        }
        scroll.show_rows(ui, row_height, page.entries.len(), |ui, rows| {
            for index in rows {
                self.problems.rendered_rows += 1;
                let problem = &page.entries[index];
                let selected = self.problems.selected.as_ref() == Some(&problem.id);
                let response = problem_row(ui, problem, selected, font, row_height);
                if response.clicked() {
                    self.problems.select(problem.id.clone());
                    self.problems.narrow_detail = narrow;
                    ui.memory_mut(|memory| memory.request_focus(list_id));
                }
                if selected && entry.has_focus() {
                    ui.painter().rect_stroke(
                        response.rect.shrink(1.),
                        2.,
                        egui::Stroke::new(1_f32, theme::ACCENT()),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        });
    }

    fn problem_details(&mut self, ui: &mut egui::Ui) {
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
                ui.label(theme::muted(format!(
                    "{} · {}",
                    domain_label(problem.domain),
                    problem.code
                )));
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
            ui.add(
                egui::Label::new(
                    RichText::new(&problem.message).size(self.personal.settings.body_size),
                )
                .wrap(),
            );
            if let Some(note) = &problem.note {
                ui.label(format!("说明：{note}"));
            }
            if let Some(suggestion) = &problem.suggestion {
                ui.label(format!("建议：{suggestion}"));
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

    fn problem_coverage_menu(&self, ui: &mut egui::Ui) {
        let Some(report) = &self.problems.report else {
            return;
        };
        ui.menu_button("检查范围", |ui| {
            ui.set_max_width(620.);
            ui.label(format!("工程缓冲 · 报告 {}", report.report_version));
            ui.label("仅支持的静态检查；不含实际发布、本地化导入导出检查或未应用草稿");
            if ui.button("复制覆盖明细").clicked() {
                ui.ctx()
                    .copy_text(serde_json::to_string_pretty(&report.coverage).unwrap_or_default());
            }
            for reason in &report.reasons {
                ui.colored_label(theme::GOLD(), reason);
            }
            egui::ScrollArea::vertical().max_height(330.).show_rows(
                ui,
                22.,
                report.coverage.len(),
                |ui, rows| {
                    for index in rows {
                        let item = &report.coverage[index];
                        let text = format!(
                            "{} · {} · {}",
                            domain_label(item.domain),
                            item.path.as_deref().unwrap_or("域汇总"),
                            coverage_label(item.state)
                        );
                        ui.add(egui::Label::new(text).truncate())
                            .on_hover_text(item.reasons.join("\n"));
                    }
                },
            );
        });
    }

    pub(in crate::app) fn problems_status(&mut self, ui: &mut egui::Ui, content: &str) {
        let text = if let Some(report) = &self.problems.report {
            let errors = report
                .entries
                .iter()
                .filter(|entry| entry.severity == Severity::Error)
                .count();
            let warnings = report
                .entries
                .iter()
                .filter(|entry| entry.severity == Severity::Warning)
                .count();
            let status = if self.problems.error.is_some() {
                "检查未完成"
            } else if self.problems.cancelling() {
                "等待旧检查退出"
            } else if self.problems.job.is_some() {
                "刷新中"
            } else if self.problems.stale(self.version) {
                "已过期"
            } else if report.truncated {
                "已截断"
            } else if !report.complete {
                "部分覆盖"
            } else {
                "当前缓冲"
            };
            format!("工程问题 {errors}错/{warnings}提醒 · {status}")
        } else if self.problems.error.is_some() {
            "工程问题 · 检查失败".into()
        } else {
            "工程问题 · 未就绪".into()
        };
        let full = format!("{content} · {text}");
        // 为保存状态与长回执保留真实宽度；窄窗只截断入口文字，完整信息仍可悬停或打开。
        let width = (ui.available_width() * 0.58).min(540.).max(0.);
        if ui
            .add_sized(
                [width, ui.spacing().interact_size.y],
                egui::Button::new(&full).small().truncate(),
            )
            .on_hover_text(full)
            .clicked()
        {
            self.open_problems(ui.ctx());
        }
    }
}
