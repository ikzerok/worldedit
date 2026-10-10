//! 同稿制作台本：查询、来源和交付语义全部由 core 的不可变快照提供。
mod delivery;
mod filters;
mod jobs;
mod navigation;
mod rows;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod workflow_tests;
use crate::app::WorldeditApp;
use crate::theme;
use std::{collections::BTreeSet, sync::Arc};
use worldline_core::manuscript::{ManuscriptQueryDraft, ManuscriptQueryRequest};
use worldline_core::production_script::*;

pub(super) struct State {
    pub open: bool,
    scope: u8,
    speaker: Option<worldline_core::TargetRef>,
    locale: String,
    fallback: bool,
    fragments: bool,
    narration: bool,
    choices: bool,
    status: Option<ProductionStatus>,
    search: String,
    selected_only: bool,
    selected_chapters: BTreeSet<String>,
    chapter_offset: usize,
    snapshot: Option<Arc<ProductionScriptSnapshot>>,
    captured: Option<Input>,
    page: Option<ProductionScriptPage>,
    offset: usize,
    details_offset: usize,
    pub notice: Option<String>,
    job: Option<jobs::Job>,
    polled_frame: Option<u64>,
    observation_epoch: u64,
    observation_unavailable: bool,
    format: ProductionFormat,
    direction: bool,
    artifact: Option<ProductionArtifact>,
    artifact_options: Option<ProductionExportOptions>,
    artifact_page: usize,
    confirmed: bool,
    export_open: bool,
    #[cfg(not(target_arch = "wasm32"))]
    destination: String,
    pub return_origin: Option<crate::app::personal::Location>,
    origin: Option<super::ManuscriptSession>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            open: false,
            scope: 0,
            speaker: None,
            locale: String::new(),
            fallback: false,
            fragments: true,
            narration: false,
            choices: false,
            status: None,
            search: String::new(),
            selected_only: false,
            selected_chapters: BTreeSet::new(),
            chapter_offset: 0,
            snapshot: None,
            captured: None,
            page: None,
            offset: 0,
            details_offset: 0,
            notice: None,
            job: None,
            polled_frame: None,
            observation_epoch: 0,
            observation_unavailable: false,
            format: ProductionFormat::Json,
            direction: false,
            artifact: None,
            artifact_options: None,
            artifact_page: 0,
            confirmed: false,
            export_open: false,
            #[cfg(not(target_arch = "wasm32"))]
            destination: String::new(),
            return_origin: None,
            origin: None,
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
struct Input {
    version: u64,
    observation: String,
    observation_epoch: u64,
    root: std::path::PathBuf,
    buffers: Vec<(std::path::PathBuf, String)>,
    drafts: Vec<ManuscriptQueryDraft>,
    request: ProductionScriptRequest,
    retained: bool,
}
impl WorldeditApp {
    pub(in crate::app) fn open_production_script(&mut self) {
        if !self.manuscript.production.open {
            if self.manuscript.active_writing_target().is_none() {
                self.manuscript.production.scope = 2;
            }
            self.manuscript.production.origin = Some(self.manuscript_session());
            self.manuscript.production.open = true;
        }
    }
    /// 失败恢复必须重新刷新，即使扫描 stamp 回到原值，也不能遗留 core 的错误观察。
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn manuscript_refresh_required(&self, stamp_changed: bool) -> bool {
        stamp_changed || self.manuscript.production.observation_unavailable
    }
    /// 仅维护宿主观察可用性；没有语义查询，不清作者历史或输入。
    #[cfg(not(target_arch = "wasm32"))]
    pub(in crate::app) fn manuscript_observation_result(&mut self, available: bool) {
        let state = &mut self.manuscript.production;
        let unavailable = !available;
        if state.observation_unavailable == unavailable {
            return;
        }
        state.observation_unavailable = unavailable;
        if !available {
            state.observation_epoch = state
                .observation_epoch
                .checked_add(1)
                .expect("observation epoch exhausted");
            state.confirmed = false;
        }
        // 恢复后保留 epoch，先前通过的 receipt 不能自动复活。
        self.manuscript.invalidate_query_cache();
    }
    fn production_buffers(&self) -> Vec<worldline_core::manuscript::WritingBuffer> {
        self.manuscript
            .writing_buffers()
            .into_iter()
            .filter(|buffer| buffer.is_changed())
            .collect()
    }
    fn production_drafts(&self) -> Vec<ManuscriptQueryDraft> {
        self.manuscript
            .books
            .values()
            .filter(|local| local.changed)
            .map(|local| ManuscriptQueryDraft {
                expected_baseline: local.baseline.clone(),
                draft: local.draft.clone(),
            })
            .collect()
    }
    fn production_book_query(&self) -> Result<ManuscriptQueryRequest, String> {
        let session = &self.manuscript.navigation.session;
        Ok(ManuscriptQueryRequest {
            manuscript_id: self
                .manuscript
                .selected_book
                .clone()
                .ok_or("请先选择书稿")?,
            text: session.text.clone(),
            status: session.status.clone(),
            pov: session.pov.clone(),
            section_id: session.section_id.clone(),
            ..Default::default()
        })
    }
    fn production_request(&self) -> Result<ProductionScriptRequest, String> {
        let state = &self.manuscript.production;
        let scope = match state.scope {
            0 => ProductionScope::CurrentTarget {
                target: self
                    .manuscript
                    .active_writing_target()
                    .ok_or("请先打开带正文来源的章节")?
                    .0,
            },
            1 => ProductionScope::Manuscript {
                query: Box::new(self.production_book_query()?),
                chapter_ids: state
                    .selected_only
                    .then(|| state.selected_chapters.iter().cloned().collect()),
                expected_query_key: None,
            },
            _ => ProductionScope::Project,
        };
        let mut request = ProductionScriptRequest::new(scope);
        request.include_fragments = state.fragments;
        request.speaker = state.speaker.clone();
        request.include_narration = state.narration;
        request.include_choices = state.choices;
        request.target_locale =
            (!state.locale.trim().is_empty()).then(|| state.locale.trim().to_owned());
        request.locale_policy = if state.fallback {
            ProductionLocalePolicy::SourceFallback
        } else {
            ProductionLocalePolicy::Strict
        };
        request.statuses = state.status.into_iter().collect();
        request.search = state.search.clone();
        Ok(request)
    }
    fn production_input(&self) -> Result<Input, String> {
        if self.manuscript.production.observation_unavailable {
            return Err("磁盘观察尚未恢复，旧台本不代表当前稿；请待扫描成功后重新生成".into());
        }
        let drafts = self.production_drafts();
        let mut buffers: Vec<_> = self
            .manuscript
            .writing_buffers
            .values()
            .filter(|buffer| buffer.is_changed())
            .map(|buffer| (buffer.path().to_owned(), buffer.identity()))
            .collect();
        buffers.sort();
        Ok(Input {
            version: self.version,
            observation: self.project.catalog_scope_observation_key(),
            observation_epoch: self.manuscript.production.observation_epoch,
            root: self.project.root.clone(),
            buffers,
            drafts,
            request: self.production_request()?,
            retained: self.manuscript.writing_view.has_retained_input(),
        })
    }
    fn production_is_current(&self) -> bool {
        let state = &self.manuscript.production;
        state.job.is_none()
            && !self.manuscript.writing_view.has_retained_input()
            && state.snapshot.is_some()
            && state.captured.as_ref() == self.production_input().ok().as_ref()
    }
    pub(in crate::app) fn production_tab(&mut self, ctx: &egui::Context) {
        self.poll_production_script(ctx);
        let blocked = self.review_input_blocker(ctx);
        let mut generate = false;
        let mut close = false;
        let mut selected = None;
        let mut speaker = None;
        let mut export = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("角色制作台本");
                close = theme::add_enabled(ui, blocked.is_none(), egui::Button::new("返回正文")).clicked();
                generate = theme::add_enabled(ui, blocked.is_none() && self.manuscript.production.job.is_none(), theme::primary("生成当前稿台本")).clicked();
                if self.manuscript.production.job.is_some() {
                    ui.spinner();
                    ui.label("正在生成完整范围…");
                    if ui.button("取消生成").clicked() {
                        self.manuscript.production.job = None;
                        self.manuscript.production.notice = Some("已取消接收本次结果，未生成交付文件".into());
                    }
                }
            });
            let current = self.production_is_current();
            egui::ScrollArea::vertical().id_salt("production-script-workbench").auto_shrink([false, false]).show(ui, |ui| {
                filters::draw(self, ui, blocked.is_none());
                if let Some(reason) = &blocked { ui.colored_label(theme::WARNING(), reason); }
                if self.manuscript.writing_view.has_retained_input() {
                    ui.colored_label(theme::WARNING(), "有未插入的对白或组合输入，请先处理；不能把缺少这些输入的材料称为当前稿。");
                }
                if let Some(notice) = &self.manuscript.production.notice { ui.add(egui::Label::new(theme::muted(notice)).wrap()); }
                if self.manuscript.production.snapshot.is_some() && !current {
                    ui.colored_label(theme::WARNING(), "下方是上次结果；当前稿或范围已变化，来源导航与交付停用。请重新生成。");
                }
                if let Some(snapshot) = self.manuscript.production.snapshot.clone() {
                    rows::summary(ui, &snapshot, &mut self.manuscript.production.details_offset);
                    let state = &mut self.manuscript.production;
                    if state.page.as_ref().is_none_or(|page| page.offset != state.offset) {
                        match snapshot.page(state.offset, 40) { Ok(page) => state.page = Some(page), Err(error) => state.notice = Some(error.to_string()) }
                    }
                    if let Some(page) = &state.page {
                        ui.horizontal_wrapped(|ui| {
                            if theme::add_enabled(ui, page.offset > 0, egui::Button::new("上一页台词")).clicked() { state.offset = page.offset.saturating_sub(40); }
                            if theme::add_enabled(ui, page.next_offset.is_some(), egui::Button::new("下一页台词")).clicked() { state.offset = page.next_offset.unwrap_or(page.offset); }
                            ui.label(format!("第 {}–{} / {} 行", if page.total == 0 { 0 } else { page.offset + 1 }, page.offset + page.rows.len(), page.total));
                        });
                        for row in &page.rows {
                            rows::row(ui, &snapshot, row, current && blocked.is_none(), &mut selected, &mut speaker);
                        }
                        if page.total == 0 { ui.label("所选范围没有匹配行，未借用其他范围。"); }
                    }
                    export = delivery::draw(ui, state, current && blocked.is_none());
                }
            });
        });
        if generate {
            self.begin_production_script(ctx);
        }
        if let Some(row) = selected {
            self.navigate_production_source(ctx, &row);
        }
        if let Some(target) = speaker {
            self.open_reading(target);
        }
        if let Some(action) = export {
            self.finish_production_export(ctx, action);
        }
        if close {
            self.manuscript.production.open = false;
            if let Some(origin) = self.manuscript.production.origin.take() {
                self.restore_manuscript_session(origin);
            }
        }
    }
}
