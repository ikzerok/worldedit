//! Bounded result and before/after views share one review pane.
use super::*;
use egui::{text::LayoutJob, TextFormat, TextStyle};

impl WorldeditApp {
    pub(super) fn search_review_pane(&mut self, ui: &mut egui::Ui, hits: &[SearchMatch]) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.search_state.review_preview, false, "命中与勾选");
            ui.add_enabled_ui(self.search_state.plan.is_some(), |ui| {
                ui.selectable_value(&mut self.search_state.review_preview, true, "选中项前后预览");
            });
        });
        if self.search_state.review_preview {
            self.search_preview_pane(ui);
        } else {
            self.search_results_pane(ui, hits);
        }
    }

    fn search_results_pane(&mut self, ui: &mut egui::Ui, hits: &[SearchMatch]) {
        let page = self.search_result_page();
        let pages = hits.len().div_ceil(review::PAGE_SIZE).max(1);
        if let Some(next) = pager(ui, page, pages, hits.len()) {
            self.set_search_result_page(next, hits.len());
        }
        let page = self.search_result_page();
        let height = ui.available_height().max(50.0);
        egui::ScrollArea::vertical()
            .id_salt(("search-results", page))
            .auto_shrink([false, false])
            .max_height(height)
            .show(ui, |ui| {
                if hits.is_empty() {
                    ui.label("没有可显示的命中；输入文字或调整明确的查找范围");
                }
                for (index, hit) in hits.iter().enumerate()
                    .skip(page * review::PAGE_SIZE).take(review::PAGE_SIZE)
                {
                    ui.push_id(("search-hit", index), |ui| {
                        ui.horizontal_top(|ui| {
                            if self.search_state.replace {
                                let mut chosen = self.search_hit_is_chosen(hit);
                                let label = if hit.replaceable { "待改" } else { "保护" };
                                if ui.add_enabled(hit.replaceable,
                                    egui::Checkbox::new(&mut chosen, label))
                                    .on_hover_text(format!("选择第 {} 处加入替换集合", index + 1))
                                    .changed()
                                {
                                    self.choose_search_hit(hit, chosen);
                                }
                            }
                            let current = index == self.search_state.selected;
                            let mut job = match &hit.context {
                                Some(context) => context_job(ui, context),
                                None => LayoutJob::simple_singleline(hit.preview.clone(),
                                    TextStyle::Body.resolve(ui.style()), crate::theme::TEXT()),
                            };
                            let path = hit.path.strip_prefix(&self.project.root).unwrap_or(&hit.path);
                            let metadata = format!("\n{}:{}:{} · 第 {} 处{}{}{}",
                                path.display(), hit.line, hit.column, index + 1,
                                if current { " · 当前项" } else { "" },
                                if hit.draft { " · 未应用草稿" } else { "" },
                                if hit.replaceable { "" } else { " · 受保护，仅查看" });
                            job.append(&metadata, 0.0, meta_format(ui));
                            let response = ui.add_sized(
                                [ui.available_width(), 0.0],
                                egui::Button::selectable(current, job).wrap(),
                            );
                            if current && self.search_state.scroll_current {
                                response.scroll_to_me(Some(egui::Align::Center));
                                self.search_state.scroll_current = false;
                            }
                            if response.clicked() {
                                self.search_state.selected = index;
                                self.search_state.located = None;
                            }
                        });
                    });
                }
            });
    }

    fn search_preview_pane(&mut self, ui: &mut egui::Ui) {
        let Some(plan) = self.search_state.plan.clone() else {
            self.search_state.review_preview = false;
            return;
        };
        let pages = plan.occurrences.len().div_ceil(review::PAGE_SIZE).max(1);
        self.search_state.preview_page = self.search_state.preview_page.min(pages - 1);
        if let Some(next) = pager(ui, self.search_state.preview_page, pages, plan.occurrences.len()) {
            self.search_state.preview_page = next;
        }
        let page = self.search_state.preview_page;
        egui::ScrollArea::vertical()
            .id_salt(("search-preview", page))
            .auto_shrink([false, false])
            .max_height(ui.available_height().max(50.0))
            .show(ui, |ui| {
                for (index, occurrence) in plan.occurrences.iter().enumerate()
                    .skip(page * review::PAGE_SIZE).take(review::PAGE_SIZE)
                {
                    let path = occurrence.path.strip_prefix(&self.project.root)
                        .unwrap_or(&occurrence.path);
                    let hit = &plan.hits[index];
                    ui.strong(format!("待改 {} / {} · {}:{}:{}",
                        index + 1, plan.hits.len(), path.display(), hit.line, hit.column));
                    ui.small(format!("原范围 {}–{} → 新范围 {}–{}（UTF-8 字节）",
                        occurrence.before_range.start, occurrence.before_range.end,
                        occurrence.after_range.start, occurrence.after_range.end));
                    ui.label("修改前");
                    ui.label(context_job(ui, &occurrence.before_context));
                    ui.label("修改后");
                    ui.label(context_job(ui, &occurrence.after_context));
                    ui.separator();
                }
            });
    }
}

fn pager(ui: &mut egui::Ui, page: usize, pages: usize, count: usize) -> Option<usize> {
    let mut next = None;
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(page > 0, egui::Button::new("上一页")).clicked() {
            next = Some(page - 1);
        }
        ui.small(format!("第 {} / {} 页 · 本范围共 {} 处", page + 1, pages, count));
        if ui.add_enabled(page + 1 < pages, egui::Button::new("下一页")).clicked() {
            next = Some(page + 1);
        }
    });
    next
}

fn meta_format(ui: &egui::Ui) -> TextFormat {
    TextFormat {
        font_id: TextStyle::Small.resolve(ui.style()),
        color: crate::theme::MUTED(),
        ..Default::default()
    }
}

pub(super) fn context_job(ui: &egui::Ui, context: &SearchContext) -> LayoutJob {
    let mut job = LayoutJob::default();
    let normal = TextFormat {
        font_id: TextStyle::Body.resolve(ui.style()),
        color: crate::theme::TEXT(),
        ..Default::default()
    };
    let highlighted = TextFormat {
        background: crate::theme::ACCENT().gamma_multiply(0.2),
        underline: egui::Stroke::new(1.5, crate::theme::ACCENT()),
        ..normal.clone()
    };
    // All byte ranges and clipping decisions come from core.
    if let (Some(before), Some(matched), Some(after)) = (
        context.text.get(..context.highlight.start),
        context.text.get(context.highlight.clone()),
        context.text.get(context.highlight.end..),
    ) {
        job.append(before, 0.0, normal.clone());
        job.append(matched, 0.0, highlighted);
        job.append(after, 0.0, normal);
    } else {
        job.append(&context.text, 0.0, normal);
    }
    let mut notes = Vec::new();
    if context.omitted_before { notes.push("前文已省略"); }
    if context.omitted_after { notes.push("后文已省略"); }
    if context.match_omitted_before || context.match_omitted_after {
        notes.push("匹配内容未完整显示，请查看原文");
    }
    if context.grapheme_clipped { notes.push("组合字符显示受限"); }
    if !notes.is_empty() {
        job.append(&format!("\n〔{}〕", notes.join(" · ")), 0.0, meta_format(ui));
    }
    job
}
