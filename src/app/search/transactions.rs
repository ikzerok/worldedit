use super::*;
impl WorldeditApp {
    pub(in crate::app) fn record_writing_draft_edit(
        &mut self,
        before: WritingBuffer,
        after: WritingBuffer,
    ) {
        self.search_state.undo.push(DraftUndo {
            before,
            after,
            depth: self.history.len(),
        });
        self.search_state.redo.clear();
    }
    pub(in crate::app) fn record_writing_project_edit(
        &mut self,
        before: &worldline_core::project::Project,
        buffers: Vec<WritingBuffer>,
        paths: Vec<PathBuf>,
    ) {
        self.search_state.project_edits.push(ProjectEdit {
            before: before.content_baseline(),
            after: self.project.content_baseline(),
            buffers,
            paths,
        });
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
                        self.search_state.undo.push(DraftUndo {
                            before: buffer.clone(),
                            after: next,
                            depth: self.history.len(),
                        });
                        self.search_state.redo.clear();
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
            self.search_state.project_edits.push(ProjectEdit {
                before: before.content_baseline(),
                after: self.project.content_baseline(),
                buffers: drafts
                    .iter()
                    .filter(|d| paths.contains(&d.path().to_owned()))
                    .cloned()
                    .collect(),
                paths: paths.clone(),
            });
            self.remember(before);
            self.manuscript.clear_applied_writing_buffers(&paths);
            self.recompile();
        }
        Ok(())
    }
    pub(in crate::app) fn search_project_undo(&mut self, forward: bool) -> bool {
        let baseline = self.project.content_baseline();
        let Some(index) = self.search_state.project_edits.iter().rposition(|edit| {
            if forward {
                edit.before == baseline
            } else {
                edit.after == baseline
            }
        }) else {
            return false;
        };
        let edit = &self.search_state.project_edits[index];
        let current = self.manuscript.writing_buffers();
        let unsafe_draft = current
            .iter()
            .filter(|b| b.is_changed() && edit.paths.contains(&b.path().to_owned()))
            .any(|b| {
                !forward
                    || !edit.buffers.iter().any(|original| {
                        original.path() == b.path() && original.source() == b.source()
                    })
            });
        if unsafe_draft {
            self.io_error = Some("事务后又有未应用输入，撤销或重做不能覆盖草稿".into());
            return true;
        }
        let expected = if forward {
            edit.after.clone()
        } else {
            edit.before.clone()
        };
        self.undo(forward);
        if self.project.content_baseline() == expected {
            let edit = &self.search_state.project_edits[index];
            if forward {
                self.manuscript.clear_applied_writing_buffers(&edit.paths);
            } else {
                self.manuscript.restore_writing_buffers(&edit.buffers);
            }
        }
        self.manuscript
            .rebase_unchanged_writing_buffers(&self.project);
        self.message = None;
        self.search_state.applied_count = None;
        self.search_state.plan = None;
        true
    }
    pub(in crate::app) fn search_draft_undo(&mut self, forward: bool) -> bool {
        let stack = if forward {
            &mut self.search_state.redo
        } else {
            &mut self.search_state.undo
        };
        if stack
            .last()
            .is_none_or(|entry| entry.depth != self.history.len())
        {
            return false;
        }
        let entry = stack.pop().unwrap();
        let expected = if forward { &entry.before } else { &entry.after };
        let replacement = if forward { &entry.after } else { &entry.before };
        let valid = self.project.content_baseline() == expected.baseline()
            && self
                .manuscript
                .writing_buffer_mut(expected.path())
                .is_some_and(|current| current.source() == expected.source());
        if !valid {
            self.io_error = Some("草稿在组合编辑后已变化，拒绝覆盖后续输入".into());
            return true;
        }
        *self.manuscript.writing_buffer_mut(expected.path()).unwrap() = replacement.clone();
        if forward {
            self.search_state.undo.push(entry);
        } else {
            self.search_state.redo.push(entry);
        }
        self.message = None;
        self.search_state.applied_count = None;
        self.search_state.plan = None;
        true
    }
}
