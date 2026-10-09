use super::*;
impl WorldeditApp {
    /// Explicit discard ends only this file's draft operations, across all nodes.
    pub(in crate::app) fn discard_writing_draft_history(&mut self, path: &std::path::Path) {
        self.search_state
            .undo
            .retain(|entry| entry.before.path() != path && entry.after.path() != path);
        self.search_state
            .redo
            .retain(|entry| entry.before.path() != path && entry.after.path() != path);
        self.clear_draft_history_guard();
    }

    pub(in crate::app) fn discard_all_writing_draft_history(&mut self) {
        self.search_state.undo.clear();
        self.search_state.redo.clear();
        self.clear_draft_history_guard();
    }

    pub(in crate::app) fn record_writing_draft_edit(
        &mut self,
        before: WritingBuffer,
        after: WritingBuffer,
    ) {
        let guard = self.draft_history_guard();
        self.search_state.undo.push(DraftUndo {
            before,
            after,
            node: self.history_state.current,
            guard,
        });
        self.search_state.redo.clear();
        self.redo.clear();
        self.retain_reachable_draft_history();
    }

    pub(super) fn preview_search_replacement(&mut self) {
        self.prepare_search_replacement(false);
    }
    pub(super) fn preview_selected_search_replacement(&mut self) {
        self.prepare_search_replacement(true);
    }
    fn prepare_search_replacement(&mut self, selected_only: bool) {
        if let Ok(hits) = self.current_search_hits() {
            self.reconcile_search_review(&hits);
        }
        let result = self.search_request().and_then(|request| {
            let drafts = self.manuscript.writing_buffers();
            if selected_only {
                self.project.preview_search_replace_selected(
                    &request,
                    &drafts,
                    &self.search_state.chosen,
                )
            } else {
                self.project.preview_search_replace(&request, &drafts)
            }
        });
        match result {
            Ok(plan) => {
                self.search_state.chosen = plan.hits.clone();
                self.search_state.plan = Some(plan);
                self.search_state.review_preview = true;
                self.search_state.preview_page = 0;
                self.search_state.review_notice = None;
                self.search_state.applied_count = None;
                self.search_state.error = None;
            }
            Err(error) => {
                self.search_state.plan = None;
                self.search_state.error = Some(error);
            }
        }
    }
    pub(super) fn apply_search_replacement(&mut self) {
        let Some(plan) = self.search_state.plan.clone() else {
            return;
        };
        if self.search_request().as_ref().ok() != Some(plan.request())
            || !review::same_choice_keys(&plan.hits, &self.search_state.chosen)
        {
            self.search_state.plan = None;
            self.search_state.review_preview = false;
            self.search_state.error = Some("查找范围、替换文字或待改集合已变化，请重新预览".into());
            return;
        }
        let drafts = self.manuscript.writing_buffers();
        let result = if self.search_state.scope != Scope::Project && plan.changes.len() == 1 {
            let path = &plan.changes[0].path;
            if let Some(buffer) = drafts.iter().find(|b| b.path() == path) {
                self.project
                    .replace_search_draft(&plan, &drafts, buffer)
                    .and_then(|next| {
                        let slot = self
                            .manuscript
                            .writing_buffer_mut(path)
                            .ok_or("正文草稿已关闭")?;
                        *slot = next.clone();
                        self.record_writing_draft_edit(buffer.clone(), next);
                        Ok(())
                    })
            } else {
                self.commit_search_plan(&plan, &drafts)
            }
        } else {
            self.commit_search_plan(&plan, &drafts)
        };
        match result {
            Ok(()) => {
                self.search_state.plan = None;
                self.search_state.error = None;
                let count: usize = plan.changes.iter().map(|change| change.count).sum();
                self.search_state.chosen.clear();
                self.search_state.review_preview = false;
                self.search_state.review_notice = None;
                // Establish the post-transaction basis before showing its completion count.
                let hits = self.current_search_hits().unwrap_or_default();
                self.refresh_search_navigation(&hits);
                self.search_state.focus_review_tab = true;
                self.search_state.applied_count = Some(count);
                self.search_state.applied_signature = Some(self.search_source_signature());
                self.message = Some(format!("已应用 {count} 处替换，可一次撤销；尚未保存工程"));
            }
            Err(error) => self.search_state.error = Some(error),
        }
    }
    fn commit_search_plan(
        &mut self,
        plan: &ReplacePlan,
        drafts: &[WritingBuffer],
    ) -> Result<(), String> {
        let before = self.project.clone();
        self.project.apply_search_replace(plan, drafts)?;
        if !plan.changes.is_empty() {
            let paths: Vec<_> = plan.changes.iter().map(|c| c.path.clone()).collect();
            let buffers = drafts
                .iter()
                .filter(|draft| paths.contains(&draft.path().to_owned()))
                .cloned()
                .collect();
            self.remember_writing_project_edit(before, buffers, paths.clone());
            self.manuscript.clear_applied_writing_buffers(&paths);
            self.recompile();
        }
        Ok(())
    }
    pub(in crate::app) fn search_draft_undo(&mut self, forward: bool) -> bool {
        let stack = if forward {
            &self.search_state.redo
        } else {
            &self.search_state.undo
        };
        let Some(entry) = stack
            .last()
            .filter(|entry| entry.node == self.history_state.current)
        else {
            return false;
        };
        let expected = if forward { &entry.before } else { &entry.after };
        let replacement = if forward { &entry.after } else { &entry.before };
        let current = self
            .manuscript
            .writing_buffers()
            .into_iter()
            .find(|buffer| buffer.path() == expected.path());
        let valid = current.as_ref().is_some_and(|buffer| {
            buffer.source() == expected.source() && buffer.generation() == expected.generation()
        });
        if !valid {
            self.io_error = Some("草稿在组合编辑后已变化，拒绝覆盖后续输入".into());
            return true;
        }
        // The witness is only a core root/refresh guard. Never apply its old project.
        let mut guard_candidate = self.project.clone();
        if !guard_candidate.restore((*entry.guard).clone()) {
            self.io_error = Some("撤销快照已因外部刷新失效，未改变当前工程或草稿".into());
            return true;
        }
        drop(guard_candidate);
        let mut expected = expected.clone();
        let mut replacement = replacement.clone();
        let mut current = current.unwrap();
        if expected.rebase_unchanged_source(&self.project).is_err()
            || current.rebase_unchanged_source(&self.project).is_err()
            || replacement.rebase_unchanged_source(&self.project).is_err()
        {
            self.io_error = Some("草稿原文已变化，拒绝覆盖后续输入".into());
            return true;
        }
        *self.manuscript.writing_buffer_mut(expected.path()).unwrap() = replacement;
        let entry = if forward {
            self.search_state.redo.pop()
        } else {
            self.search_state.undo.pop()
        }
        .unwrap();
        if forward {
            self.search_state.undo.push(entry);
        } else {
            self.search_state.redo.push(entry);
        }
        self.manuscript.invalidate_query_cache();
        self.io_error = None;
        self.message = None;
        self.search_state.clear_applied_operation();
        true
    }
}
