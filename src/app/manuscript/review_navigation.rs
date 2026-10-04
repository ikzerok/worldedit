//! 来源点击携带审稿快照；确认当前文件/草稿/磁盘后才改变作者位置。
use super::*;
use crate::app::{personal::Location, Tab, WorldeditApp};
use std::sync::Arc;
use worldline_core::{
    manuscript::{validate_review_source, ReviewProjection, ReviewSource},
    search_replace::SearchMatch,
};

#[derive(Clone)]
pub(super) struct ReviewRequest {
    pub key: String,
    pub review: Arc<ReviewProjection>,
    pub source: ReviewSource,
}

#[derive(Default)]
pub(super) struct ReviewNavigation {
    pub pending: Option<ReviewRequest>,
    pub notice: Option<String>,
    pub back: bool,
    origin: Option<Location>,
}

impl WorldeditApp {
    pub(super) fn review_input_blocker(&self, ctx: &egui::Context) -> Option<String> {
        if self.ime_composing
            || self.command_palette.ime
            || self.command_palette.ime_frame
            || ctx.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Ime(_)))
            })
        {
            return Some("请先完成输入法组合；当前位置和输入已保留".into());
        }
        if !self.project.recovery_conflicts().is_empty() {
            return Some("工作区仍有恢复冲突，来源跳转已停用；草稿保留".into());
        }
        None
    }

    pub(super) fn manuscript_review_shortcut(&mut self, ctx: &egui::Context) {
        if self.review_input_blocker(ctx).is_some() {
            return;
        }
        if ctx.input_mut(|input| {
            input.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::R,
            )
        }) {
            self.manuscript.reader_open = true;
            self.manuscript.narrow_preview = !self.manuscript.narrow_preview;
            self.manuscript.review_focus = self.manuscript.narrow_preview;
            if !self.manuscript.narrow_preview {
                self.manuscript.writing_view.focus_existing_editor();
            }
        }
    }

    pub(super) fn review_return_available(&self) -> bool {
        self.manuscript
            .review_navigation
            .origin
            .as_ref()
            .is_some_and(|origin| self.personal.history.last() == Some(origin))
    }

    pub(super) fn review_source_hit(
        &self,
        ctx: &egui::Context,
        request: &ReviewRequest,
    ) -> Result<SearchMatch, String> {
        if let Some(reason) = self.review_input_blocker(ctx) {
            return Err(reason);
        }
        let buffers = self.manuscript.writing_buffers();
        let cache = &self.manuscript.preview_cache;
        if request.key != cache.key
            || !cache.is_current(&self.project, &buffers)
            || !cache
                .current
                .get(&request.review.target)
                .is_some_and(|review| Arc::ptr_eq(review, &request.review))
        {
            return Err("此审稿来源已过期，请等待当前稿重新生成；未离开当前位置".into());
        }
        // 一次性核对完整库存与保存基线，覆盖未点中的源码、manifest、编排和新文件。
        self.project.verify_review_navigation()?;
        let compiled = self.project.compile_writing_drafts(&buffers)?;
        validate_review_source(&compiled, &request.review, &request.source)
            .map_err(|error| error.to_string())?;
        let path = worldline_core::file_access::within(
            &self.project.root,
            std::path::Path::new(&request.source.file),
        )?;
        // 校验工程原稿的磁盘基线，再定位同一路径的未应用文件草稿。
        let original = self.project.document(&path)?;
        let draft = buffers
            .iter()
            .find(|buffer| buffer.path() == path && buffer.is_changed());
        let source = draft.map_or(original, WritingBuffer::source);
        let range = request.source.byte_start..request.source.byte_end;
        let excerpt = source.get(range.clone()).ok_or("来源范围已变化，未跳转")?;
        if excerpt != request.source.excerpt {
            return Err("来源片段已变化，未跳转".into());
        }
        Ok(SearchMatch {
            path,
            range,
            line: request.source.line,
            column: request.source.column,
            preview: excerpt.into(),
            context: None,
            identity: None,
            replaceable: false,
            draft: draft.is_some(),
        })
    }

    pub(super) fn jump_review_source(
        &mut self,
        ctx: &egui::Context,
        request: &ReviewRequest,
    ) -> Result<(), String> {
        let hit = self.review_source_hit(ctx, request)?;
        let origin = self.author_location(Some(ctx));
        if let Some(plan) =
            self.manuscript
                .plan_review_match(&request.review.target, &hit.path, &self.project)
        {
            self.manuscript.apply_writing_match(plan, &self.project)?;
            let buffer = self
                .manuscript
                .writing_buffer_mut(&hit.path)
                .ok_or("源文件草稿未能打开")?;
            crate::app::search::request_writing_selection(
                ctx,
                hit.path.clone(),
                buffer.source().into(),
                hit.range,
                buffer.generation(),
            );
            crate::app::search::mark_pending_selection_programmatic(ctx);
            self.remember_author_location(origin.clone());
            self.tab = Tab::Manuscript;
        } else {
            if hit.draft {
                return Err("未应用稿的关联章节无法确认；保留完整草稿，未切换到旧工程原文".into());
            }
            self.go_author_source_position(ctx, &hit, false)?;
        }
        self.manuscript.review_navigation.origin = Some(origin);
        self.message = Some(
            "已定位审稿对应的真实原文；Alt+Left 或“返回审稿”恢复原作者位置。未应用或保存。".into(),
        );
        Ok(())
    }

    /// 必须在 workbench 将 LocalBook 放回之后执行，保证原章节和光标可返回。
    pub(super) fn finish_review_navigation(&mut self, ctx: &egui::Context) {
        if std::mem::take(&mut self.manuscript.review_navigation.back) {
            if let Some(reason) = self.review_input_blocker(ctx) {
                self.manuscript.review_navigation.notice = Some(reason);
            } else if self.review_return_available() {
                self.author_back(ctx);
                self.manuscript.review_navigation.origin = None;
                self.manuscript.review_navigation.notice = None;
            }
        }
        if let Some(request) = self.manuscript.review_navigation.pending.take() {
            self.manuscript.review_navigation.notice = self.jump_review_source(ctx, &request).err();
        }
    }
}
