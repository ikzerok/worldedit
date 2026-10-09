use super::world_links::State;
use crate::app::{search, WorldeditApp};

impl WorldeditApp {
    pub(super) fn preview_manuscript_world_link(&self, state: &mut State) {
        let result = state.request().and_then(|request| {
            self.project
                .preview_writing_authoring(&self.manuscript.writing_buffers(), &request)
        });
        state.focus_result = true;
        match result {
            Ok(plan) => {
                state.plan = Some(plan);
                state.error = None;
            }
            Err(error) => {
                state.plan = None;
                state.error = Some(error);
            }
        }
    }

    pub(super) fn apply_manuscript_world_link(
        &mut self,
        ctx: &egui::Context,
        state: &mut State,
    ) -> bool {
        let Some(plan) = state.plan.clone() else {
            return false;
        };
        let same_request = state
            .request()
            .ok()
            .and_then(|request| serde_json::to_value(request).ok())
            == serde_json::to_value(plan.request()).ok();
        if !same_request {
            state.plan = None;
            state.error = Some("关联输入已变化，请重新预览".into());
            return false;
        }
        if self.review_input_blocker(ctx).is_some() || self.manuscript.navigation.input.blocked() {
            state.error = Some("请先完成输入法组合，关联计划未应用".into());
            return false;
        }
        let paths = plan.changed_files();
        let result = if plan.creates_object() {
            let before = self.project.clone();
            let drafts = self.manuscript.writing_buffers();
            self.project
                .apply_writing_authoring(&drafts, &plan)
                .map(|()| {
                    let included = drafts
                        .into_iter()
                        .filter(|buffer| paths.contains(&buffer.path().to_owned()))
                        .collect();
                    self.remember_writing_project_edit(before, included, paths.clone());
                    self.manuscript.clear_applied_writing_buffers(&paths);
                    self.manuscript.rebase_unchanged_manuscripts(&self.project);
                    self.recompile();
                    self.message =
                        Some("资料与所列全文草稿已一次应用，可一次撤销；尚未保存".into());
                })
        } else {
            let before = self
                .manuscript
                .writing_buffers
                .get(&plan.source_path)
                .cloned();
            if let Some(before) = before {
                let mut after = before.clone();
                self.project
                    .insert_writing_reference(&mut after, &plan)
                    .map(|()| {
                        self.manuscript
                            .writing_buffers
                            .insert(plan.source_path.clone(), after.clone());
                        self.record_writing_draft_edit(before, after);
                        self.manuscript.invalidate_query_cache();
                        self.message = Some("稳定引用已插入正文草稿；尚未应用或保存".into());
                    })
            } else {
                Err("正文缓冲已关闭，请保留输入并重新选词".into())
            }
        };
        if let Err(error) = result {
            state.focus_result = true;
            state.error = Some(error);
            return false;
        }
        let buffer = match self
            .manuscript
            .writing_buffers
            .get(&plan.source_path)
            .cloned()
        {
            Some(buffer) => buffer,
            None => match self.project.open_source_writing_buffer(&plan.source_path) {
                Ok(buffer) => {
                    self.manuscript
                        .writing_buffers
                        .insert(plan.source_path.clone(), buffer.clone());
                    buffer
                }
                Err(error) => {
                    self.io_error = Some(error);
                    return true;
                }
            },
        };
        let mut origin = state.origin.clone();
        origin.cursor = None;
        origin.anchor = Some(super::session::WritingAnchor {
            path: plan.source_path.clone(),
            target: plan.source.clone(),
            source_baseline: crate::app::writing_workspace::fingerprint(buffer.source()),
            buffer_baseline: buffer.baseline().into(),
            generation: buffer.generation(),
        });
        self.restore_manuscript_session(origin);
        search::request_writing_selection(
            ctx,
            plan.source_path,
            buffer.source().into(),
            plan.cursor_utf8..plan.cursor_utf8,
            buffer.generation(),
        );
        state.plan = None;
        state.touched = false;
        state.open = false;
        self.io_error = None;
        true
    }
}
