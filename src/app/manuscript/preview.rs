//! 当前源文件草稿的只读全分支审稿；语义和物理来源一律由 core 提供。
use super::*;
mod delivery;
mod delivery_compact;
mod delivery_export;
mod delivery_job;
#[cfg(test)]
mod delivery_tests;
mod delivery_view;
mod inputs;
mod markdown_preview;
use crate::theme;
use std::sync::Arc;
use worldline_core::manuscript::{
    review_projection_with_snapshot, ManuscriptIndex, ReviewProjection, ReviewSnapshot,
};

const REVIEW_CHAPTERS_PER_PAGE: usize = 8;
const REVIEW_PAGE_NODES: usize = 12_000;
const REVIEW_PAGE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Default)]
pub(super) struct PageBudget {
    nodes: usize,
    bytes: usize,
}
impl PageBudget {
    pub(super) fn admit(&mut self, nodes: usize, bytes: usize) -> bool {
        let next_nodes = self.nodes.saturating_add(nodes);
        let next_bytes = self.bytes.saturating_add(bytes);
        if next_nodes > REVIEW_PAGE_NODES || next_bytes > REVIEW_PAGE_BYTES {
            return false;
        }
        self.nodes = next_nodes;
        self.bytes = next_bytes;
        true
    }
}

#[derive(Default)]
pub(super) struct PreviewCache {
    pub key: String,
    pub(super) scoped: bool,
    query_snapshot: Option<Arc<worldline_core::manuscript::ManuscriptQuerySnapshot>>,
    delivery: delivery::State,
    targets: Vec<TargetRef>,
    pub current: HashMap<TargetRef, Arc<ReviewProjection>>,
    last_valid: HashMap<TargetRef, Arc<ReviewProjection>>,
    sizes: HashMap<TargetRef, usize>,
    errors: HashMap<TargetRef, String>,
    error: Option<String>,
}

impl PreviewCache {
    pub(super) fn delivery_view_key(&self) -> Option<String> {
        self.delivery.reviewed.as_ref().map(|report| {
            format!(
                "{}|{}",
                report.scope().snapshot_key,
                serde_json::to_string(&report.scope().request).expect("范围DTO可序列化")
            )
        })
    }
    pub(super) fn can_restore_delivery_view(&self, session: &super::ManuscriptSession) -> bool {
        session.preview_scoped == Some(true)
            && session
                .preview_delivery_key
                .as_ref()
                .zip(self.delivery_view_key().as_ref())
                .is_some_and(|(saved, current)| saved == current)
            && self.delivery.reviewed.as_ref().is_some_and(|report| {
                session.manuscript_id.as_deref()
                    == Some(report.scope().request.query.manuscript_id.as_str())
            })
    }
    pub(super) fn restore_delivery_page(&mut self, page: usize) {
        self.delivery.page = page;
    }
    pub(super) fn set_query_snapshot(
        &mut self,
        snapshot: Option<Arc<worldline_core::manuscript::ManuscriptQuerySnapshot>>,
    ) {
        self.query_snapshot = snapshot;
    }
    pub(super) fn basis<'a>(
        project: &worldline_core::project::Project,
        buffers: impl IntoIterator<Item = &'a WritingBuffer>,
        targets: &[TargetRef],
    ) -> String {
        format!(
            "{}|{:?}",
            project.manuscript_query_key_refs(
                buffers.into_iter().filter(|buffer| buffer.is_changed()),
                &[],
            ),
            targets
        )
    }

    pub(super) fn is_current(
        &self,
        project: &worldline_core::project::Project,
        buffers: &[WritingBuffer],
    ) -> bool {
        self.key == Self::basis(project, buffers, &self.targets)
    }

    pub(super) fn refresh<'a>(
        &mut self,
        project: &worldline_core::project::Project,
        buffers: impl IntoIterator<Item = &'a WritingBuffer>,
        targets: &[TargetRef],
    ) {
        let buffers: Vec<_> = buffers
            .into_iter()
            .filter(|buffer| buffer.is_changed())
            .collect();
        let key = Self::basis(project, buffers.iter().copied(), targets);
        if self.key == key {
            return;
        }
        self.key = key;
        self.targets = targets.to_vec();
        self.current.clear();
        self.errors.clear();
        self.error = None;
        self.last_valid.retain(|target, _| targets.contains(target));
        self.sizes.retain(|target, _| targets.contains(target));
        let owned: Vec<_> = buffers.iter().copied().cloned().collect();
        match project.compile_writing_drafts(&owned) {
            Ok(result) => {
                let snapshot = match ReviewSnapshot::new(&result) {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        self.error = Some(error.to_string());
                        return;
                    }
                };
                let mut budget = PageBudget::default();
                for target in targets {
                    if self.current.contains_key(target) || self.errors.contains_key(target) {
                        continue;
                    }
                    match review_projection_with_snapshot(&result, target, &snapshot) {
                        Ok(projection) if projection.complete => {
                            let size = serde_json::to_vec(&projection)
                                .map_or(REVIEW_PAGE_BYTES + 1, |json| json.len());
                            if !budget.admit(projection.node_count, size) {
                                self.last_valid.remove(target);
                                self.errors.insert(target.clone(), "本页累计审稿预算已满；请选择本章单独审稿。此章未展示，整书尚未审完".into());
                                continue;
                            }
                            self.sizes.insert(target.clone(), size);
                            let projection = Arc::new(projection);
                            self.last_valid.insert(target.clone(), projection.clone());
                            self.current.insert(target.clone(), projection);
                        }
                        Ok(_) => {
                            self.errors
                                .insert(target.clone(), "审稿未完整生成，未展示残稿".into());
                        }
                        Err(error) => {
                            self.errors.insert(target.clone(), error.to_string());
                        }
                    }
                }
            }
            Err(error) => self.error = Some(error),
        }
    }
}

pub(super) fn draw_reader_preview(
    app: &mut super::super::WorldeditApp,
    ui: &mut egui::Ui,
    index: &ManuscriptIndex,
    selected: Option<&ManuscriptEntryDraft>,
) {
    ui.horizontal_wrapped(|ui| {
        ui.heading("全分支审稿");
        let current = ui.selectable_label(
            !app.manuscript.preview_cache.scoped && !app.manuscript.reader_whole_book,
            "当前章节",
        );
        if current.clicked() {
            app.manuscript.reader_whole_book = false;
            app.manuscript.preview_cache.scoped = false;
        }
        if app.manuscript.review_focus {
            current.request_focus();
            app.manuscript.review_focus = false;
        }
        if ui
            .selectable_label(
                !app.manuscript.preview_cache.scoped && app.manuscript.reader_whole_book,
                "整书",
            )
            .clicked()
        {
            app.manuscript.reader_whole_book = true;
            app.manuscript.preview_cache.scoped = false;
        }
        if ui
            .selectable_label(app.manuscript.preview_cache.scoped, "当前筛选范围")
            .clicked()
        {
            app.manuscript.preview_cache.scoped = true;
        }
    });
    if app.manuscript.preview_cache.scoped {
        delivery_view::draw(app, ui, index);
        return;
    }
    ui.label(egui::RichText::new("静态全分支 · 不代表真实路线").strong());
    let entries: Vec<_> = if app.manuscript.reader_whole_book {
        let mut page = index.page(app.manuscript.review_page_offset, REVIEW_CHAPTERS_PER_PAGE);
        if page.offset >= page.total && page.total > 0 {
            app.manuscript.review_page_offset =
                (page.total - 1) / REVIEW_CHAPTERS_PER_PAGE * REVIEW_CHAPTERS_PER_PAGE;
            page = index.page(app.manuscript.review_page_offset, REVIEW_CHAPTERS_PER_PAGE);
        }
        let old_offset = app.manuscript.review_page_offset;
        ui.horizontal_wrapped(|ui| {
            if crate::theme::add_enabled(ui, page.offset > 0, egui::Button::new("上一批章节"))
                .clicked()
            {
                app.manuscript.review_page_offset =
                    page.offset.saturating_sub(REVIEW_CHAPTERS_PER_PAGE);
            }
            if crate::theme::add_enabled(
                ui,
                page.next_offset.is_some(),
                egui::Button::new("下一批章节"),
            )
            .clicked()
            {
                app.manuscript.review_page_offset = page.next_offset.unwrap_or(page.offset);
            }
            ui.label(format!(
                "整书分批 · 第 {}–{} / {} 章",
                if page.total == 0 { 0 } else { page.offset + 1 },
                page.offset + page.chapters.len(),
                page.total
            ));
        });
        if old_offset != app.manuscript.review_page_offset {
            page = index.page(app.manuscript.review_page_offset, REVIEW_CHAPTERS_PER_PAGE);
            app.manuscript.pending_review_scroll = Some(0.0);
        }
        if page.total > REVIEW_CHAPTERS_PER_PAGE {
            ui.label(theme::muted(
                "仅当前批次正文已加载；其他章节请翻页审阅，不代表整书已审完。",
            ));
        }
        page.chapters
            .into_iter()
            .map(|entry| (entry.id, entry.title, entry.target_ref))
            .collect()
    } else {
        selected
            .filter(|entry| entry.kind == ManuscriptEntryKind::Chapter)
            .map(|entry| {
                vec![(
                    entry.id.clone(),
                    entry.title.clone(),
                    entry.target_ref.clone(),
                )]
            })
            .unwrap_or_default()
    };
    let targets: Vec<_> = entries
        .iter()
        .filter_map(|(_, _, target)| target.clone())
        .collect();
    if app.manuscript.writing_view.has_retained_input() {
        ui.colored_label(
            theme::WARNING(),
            "有受保护的输入尚未插入正文；下方审稿不包含这部分保留输入。",
        );
    }
    let has_drafts = app
        .manuscript
        .writing_buffers
        .values()
        .any(WritingBuffer::is_changed);
    app.manuscript.preview_cache.refresh(
        &app.project,
        app.manuscript.writing_buffers.values(),
        &targets,
    );
    let cache = &app.manuscript.preview_cache;
    if let Some(error) = &cache.error {
        ui.colored_label(theme::ERROR(), format!("预览过期 · {error}"));
        ui.label(theme::muted(
            "以下仅显示同一章节的上次有效预览；全部当前输入仍保留，旧来源跳转已停用。",
        ));
    } else {
        ui.label(theme::muted(if has_drafts {
            "当前稿 · 包含未应用输入；只读静态预览"
        } else {
            "当前工程稿 · 只读静态预览"
        }));
    }
    egui::CollapsingHeader::new("预览范围说明").show(ui, |ui| {
        ui.label("条件不求值，互斥分支和各个选择按源码顺序分别呈现；合流仅指控制流继续的情况。动态文字保留标记，call 不展开。整书按书稿编排顺序读取，不会执行、应用、保存或改变发布范围。");
        ui.label("Ctrl+Shift+R 切换编辑 / 审稿；Tab 到来源按钮，Enter 定位；Alt+Left 返回原作者位置。");
    });
    if let Some(notice) = &app.manuscript.review_navigation.notice {
        ui.colored_label(theme::ERROR(), notice);
    }
    let mut display_budget = PageBudget::default();
    let lines: Vec<_> = entries.into_iter().map(|(id, title, target)| {
        let current = target.as_ref().and_then(|target| cache.current.get(target)).cloned();
        let mut projection = current.clone().or_else(|| target.as_ref().and_then(|target| cache.last_valid.get(target)).cloned());
        let mut error = target.as_ref().and_then(|target| cache.errors.get(target)).cloned();
        if projection.as_ref().is_some_and(|review| !display_budget.admit(review.node_count, cache.sizes.get(&review.target).copied().unwrap_or(REVIEW_PAGE_BYTES + 1))) {
            projection = None;
            error = Some("本批累计显示预算已满；重复引用和旧稿也计入预算。请选择本章单独审稿，整书尚未审完".into());
        }
        (id, title, target, projection, current.is_some(), error)
    }).collect();
    let key = cache.key.clone();
    let typography = super::super::writing_workspace::Typography {
        compact: app.manuscript_focus_layout(),
        size: app.personal.appearance().body_size,
        spacing: app.personal.appearance().line_spacing,
        width: app.personal.appearance().reading_width,
        source_size: app.personal.appearance().source_size,
    };
    let blocked = app.review_input_blocker(ui.ctx());
    let mut actions = super::review_render::Actions::default();
    let id = ui.make_persistent_id((
        "manuscript-preview-scroll",
        &index.id,
        app.manuscript.reader_whole_book,
    ));
    let mut scroll = egui::ScrollArea::vertical()
        .id_salt((
            "manuscript-preview-scroll",
            &index.id,
            app.manuscript.reader_whole_book,
        ))
        .auto_shrink([false, false]);
    if let Some(offset) = app.manuscript.pending_review_scroll.take() {
        let mut state = egui::scroll_area::State::default();
        state.offset.y = offset;
        state.store(ui.ctx(), id);
        scroll = scroll.vertical_scroll_offset(offset).animated(false);
    }
    let ledger = theme::style_preset() == theme::StylePreset::Ledger;
    let output = scroll.show(ui, |ui| {
        let content = |ui: &mut egui::Ui| {
            for (id, title, target, projection, current, error) in lines {
                ui.push_id((id, "reader"), |ui| {
                    let chapter = |ui: &mut egui::Ui| {
                        if !ledger {
                            ui.separator();
                            ui.label(egui::RichText::new(&title).strong());
                        }
                        if let Some(error) = &error {
                            ui.colored_label(theme::ERROR(), format!("本章审稿未完成 · {error}"));
                            if projection.is_some() {
                                ui.label("下方是上次有效预览，已停用来源跳转。");
                            }
                        }
                        let Some(projection) = projection else {
                            ui.colored_label(
                                theme::ERROR(),
                                if target.is_none() {
                                    "章节尚未选择正文来源。"
                                } else {
                                    "当前章节没有完整审稿；请保留草稿并修复来源。"
                                },
                            );
                            return;
                        };
                        let reason = if !current {
                            Some("预览已过期，请修复当前稿后再定位")
                        } else {
                            blocked.as_deref()
                        };
                        super::review_render::draw(
                            ui,
                            &projection,
                            &key,
                            typography,
                            reason,
                            &mut actions,
                        );
                    };
                    if ledger {
                        theme::document_surface_titled(ui, &title, typography.width, chapter);
                    } else {
                        chapter(ui);
                    }
                });
            }
            if targets.is_empty() {
                ui.label(theme::muted("请选择带正文来源的章节。"));
            }
        };
        if ledger {
            content(ui);
        } else {
            theme::document_surface(ui, typography.width, content);
        }
    });
    app.manuscript.review_scroll_y = output.state.offset.y;
    if let Some(request) = actions.source {
        app.manuscript.review_navigation.pending = Some(request);
    }
    if let Some(target) = actions.reference {
        app.open_reading(target);
    }
}
