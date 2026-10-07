use super::*;
use worldline_core::catalog::Catalog;
use worldline_core::manuscript::ManuscriptIndex;

type BodyAction = Option<(PathBuf, super::super::writing_workspace::Action)>;

pub(super) fn parallel_review(width: f32, body_size: f32) -> bool {
    width >= 960.0_f32.max(body_size * 48.0 + 96.0)
}

pub(super) fn compact_workspace(ctx: &egui::Context) -> bool {
    ctx.screen_rect().width() < 600.0 || ctx.screen_rect().height() < 420.0
}

impl super::super::WorldeditApp {
    pub(super) fn manuscript_focus_layout(&self) -> bool {
        self.personal.settings.focus || self.focus_style()
    }

    pub(super) fn draw_manuscript_content(
        &mut self,
        ui: &mut egui::Ui,
        book: &str,
        local: &mut LocalBook,
        catalog: &Catalog,
        index: &ManuscriptIndex,
        preview: &ManuscriptIndex,
    ) -> BodyAction {
        let selected = local
            .selected_entry
            .as_ref()
            .and_then(|id| local.draft.entries.iter().find(|entry| &entry.id == id))
            .cloned();
        if let Some(entry) = selected
            .as_ref()
            .filter(|entry| entry.kind != ManuscriptEntryKind::Chapter)
        {
            ui.add(egui::Label::new(egui::RichText::new(&entry.title).heading()).truncate())
                .on_hover_text(&entry.title);
        }
        // 常规字号保留已有并排入口；大字号需要足够的每栏正文宽度。
        let wide = !self.focus_style()
            && parallel_review(ui.available_width(), self.personal.appearance().body_size);
        let hidden = self.manuscript.reader_open && !wide && self.manuscript.narrow_preview;
        if self.manuscript.writing_view.needs_input_rescue(
            selected
                .as_ref()
                .and_then(|entry| entry.target_ref.as_ref()),
            index.read_only,
            hidden,
        ) {
            // 在已有可用 Ui 中保留原 TextEdit，不能迁往首帧 disabled 的浮窗测量层。
            self.manuscript_orphaned_drafts(ui);
            return None;
        }
        if self.manuscript.reader_open
            && !wide
            && !(self.manuscript_focus_layout() || compact_workspace(ui.ctx()))
        {
            let enabled = self.review_input_blocker(ui.ctx()).is_none();
            crate::theme::add_enabled_ui(ui, enabled, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.manuscript.narrow_preview, false, "编辑");
                    ui.selectable_value(&mut self.manuscript.narrow_preview, true, "预览");
                });
            });
        }
        let mut action = None;
        if self.manuscript.reader_open && wide {
            ui.columns(2, |columns| {
                action = self.draw_manuscript_editor_area(
                    &mut columns[0],
                    book,
                    local,
                    catalog,
                    index,
                    preview,
                    selected.as_ref(),
                );
                super::preview::draw_reader_preview(
                    self,
                    &mut columns[1],
                    preview,
                    selected.as_ref(),
                );
            });
        } else if self.manuscript.reader_open && self.manuscript.narrow_preview {
            super::preview::draw_reader_preview(self, ui, preview, selected.as_ref());
        } else {
            action = self.draw_manuscript_editor_area(
                ui,
                book,
                local,
                catalog,
                index,
                preview,
                selected.as_ref(),
            );
        }
        action
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_manuscript_editor_area(
        &mut self,
        ui: &mut egui::Ui,
        book: &str,
        local: &mut LocalBook,
        catalog: &Catalog,
        index: &ManuscriptIndex,
        preview: &ManuscriptIndex,
        selected: Option<&ManuscriptEntryDraft>,
    ) -> BodyAction {
        if self.review_return_available() {
            let blocked = self.review_input_blocker(ui.ctx());
            if crate::theme::add_enabled(ui, blocked.is_none(), egui::Button::new("返回审稿"))
                .on_hover_text(
                    blocked
                        .as_deref()
                        .unwrap_or("Alt+Left · 返回原章节、模式、光标和审稿滚动；不丢草稿"),
                )
                .clicked()
            {
                self.manuscript.review_navigation.back = true;
            }
        }
        let mut action = None;
        if selected.is_none_or(|entry| {
            entry.kind != ManuscriptEntryKind::Chapter || entry.target_ref.is_none()
        }) {
            // A restored section has no document viewport. Its offset must not be
            // carried to the next chapter selected by the author.
            self.manuscript.pending_scroll = None;
            self.manuscript.scroll_y = 0.0;
        }
        let metadata_salt = ("manuscript-metadata", book, local.selected_entry.clone());
        let visible_key = egui::Id::new(("manuscript-metadata-visible", &metadata_salt));
        let expanded_in_small_window = compact_workspace(ui.ctx())
            && ui
                .ctx()
                .data(|data| data.get_temp::<bool>(visible_key))
                .unwrap_or(false);
        if !(self.manuscript_focus_layout() || compact_workspace(ui.ctx()))
            || self.manuscript.focus_management
            || expanded_in_small_window
        {
            let metadata_default = selected.is_some_and(|entry| {
                entry.kind == ManuscriptEntryKind::Section || entry.target_ref.is_none()
            });
            // The focused metadata field must stay enabled to receive its own
            // Preedit/Commit. The shared composition scope protects navigation.
            let editable = !index.read_only && self.project.recovery_conflicts().is_empty();
            let access = editing::MetadataAccess {
                editable,
                actions_enabled: editable && self.review_input_blocker(ui.ctx()).is_none(),
                receiver: self
                    .manuscript
                    .writing_view
                    .composition_receiver(ui.ctx())
                    .filter(|id| editing::owns_receiver(&self.project.root, local, *id)),
            };
            access.restrict_guarded_input(ui.ctx());
            ui.scope(|ui| {
                let response = egui::CollapsingHeader::new("编排与来源")
                    .id_salt(metadata_salt)
                    .default_open(metadata_default)
                    .show(ui, |ui| {
                        let height = ui.available_height() * 0.6;
                        egui::ScrollArea::vertical()
                            .id_salt(("manuscript-metadata-content", book, &local.selected_entry))
                            .max_height(height)
                            .min_scrolled_height(0.0)
                            .show(ui, |ui| {
                                ui.scope(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label("书名");
                                        let id = egui::Id::new((
                                            "manuscript-book-title",
                                            &self.project.root,
                                            book,
                                        ));
                                        let response = crate::theme::add_enabled(
                                            ui,
                                            access.field_enabled(id),
                                            egui::TextEdit::singleline(&mut local.draft.title)
                                                .id(id),
                                        );
                                        super::super::writing_workspace::register_input(&response);
                                        local.changed |= response.changed();
                                    });
                                    editing::draw_entry_editor(
                                        ui,
                                        local,
                                        catalog,
                                        preview,
                                        &mut self.manuscript.pending_remove,
                                        &self.project.root,
                                        access,
                                    );
                                });
                            });
                    });
                // Keep a currently expanded receiver through a responsive resize.
                // This records presentation only; the original TextEdits own their input.
                ui.ctx().data_mut(|data| {
                    data.insert_temp(visible_key, response.body_response.is_some())
                });
            });
        }
        if let Some(entry) = selected.filter(|entry| entry.kind == ManuscriptEntryKind::Chapter) {
            if let Some(target) = &entry.target_ref {
                action = self.draw_manuscript_writing(
                    ui,
                    book,
                    &entry.id,
                    target,
                    &entry.title,
                    index.read_only,
                );
            } else {
                ui.label("此章节尚未选择来源；请展开“编排与来源”明确选择。");
            }
        }
        action
    }
}
