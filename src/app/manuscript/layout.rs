use super::*;
use worldline_core::catalog::Catalog;
use worldline_core::manuscript::ManuscriptIndex;

type BodyAction = Option<(PathBuf, super::super::writing_workspace::Action)>;

impl super::super::WorldeditApp {
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
        if let Some(entry) = &selected {
            ui.add(egui::Label::new(egui::RichText::new(&entry.title).heading()).truncate())
                .on_hover_text(&entry.title);
        }
        let wide = ui.available_width() >= 960.0;
        if self.manuscript.reader_open && !wide {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.manuscript.narrow_preview, false, "编辑");
                ui.selectable_value(&mut self.manuscript.narrow_preview, true, "预览");
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
        let mut action = None;
        let scroll_salt = egui::Id::new(("manuscript-main", book, &local.selected_entry));
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt(("manuscript-main", book, &local.selected_entry))
            .auto_shrink([false, false]);
        let restored_scroll = self.manuscript.pending_scroll.take();
        if let Some(offset) = restored_scroll {
            // 返回的视口是完整位置：旧搜索遗留的动画目标和惯性不能覆盖它。
            // egui 的 vertical_scroll_offset 仅改 offset，不清理这些内部状态。
            let mut state = egui::scroll_area::State::default();
            state.offset.y = offset;
            state.store(ui.ctx(), ui.make_persistent_id(scroll_salt));
            scroll = scroll.vertical_scroll_offset(offset).animated(false);
        }
        let output = scroll.show(ui, |ui| {
            if !self.personal.settings.focus || self.manuscript.focus_management {
                let metadata_default = selected.is_some_and(|entry| {
                    entry.kind == ManuscriptEntryKind::Section || entry.target_ref.is_none()
                });
                egui::CollapsingHeader::new("编排与来源")
                    .id_salt(("manuscript-metadata", book, &local.selected_entry))
                    .default_open(metadata_default)
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!index.read_only, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("书名");
                                local.changed |=
                                    ui.text_edit_singleline(&mut local.draft.title).changed();
                            });
                            editing::draw_entry_editor(
                                ui,
                                local,
                                catalog,
                                preview,
                                &mut self.manuscript.pending_remove,
                            );
                        });
                    });
            }
            if let Some(entry) = selected.filter(|entry| entry.kind == ManuscriptEntryKind::Chapter)
            {
                if let Some(target) = &entry.target_ref {
                    action =
                        self.draw_manuscript_writing(ui, book, &entry.id, target, index.read_only);
                } else {
                    ui.label("此章节尚未选择来源；请展开“编排与来源”明确选择。");
                }
            }
            if restored_scroll.is_some() {
                // 此帧明确恢复视口，保留选区方向但不强制把光标重新滚到屏幕中央。
                // 用当前可见区替换 TextEdit 的自动滚动请求，下一帧恢复普通滚动行为。
                ui.scroll_to_rect(ui.clip_rect(), None);
            }
        });
        self.manuscript.scroll_y = output.state.offset.y;
        action
    }
}
