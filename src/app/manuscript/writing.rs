//! 书稿章节打开同一文件的唯一 WritingBuffer。
use super::*;
use crate::theme;

impl super::super::WorldeditApp {
    pub(super) fn draw_manuscript_writing(
        &mut self,
        ui: &mut egui::Ui,
        book: &str,
        chapter: &str,
        target: &TargetRef,
        title: &str,
        read_only: bool,
    ) -> Option<(PathBuf, super::super::writing_workspace::Action)> {
        let key = (book.to_owned(), chapter.to_owned());
        let cached = self
            .manuscript
            .chapter_sources
            .get(&key)
            .filter(|(old_target, _)| old_target == target)
            .map(|(_, path)| path.clone());
        let path = if let Some(path) = cached {
            if !self.manuscript.writing_buffers.contains_key(&path) {
                if let Ok(buffer) = self.project.open_source_writing_buffer(&path) {
                    self.manuscript.writing_buffers.insert(path.clone(), buffer);
                }
            }
            path
        } else {
            match self.project.open_writing_buffer(target) {
                Ok(buffer) => {
                    let path = buffer.path().to_owned();
                    self.manuscript
                        .writing_buffers
                        .entry(path.clone())
                        .or_insert(buffer);
                    self.manuscript
                        .chapter_sources
                        .insert(key, (target.clone(), path.clone()));
                    path
                }
                Err(error) => {
                    ui.colored_label(theme::ERROR(), error);
                    self.manuscript.pending_scroll = None;
                    self.manuscript.scroll_y = 0.0;
                    return None;
                }
            }
        };
        if read_only && self.manuscript.writing_view.has_pending_for(&path, target) {
            ui.colored_label(
                theme::WARNING(),
                "来源已只读；组合输入会单独保留，不写入此来源",
            );
            return None;
        }
        self.draw_world_link_return(ui);
        let compact = self.manuscript_focus_layout() || layout::compact_workspace(ui.ctx());
        let Some(buffer) = self.manuscript.writing_buffers.get_mut(&path) else {
            self.manuscript.pending_scroll = None;
            self.manuscript.scroll_y = 0.0;
            return None;
        };
        let typography = super::super::writing_workspace::Typography {
            compact,
            size: self.personal.appearance().body_size,
            spacing: self.personal.appearance().line_spacing,
            width: self.personal.appearance().reading_width,
            source_size: self.personal.appearance().source_size,
        };
        let _composition = self.manuscript.writing_view.begin_input(ui.ctx());
        let mut action = crate::theme::add_enabled_ui(ui, !read_only, |ui| {
            super::super::writing_workspace::draw_controls(
                ui,
                &self.project,
                buffer,
                target,
                title,
                &mut self.manuscript.writing_view,
                typography,
            )
        })
        .inner;
        let scroll_salt = ("manuscript-main", book, Some(chapter));
        // A live typed form must reveal its real focused field/diagnostic on the
        // requested next paint. egui's animated target is applied after creating
        // the content Ui, even with zero animation duration; ordinary writing,
        // search restoration and other modes retain their existing scroll policy.
        let immediate_form = !read_only
            && self.manuscript.writing_view.uses_immediate_dialogue_scroll(
                &self.project,
                buffer,
                target,
            );
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt(scroll_salt)
            .auto_shrink([false, false])
            .animated(!immediate_form)
            .min_scrolled_height(0.0);
        let restored_scroll = self.manuscript.pending_scroll.take();
        if let Some(offset) = restored_scroll {
            // Clear old search animation and momentum when restoring a real viewport.
            let mut state = egui::scroll_area::State::default();
            state.offset.y = offset;
            state.store(ui.ctx(), ui.make_persistent_id(egui::Id::new(scroll_salt)));
            scroll = scroll.vertical_scroll_offset(offset).animated(false);
        }
        let output = scroll.show(ui, |ui| {
            crate::theme::add_enabled_ui(ui, !read_only, |ui| {
                super::super::writing_workspace::draw_document(
                    ui,
                    &self.project,
                    buffer,
                    target,
                    title,
                    &mut self.manuscript.writing_view,
                    typography,
                    &mut action,
                );
            });
            if restored_scroll.is_some() {
                // The restored viewport takes precedence over the caret's scroll request.
                ui.scroll_to_rect(ui.clip_rect(), None);
            }
        });
        self.manuscript.scroll_y = output.state.offset.y;
        Some((path, action))
    }
}
