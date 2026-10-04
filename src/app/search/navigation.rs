//! 查找显示选择与真实定位分离；每个导航都重新核对 core 当前结果。
use super::*;
use crate::app::{personal::Location, Tab};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct NavigationBasis {
    request: SearchRequest,
    scope: Scope,
    sources: Vec<(PathBuf, String, Option<u64>, String)>,
}

impl WorldeditApp {
    pub(super) fn refresh_search_navigation(&mut self, hits: &[SearchMatch]) {
        let basis = self.search_request().ok().map(|mut request| {
            // Replacement text is not a matching/navigation input.
            request.replacement.clear();
            let buffers = self.manuscript.writing_buffers();
            let sources = request
                .files
                .iter()
                .map(|file| {
                    let buffer = buffers.iter().find(|buffer| buffer.path() == file.path);
                    let source = buffer
                        .filter(|buffer| buffer.is_changed())
                        .map(|buffer| buffer.source())
                        .or_else(|| self.project.document(&file.path).ok())
                        .unwrap_or("");
                    (
                        file.path.clone(),
                        crate::app::writing_workspace::fingerprint(source),
                        buffer.filter(|buffer| buffer.is_changed() || buffer.generation() != 0).map(WritingBuffer::generation),
                        self.project.content_baseline(),
                    )
                })
                .collect();
            NavigationBasis { request, scope: self.search_state.scope, sources }
        });
        if self.search_state.navigation_basis != basis {
            self.invalidate_search_review("查询、范围、选项或来源已变化；旧选择与预览已清空，请重新勾选");
            self.search_state.navigation_basis = basis;
            self.search_state.selected = 0;
            self.search_state.scroll_current = true;
            self.search_state.located = None;
        }
        self.search_state.selected = self.search_state.selected.min(hits.len().saturating_sub(1));
        if self
            .search_state
            .located
            .as_ref()
            .is_some_and(|hit| reconcile_search_selection(hits, std::slice::from_ref(hit)).is_err())
        {
            self.search_state.located = None;
        }
    }

    pub(super) fn next_search_index(&self, len: usize, previous: bool) -> usize {
        if self.search_state.located.is_none() {
            self.search_state.selected.min(len.saturating_sub(1))
        } else if previous {
            (self.search_state.selected + len - 1) % len
        } else {
            (self.search_state.selected + 1) % len
        }
    }

    pub(in crate::app) fn go_search_hit(&mut self, ctx: &egui::Context, hit: &SearchMatch) {
        if self.ime_composing || self.command_palette.ime || self.command_palette.ime_frame {
            return;
        }
        let Ok(hits) = self.current_search_hits() else {
            self.search_state.error =
                Some("当前查找依据已变化，请重新查找；当前位置与草稿已保留".into());
            return;
        };
        if reconcile_search_selection(&hits, std::slice::from_ref(hit)).is_err() {
            self.search_state.error = Some("此命中已过期，请重新查找；当前位置与草稿已保留".into());
            return;
        }
        self.go_search_position(ctx, hit);
    }

    pub(super) fn go_search_position(&mut self, ctx: &egui::Context, hit: &SearchMatch) {
        let _ = self.go_author_source_position(ctx, hit, false);
    }

    pub(in crate::app) fn go_author_source_position(
        &mut self,
        ctx: &egui::Context,
        hit: &SearchMatch,
        prefer_writing: bool,
    ) -> Result<(), String> {
        if self.ime_composing || self.command_palette.ime || self.command_palette.ime_frame {
            return Err("输入法组合尚未完成，未离开当前位置".into());
        }
        let drafts = self.manuscript.writing_buffers();
        let source = drafts
            .iter()
            .find(|buffer| buffer.path() == hit.path && buffer.is_changed())
            .map(|buffer| buffer.source().to_owned())
            .or_else(|| self.project.document(&hit.path).ok().map(str::to_owned));
        let source = source.ok_or("作者来源文件不存在，当前位置与草稿已保留")?;
        let position: Location = self.author_location(Some(ctx));
        if prefer_writing || hit.draft || self.tab == Tab::Manuscript {
            if let Some(plan) =
                self.manuscript
                    .plan_writing_match(&hit.path, &hit.range, &self.project)
            {
                let reason = plan.reason.clone();
                if let Err(error) = self.manuscript.apply_writing_match(plan, &self.project) {
                    self.search_state.error = Some(error.clone());
                    return Err(error);
                }
                let generation = self
                    .manuscript
                    .writing_buffer_mut(&hit.path)
                    .unwrap()
                    .generation();
                selection::request_writing_selection(
                    ctx,
                    hit.path.clone(),
                    source,
                    hit.range.clone(),
                    generation,
                );
                self.remember_author_location(position);
                self.tab = Tab::Manuscript;
                self.search_state.error = None;
                self.message = reason.map(|reason| {
                    format!("已在同一草稿的源码中定位：{reason}；可用返回恢复原作者位置")
                });
                if let Ok(hits) = self.current_search_hits() {
                    self.refresh_search_navigation(&hits);
                    self.search_state.selected =
                        hits.iter().position(|current| current.path == hit.path && current.range == hit.range).unwrap_or(0);
                }
                self.search_state.scroll_current = true;
                self.search_state.located = Some(hit.clone());
                return Ok(());
            }
            if hit.draft {
                self.search_state.error = Some("此命中来自未应用草稿，但关联章节无法确认；保留当前入口与完整草稿，未切换到旧工程内容".into());
                return Err(self.search_state.error.clone().unwrap());
            }
            self.message =
                Some("此文件没有可确认的书稿章节，已在工程源码定位；可用返回恢复原作者位置".into());
        }
        self.remember_author_location(position);
        self.active_file = hit.path.clone();
        self.tab = Tab::Edit;
        self.jump = None;
        selection::request_selection(ctx, hit.path.clone(), source, hit.range.clone());
        self.search_state.error = None;
        if let Ok(hits) = self.current_search_hits() {
            self.refresh_search_navigation(&hits);
            self.search_state.selected =
                hits.iter().position(|current| current.path == hit.path && current.range == hit.range).unwrap_or(0);
        }
        self.search_state.scroll_current = true;
        self.search_state.located = Some(hit.clone());
        Ok(())
    }

    pub(in crate::app) fn navigate_search(&mut self, ctx: &egui::Context, previous: bool) {
        if let Ok(hits) = self.current_search_hits() {
            self.refresh_search_navigation(&hits);
            if !hits.is_empty() {
                self.search_state.selected = self.next_search_index(hits.len(), previous);
                self.go_search_hit(ctx, &hits[self.search_state.selected]);
            }
        }
    }
}
