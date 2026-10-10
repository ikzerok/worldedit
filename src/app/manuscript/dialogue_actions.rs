//! 正式台词提交接入既有草稿/工程历史；没有并行撤销栈。
use crate::app::{Tab, WorldeditApp};
use std::path::Path;
use worldline_core::manuscript::DialogueEditPlan;

impl WorldeditApp {
    pub(in crate::app) fn apply_dialogue_plan(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
        plan: &DialogueEditPlan,
        continue_after: bool,
    ) {
        let result = (|| {
            if let Some(error) = self.review_input_blocker(ctx) {
                return Err(error);
            }
            if !self
                .manuscript
                .writing_view
                .dialogue_plan_is_current(path, plan)
            {
                return Err("对白输入或预览已经变化，请重新核对；输入保留".into());
            }
            let before_buffer = self
                .manuscript
                .writing_buffers
                .get(path)
                .cloned()
                .ok_or("原文件缓冲已关闭；输入保留")?;
            if plan.migration.is_some() {
                let before_project = self.project.clone();
                self.project
                    .apply_dialogue_edit(&before_buffer, plan)
                    .map_err(|error| error.to_string())?;
                if !plan.no_change {
                    self.remember_writing_project_edit(
                        before_project,
                        vec![before_buffer],
                        vec![path.into()],
                    );
                    self.manuscript
                        .clear_applied_writing_buffers(&[path.into()]);
                    self.manuscript.rebase_unchanged_manuscripts(&self.project);
                    self.recompile();
                }
                self.message =
                    Some("语言能力与所列完整正文已一次应用，可一次撤销；尚未保存工程".into());
            } else {
                let mut next = before_buffer.clone();
                self.project
                    .stage_dialogue_edit(&mut next, plan)
                    .map_err(|error| error.to_string())?;
                if !plan.no_change {
                    self.manuscript
                        .writing_buffers
                        .insert(path.into(), next.clone());
                    self.record_writing_draft_edit(before_buffer, next);
                    self.manuscript.invalidate_query_cache();
                }
                self.message = Some(
                    if plan.no_change {
                        "语句没有变化，原字节、代次与保存状态保持不变"
                    } else {
                        "正式语句已纳入唯一文件草稿，可一次撤销；尚未应用或保存"
                    }
                    .into(),
                );
            }
            Ok(())
        })();
        let error = result.err();
        self.manuscript.writing_view.dialogue_plan_result(
            path,
            &plan.request.target,
            error.clone(),
        );
        self.io_error = error.clone();
        if error.is_none() && continue_after {
            let buffer = self
                .manuscript
                .writing_buffers
                .get(path)
                .cloned()
                .map(Ok)
                .unwrap_or_else(|| self.project.open_source_writing_buffer(path));
            match buffer.and_then(|buffer| {
                self.manuscript.writing_view.begin_dialogue_continuation(
                    ctx,
                    &self.project,
                    &buffer,
                    plan,
                )?;
                self.manuscript
                    .writing_buffers
                    .entry(path.into())
                    .or_insert(buffer);
                Ok(())
            }) {
                Ok(()) => {
                    self.message =
                        Some("本句已按预览处理；下一句输入已开启，角色可修改，尚未保存".into())
                }
                Err(error) => {
                    self.io_error = Some(format!(
                        "本句已处理，但下一位置无法确认：{error}；没有创建额外语句"
                    ))
                }
            }
        }
    }
    pub(super) fn finish_writing_action(
        &mut self,
        ctx: &egui::Context,
        path: &Path,
        action: crate::app::writing_workspace::Action,
    ) {
        if let Some(plan) = &action.dialogue_plan {
            self.apply_dialogue_plan(ctx, path, plan, action.dialogue_continue);
        }
        if let Some(reference) = action.reference {
            self.open_reading(reference);
        }
        if action.production {
            self.open_production_script();
        }
        if action.create_character {
            self.create_dialogue_character(ctx);
        }
        if action.world_link {
            self.begin_manuscript_world_links(ctx);
        }
        if action.comment {
            self.comment_current_selection(ctx);
        }
        if let Some(error) = action.error {
            self.io_error = Some(error);
        }
        if action.discard {
            self.discard_manuscript_body(path);
        } else if action.apply {
            self.apply_manuscript_body(path, action.source_mode);
        }
    }
    pub(super) fn create_dialogue_character(&mut self, ctx: &egui::Context) {
        if let Some(error) = self.review_input_blocker(ctx) {
            self.io_error = Some(error);
            return;
        }
        if self.prevent_replacing_draft("人物资料") || self.snapshot.is_none() {
            return;
        }
        let origin = self.author_location(Some(ctx));
        self.new_character();
        if self.character_editor.is_some() {
            self.remember_author_location(origin);
            self.tab = Tab::Characters;
            self.message = Some(
                "新建正式人物后用 Alt+Left 返回对白；原输入保留，来源变化时需明确重新核对位置"
                    .into(),
            );
        }
    }
}
