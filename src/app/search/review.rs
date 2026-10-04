//! Explicit replacement choices are core matches, never result-list offsets.
use super::*;

pub(super) const PAGE_SIZE: usize = 40;
pub(super) fn query_id() -> egui::Id {
    egui::Id::new("author-search-query")
}

impl WorldeditApp {
    pub(in crate::app) fn search_navigation_has_focus(&self, ctx: &egui::Context) -> bool {
        !self.search_state.source_view
            && (self.search_focus || ctx.memory(|memory| memory.has_focus(query_id())))
    }

    pub(super) fn invalidate_search_review(&mut self, reason: &str) {
        if !self.search_state.chosen.is_empty() || self.search_state.plan.is_some() {
            self.search_state.review_notice = Some(reason.into());
        }
        self.search_state.chosen.clear();
        self.search_state.applied_count = None;
        self.search_state.plan = None;
        self.search_state.review_preview = false;
        self.search_state.preview_page = 0;
    }

    pub(super) fn invalidate_search_preview(&mut self, reason: &str) {
        if self.search_state.plan.take().is_some() {
            self.search_state.review_notice = Some(reason.into());
        }
        self.search_state.review_preview = false;
        self.search_state.preview_page = 0;
        self.search_state.applied_count = None;
    }

    pub(super) fn reconcile_search_review(&mut self, hits: &[SearchMatch]) {
        self.refresh_search_navigation(hits);
        match reconcile_search_selection(hits, &self.search_state.chosen) {
            Ok(chosen) => self.search_state.chosen = chosen,
            Err(_) => self.invalidate_search_review(
                "来源快照已变化；旧选择与预览已清空，请重新勾选"),
        }
        if self.search_state.plan.as_ref().is_some_and(|plan| {
            self.search_request().as_ref().ok() != Some(plan.request())
                || !same_choice_keys(&plan.hits, &self.search_state.chosen)
        }) {
            self.invalidate_search_preview("替换文字或待改集合已变化，请重新预览");
        }
    }

    pub(super) fn choose_search_hit(&mut self, hit: &SearchMatch, selected: bool) {
        let Ok(hits) = self.current_search_hits() else {
            self.invalidate_search_review("查找依据已失效，请重新查找并勾选");
            return;
        };
        self.reconcile_search_review(&hits);
        if !hit.replaceable
            || reconcile_search_selection(&hits, std::slice::from_ref(hit)).is_err()
        {
            self.search_state.error = Some("此命中受保护或已过期，未加入待改集合".into());
            return;
        }
        self.invalidate_search_preview("待改集合已变化，请重新预览");
        self.search_state.chosen.retain(|chosen| chosen.path != hit.path || chosen.range != hit.range);
        if selected {
            self.search_state.chosen.push(hit.clone());
            self.search_state.chosen = reconcile_search_selection(&hits, &self.search_state.chosen)
                .expect("choices were validated against these same core results");
        }
        self.search_state.error = None;
    }

    pub(super) fn choose_all_search_hits(&mut self) {
        match self.current_search_hits() {
            Ok(hits) => {
                self.reconcile_search_review(&hits);
                self.invalidate_search_preview("待改集合已变化，请重新预览");
                self.search_state.chosen = hits.into_iter().filter(|hit| hit.replaceable).collect();
                self.search_state.error = None;
            }
            Err(error) => {
                self.invalidate_search_review("查找依据已失效，请重新查找并勾选");
                self.search_state.error = Some(error);
            }
        }
    }

    pub(super) fn search_hit_is_chosen(&self, hit: &SearchMatch) -> bool {
        self.search_state.chosen.iter().any(|chosen| chosen.path == hit.path && chosen.range == hit.range)
    }

    pub(super) fn clear_search_choices(&mut self) {
        self.invalidate_search_preview("已清空待改集合；尚未修改原文");
        self.search_state.chosen.clear();
        self.search_state.review_notice = Some("已清空待改集合；尚未修改原文".into());
    }

    pub(super) fn cancel_search_preview(&mut self) {
        self.search_state.plan = None;
        self.search_state.review_preview = false;
        self.search_state.review_notice = Some("已取消预览；原文未改，勾选仍保留".into());
    }

    pub(super) fn show_search_source(&mut self, ctx: &egui::Context, hit: &SearchMatch) {
        let Ok(hits) = self.current_search_hits() else { return; };
        self.reconcile_search_review(&hits);
        if reconcile_search_selection(&hits, std::slice::from_ref(hit)).is_ok()
            && self.go_author_source_position(ctx, hit, false).is_ok() {
            self.search_state.source_view = true;
        }
    }

    pub(super) fn return_to_search_review(&mut self, ctx: &egui::Context) {
        self.author_back(ctx);
        self.search_state.source_view = false;
        self.search_focus = true;
        self.search_state.scroll_current = true;
    }

    pub(super) fn search_result_page(&self) -> usize {
        self.search_state.selected / PAGE_SIZE
    }

    pub(super) fn set_search_result_page(&mut self, page: usize, len: usize) {
        self.search_state.selected = (page * PAGE_SIZE).min(len.saturating_sub(1));
        self.search_state.located = None;
        self.search_state.scroll_current = true;
    }
}

// Only display-state comparison; core validates exact source identities before mutation.
pub(super) fn same_choice_keys(a: &[SearchMatch], b: &[SearchMatch]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.path == b.path && a.range == b.range)
}
