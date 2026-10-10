use super::*;
impl WorldeditApp {
    pub(super) fn navigate_production_source(&mut self, ctx: &egui::Context, row: &ProductionRow) {
        let result = (|| {
            if let Some(error) = self.review_input_blocker(ctx) {
                return Err(error);
            }
            if !self.production_is_current() {
                return Err("台本已过期，请重新生成后再定位；输入已保留".into());
            }
            let snapshot = self
                .manuscript
                .production
                .snapshot
                .as_ref()
                .ok_or("尚未生成台本")?;
            let hit = self
                .project
                .production_script_source_hit(
                    &self.production_buffers(),
                    &self.production_drafts(),
                    snapshot,
                    &row.row_key,
                )
                .map_err(|error| error.to_string())?;
            let origin = self.author_location(Some(ctx));
            if let Some(plan) =
                self.manuscript
                    .plan_review_match(&row.declaration, &hit.path, &self.project)
            {
                self.manuscript.apply_writing_match(plan, &self.project)?;
                let buffer = self
                    .manuscript
                    .writing_buffer_mut(&hit.path)
                    .ok_or("未能打开已验证的来源缓冲")?;
                crate::app::search::request_writing_selection(
                    ctx,
                    hit.path.clone(),
                    buffer.source().into(),
                    hit.range,
                    buffer.generation(),
                );
                crate::app::search::mark_pending_selection_programmatic(ctx);
                self.remember_author_location(origin.clone());
                self.tab = crate::app::Tab::Manuscript;
            } else {
                // 共用文件的片段可能没有单独章节，既有精确范围导航会回到同一 buffer 的源码。
                // 真正无法确认未应用来源时，该路径仍明确拒绝，绝不显示已应用旧稿。
                self.go_author_source_position(ctx, &hit, true)?;
            }
            self.manuscript.production.return_origin = Some(origin);
            self.manuscript.production.open = false;
            Ok(())
        })();
        self.manuscript.production.notice = result.err();
    }
    pub(in crate::app::manuscript) fn production_return_button(&mut self, ui: &mut egui::Ui) {
        let available = self
            .manuscript
            .production
            .return_origin
            .as_ref()
            .is_some_and(|origin| self.personal.history.last() == Some(origin));
        if available
            && theme::add_enabled(
                ui,
                self.review_input_blocker(ui.ctx()).is_none(),
                egui::Button::new("返回角色台本"),
            )
            .clicked()
        {
            self.author_back(ui.ctx());
            self.manuscript.production.return_origin = None;
        }
    }
}
