use crate::{app::WorldeditApp, theme};
use std::sync::Arc;
use worldline_core::manuscript::{
    ManuscriptDeliveryReport, ManuscriptIndex, ManuscriptQuerySource,
};
const PAGE_SIZE: usize = 8;

pub(super) fn bounded_pages(report: &ManuscriptDeliveryReport) -> Vec<std::ops::Range<usize>> {
    let mut pages = Vec::new();
    let mut start = 0;
    let mut budget = super::PageBudget::default();
    for (position, chapter) in report.chapters().iter().enumerate() {
        let nodes = chapter
            .review
            .as_ref()
            .map_or(0, |review| review.node_count);
        if position > start
            && (position - start == PAGE_SIZE || !budget.admit(nodes, chapter.review_bytes))
        {
            pages.push(start..position);
            start = position;
            budget = super::PageBudget::default();
            assert!(
                budget.admit(nodes, chapter.review_bytes),
                "core单章预算必须小于UI单页预算"
            );
        } else if position == start {
            assert!(
                budget.admit(nodes, chapter.review_bytes),
                "core单章预算必须小于UI单页预算"
            );
        }
    }
    pages.push(start..report.chapters().len());
    pages
}

pub(super) fn draw_header(
    app: &mut WorldeditApp,
    ui: &mut egui::Ui,
) -> Option<(Arc<ManuscriptDeliveryReport>, bool, Option<String>)> {
    ui.label(egui::RichText::new("当前筛选的全部章节 · 作者私密 · 静态全分支").strong());
    if let Some(request) = app.manuscript_delivery_request() {
        let query = request.query;
        ui.add(
            egui::Label::new(theme::muted(format!(
                "文字：{} · 状态：{} · POV：{} · 分节：{}",
                label(&query.text),
                label(&query.status),
                label(&query.pov),
                query.section_id.as_deref().unwrap_or("全部"),
            )))
            .wrap(),
        );
    }
    ui.label(theme::muted(
        "包含筛选后的全部读序，不受导航页或树折叠限制；条件不求值、调用不展开，不代表实际路线。",
    ));
    ui.label(theme::muted(
        "未提交表单和未插入的保留输入不纳入；生成时逐项列出，输入始终保留。",
    ));
    let excluded = &app.manuscript.preview_cache.delivery.excluded_inputs;
    if !excluded.is_empty() {
        ui.colored_label(
            theme::WARNING(),
            format!("生成时未纳入：{}", excluded.join("；")),
        );
    }
    let blocker = app.review_input_blocker(ui.ctx());
    let busy = app.manuscript.preview_cache.delivery.job.is_some();
    ui.horizontal_wrapped(|ui| {
        if theme::add_enabled(
            ui,
            blocker.is_none() && !busy,
            theme::primary("生成同范围审稿"),
        )
        .clicked()
        {
            app.begin_manuscript_delivery(ui.ctx());
        }
        if theme::add_enabled(ui, busy, egui::Button::new("取消生成")).clicked() {
            app.cancel_manuscript_delivery();
        }
        if let Some(progress) = app
            .manuscript
            .preview_cache
            .delivery
            .job
            .as_ref()
            .and_then(|job| job.progress.as_ref())
        {
            ui.label(format!(
                "{} / {} 章 · {} 节点 · {} 字节",
                progress.completed, progress.total, progress.nodes, progress.review_bytes
            ));
        }
    });
    if let Some(error) = &blocker {
        ui.colored_label(theme::WARNING(), error);
    }
    if let Some(notice) = &app.manuscript.preview_cache.delivery.notice {
        ui.add(egui::Label::new(notice).wrap());
    }
    let Some(report) = app.manuscript.preview_cache.delivery.reviewed.clone() else {
        ui.label(theme::muted(
            "生成后可连续定位原文、返回审稿，并预览和交付同一份Markdown。",
        ));
        return None;
    };
    let current = app.manuscript_delivery_is_current();
    if !current {
        app.manuscript.preview_cache.delivery.confirmed = false;
        ui.colored_label(
            theme::WARNING(),
            "此材料已过期；保留旧证据，来源跳转、复制和导出停用。请重新生成。",
        );
    }
    let scope = report.scope();
    ui.label(format!(
        "{} · 选择 {} 个编排出现 / 匹配 {} 章 · {} 唯一源 · {} 重复出现",
        if scope.source == ManuscriptQuerySource::Draft {
            "当前稿快照（含未应用稿）"
        } else {
            "已应用工程稿快照"
        },
        scope.selected_occurrences,
        scope.matching_chapters,
        scope.unique_sources,
        scope.repeated_source_occurrences
    ));
    if !scope.writing_inputs.is_empty() {
        egui::CollapsingHeader::new(format!(
            "已纳入 {} 份正文文件草稿",
            scope.writing_inputs.len()
        ))
        .show(ui, |ui| {
            for input in &scope.writing_inputs {
                ui.add(
                    egui::Label::new(format!(
                        "{} · 草稿代次 {} · {} 字节",
                        input.file, input.generation, input.source_bytes
                    ))
                    .wrap(),
                );
            }
        });
    }
    if !report.complete() {
        ui.colored_label(
            theme::ERROR(),
            "范围或正文未完整确认；本材料不能复制或导出，缺失章没有按零字忽略。",
        );
    }
    if !scope.diagnostics.is_empty() {
        egui::CollapsingHeader::new(format!("范围诊断 · {}项", scope.diagnostics.len())).show(
            ui,
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("delivery-diagnostics")
                    .max_height(100.0)
                    .show(ui, |ui| {
                        for diagnostic in &scope.diagnostics {
                            ui.label(format!("{}：{}", diagnostic.code, diagnostic.message));
                        }
                    });
            },
        );
    }
    let state = &mut app.manuscript.preview_cache.delivery;
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut state.markdown_open, false, "连续审稿与来源");
        ui.selectable_value(&mut state.markdown_open, true, "完整Markdown原文预览");
    });
    let ready = current && blocker.is_none() && report.complete() && report.markdown().is_some();
    theme::add_enabled_ui(ui, ready, |ui| {
        ui.checkbox(
            &mut state.confirmed,
            "我已核对范围和Markdown；这是作者私密审稿材料",
        );
        #[cfg(not(target_arch = "wasm32"))]
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut state.destination)
                    .hint_text("工作区外全新 .md 路径")
                    .desired_width((ui.available_width() - 60.0).max(24.0)),
            );
            if ui.button("选择").clicked() {
                state.action = Some(super::delivery::Action::Browse);
            }
        });
        ui.horizontal_wrapped(|ui| {
            if theme::add_enabled(ui, state.confirmed, egui::Button::new("复制同一Markdown"))
                .clicked()
            {
                state.action = Some(super::delivery::Action::Copy);
            }
            if theme::add_enabled(
                ui,
                state.confirmed,
                egui::Button::new(if cfg!(target_arch = "wasm32") {
                    "请求下载Markdown"
                } else {
                    "导出新Markdown文件"
                }),
            )
            .clicked()
            {
                state.action = Some(super::delivery::Action::Save);
            }
        });
    });
    Some((report, current, blocker))
}

pub(super) fn draw(app: &mut WorldeditApp, ui: &mut egui::Ui, index: &ManuscriptIndex) {
    // 依据真实剩余空间，而非窗口标称尺寸；完整chrome在2×下会吃掉大部分中央高度。
    let compact = ui.available_height() < 320.0;
    let header = if compact {
        super::delivery_compact::controls(app, ui)
    } else {
        let height = (ui.available_height() * 0.48).min(280.0);
        egui::ScrollArea::vertical()
            .id_salt("manuscript-delivery-controls")
            .max_height(height)
            .min_scrolled_height(0.0)
            .auto_shrink([false, true])
            .show(ui, |ui| draw_header(app, ui))
            .inner
    };
    let Some((report, current, blocker)) = header else {
        return;
    };
    let scope = report.scope();
    let state = &mut app.manuscript.preview_cache.delivery;
    if state.markdown_open {
        if let Some(markdown) = report.markdown() {
            if compact {
                super::markdown_preview::draw_compact(
                    ui,
                    markdown,
                    &scope.snapshot_key,
                    &mut state.markdown_page,
                );
            } else {
                super::markdown_preview::draw(
                    ui,
                    markdown,
                    &scope.snapshot_key,
                    &mut state.markdown_page,
                );
            }
        } else {
            ui.label("没有完整Markdown产物；请修复范围/来源后重新生成。");
        }
        return;
    }
    let page = if compact {
        None
    } else {
        Some(page_controls(app, ui, &report))
    };
    draw_review(app, ui, index, &report, current, blocker.as_deref(), page);
}

fn page_controls(
    app: &mut WorldeditApp,
    ui: &mut egui::Ui,
    report: &ManuscriptDeliveryReport,
) -> std::ops::Range<usize> {
    let scope = report.scope();
    let state = &mut app.manuscript.preview_cache.delivery;
    if state.review_pages.is_empty() {
        state.review_pages = bounded_pages(report);
    }
    let mut page_index = state
        .review_pages
        .iter()
        .position(|range| range.contains(&state.page))
        .unwrap_or(0);
    ui.horizontal_wrapped(|ui| {
        if theme::add_enabled(ui, page_index > 0, egui::Button::new("上一批范围章节")).clicked()
        {
            page_index = page_index.saturating_sub(1);
            app.manuscript.pending_review_scroll = Some(0.0);
        }
        if theme::add_enabled(
            ui,
            page_index + 1 < state.review_pages.len(),
            egui::Button::new("下一批范围章节"),
        )
        .clicked()
        {
            page_index += 1;
            app.manuscript.pending_review_scroll = Some(0.0);
        }
        let page = &state.review_pages[page_index];
        ui.label(format!(
            "第 {} / {} 批 · {}–{} / {} 章",
            page_index + 1,
            state.review_pages.len(),
            if scope.chapters.is_empty() {
                0
            } else {
                page.start + 1
            },
            page.end,
            scope.chapters.len()
        ));
    });
    let page = state.review_pages[page_index].clone();
    state.page = page.start;
    app.manuscript.review_page_offset = page.start;
    page
}

fn draw_review(
    app: &mut WorldeditApp,
    ui: &mut egui::Ui,
    index: &ManuscriptIndex,
    report: &ManuscriptDeliveryReport,
    current: bool,
    blocker: Option<&str>,
    page: Option<std::ops::Range<usize>>,
) {
    let scope = report.scope();
    let typography = crate::app::writing_workspace::Typography {
        compact: app.manuscript_focus_layout(),
        size: app.personal.appearance().body_size,
        spacing: app.personal.appearance().line_spacing,
        width: app.personal.appearance().reading_width,
        source_size: app.personal.appearance().source_size,
    };
    let mut actions = super::super::review_render::Actions::default();
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt(("scoped-review", &index.id))
        .max_height(ui.available_height().max(0.0))
        .min_scrolled_height(0.0)
        .auto_shrink([false, false]);
    if let Some(offset) = app.manuscript.pending_review_scroll.take() {
        scroll = scroll.vertical_scroll_offset(offset).animated(false);
    }
    let output = scroll.show(ui, |ui| {
        // 低高度时批次与正文处于同一个滚动内容流，绝不让固定头部挤掉正文视口。
        let page = page.unwrap_or_else(|| page_controls(app, ui, report));
        if scope.chapters.is_empty() {
            ui.label("本次选择为零章；不表示整部作品没有问题。");
        }
        for chapter in &report.chapters()[page] {
            let row = &scope.chapters[chapter.occurrence];
            ui.push_id(("scope-chapter", row.ordinal), |ui| {
                theme::document_surface_titled(ui, &row.entry.title, typography.width, |ui| {
                    ui.label(theme::muted(format!(
                        "章 {} · 全书位置 {} · {}",
                        row.entry.id,
                        row.ordinal + 1,
                        row.section_path
                            .iter()
                            .map(|part| format!("{} ({})", part.title, part.id))
                            .collect::<Vec<_>>()
                            .join(" / ")
                    )));
                    if let Some(error) = &chapter.error {
                        ui.colored_label(
                            theme::ERROR(),
                            format!("{}：{}", error.code, error.message),
                        );
                    }
                    if let Some(review) = &chapter.review {
                        super::super::review_render::draw(
                            ui,
                            review,
                            &scope.snapshot_key,
                            typography,
                            if !current {
                                Some("范围快照已过期，请重新生成")
                            } else {
                                blocker
                            },
                            &mut actions,
                        );
                    }
                });
            });
        }
    });
    #[cfg(test)]
    ui.ctx().data_mut(|data| {
        data.insert_temp(egui::Id::new("delivery-review-viewport"), output.inner_rect)
    });
    app.manuscript.review_scroll_y = output.state.offset.y;
    if let Some(request) = actions.source {
        app.manuscript.review_navigation.pending = Some(request);
    }
    if let Some(target) = actions.reference {
        app.open_reading(target);
    }
}
fn label(value: &str) -> &str {
    if value.is_empty() {
        "不限"
    } else {
        value
    }
}
