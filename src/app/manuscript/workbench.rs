use super::*;
use crate::theme;
use worldline_core::manuscript::ManuscriptCommand;

impl super::super::WorldeditApp {
    pub(in crate::app) fn manuscript_tab(&mut self, ctx: &egui::Context) {
        if self.manuscript.creating_new {
            self.manuscript_creation_tab(ctx);
            return;
        }
        let baseline = self.project.content_baseline();
        if self
            .manuscript
            .books
            .values()
            .any(|book| !book.changed && book.baseline != baseline)
            || self
                .manuscript
                .writing_buffers
                .values()
                .any(|buffer| !buffer.is_changed() && buffer.baseline() != baseline)
        {
            self.manuscript.rebase_clean(&self.project);
        }
        let indices = self.project.manuscript_indices();
        let session = self.manuscript.pending_session.take();
        if let Some(session) = &session {
            if session
                .manuscript_id
                .as_ref()
                .is_some_and(|id| indices.contains_key(id))
            {
                self.manuscript
                    .selected_book
                    .clone_from(&session.manuscript_id);
            } else if session.manuscript_id.is_some() {
                self.message =
                    Some("上次书稿 ID 已失效；当前显示默认书稿，没有按同名标题替换身份".into());
            }
        }
        if self
            .manuscript
            .selected_book
            .as_ref()
            .is_none_or(|id| !indices.contains_key(id))
        {
            self.manuscript.selected_book = indices.keys().next().cloned();
        }
        let Some(book_id) = self.manuscript.selected_book.clone() else {
            self.manuscript_creation_tab(ctx);
            return;
        };
        let Some(index) = indices.get(&book_id).cloned() else {
            return;
        };
        self.manuscript
            .books
            .entry(book_id.clone())
            .or_insert_with(|| {
                let draft = ManuscriptDraft::from_index(&index);
                let selected_entry = draft
                    .entries
                    .iter()
                    .find(|entry| entry.kind == ManuscriptEntryKind::Chapter)
                    .or_else(|| draft.entries.first())
                    .map(|entry| entry.id.clone());
                LocalBook {
                    original: draft.clone(),
                    draft,
                    baseline: self.project.content_baseline(),
                    revision: Revision::default(),
                    selected_entry,
                    changed: false,
                    collapsed: HashSet::new(),
                }
            });
        let mut local = self.manuscript.books.remove(&book_id).unwrap();
        if let Some(session) = session {
            if session.manuscript_id.as_deref() == Some(&book_id) {
                if let Some(id) = session
                    .selected_id
                    .clone()
                    .filter(|id| local.draft.entries.iter().any(|entry| &entry.id == id))
                {
                    local.selected_entry = Some(id);
                    self.manuscript.pending_scroll = Some(if session.scroll_y.is_finite() {
                        session.scroll_y.clamp(0.0, 1_000_000.0)
                    } else {
                        0.0
                    });
                    self.manuscript.writing_view.restore_mode(session.mode);
                    self.manuscript.writing_view.restore_cursor(session.cursor);
                } else if session.selected_id.is_some() {
                    local.selected_entry = None;
                    self.message = Some("上次章节 ID 已失效，请明确选择章节".into());
                }
            }
        }
        let command = ManuscriptCommand {
            expected_revision: local.revision,
            expected_baseline: local.baseline.clone(),
            original: Some(book_id.clone()),
            draft: local.draft.clone(),
        };
        let (preview, error) = match self.project.preview_manuscript(local.revision, &command) {
            Ok(preview) => (preview, None),
            Err(error) => (index.clone(), Some(error)),
        };
        let catalog = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.result.analysis.catalog.clone())
            .unwrap_or_default();
        let objects = &catalog.objects;
        let mut apply_book = false;
        let mut discard_book = false;
        let mut body_action = None;
        egui::CentralPanel::default().frame(theme::panel()).show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("书稿工作台");
                egui::ComboBox::from_id_salt("manuscript-book-picker").selected_text(index.title.as_deref().unwrap_or(&book_id)).show_ui(ui, |ui| {
                    for (id, book) in &indices {
                        ui.selectable_value(&mut self.manuscript.selected_book, Some(id.clone()), format!("{} · {id}", book.title.as_deref().unwrap_or(id)));
                    }
                });
                if ui.button("新建书稿").clicked() { self.manuscript.creating_new = true; }
                ui.selectable_value(&mut self.manuscript.layout, Layout::Tree, "章节树");
                ui.selectable_value(&mut self.manuscript.layout, Layout::List, "列表");
                ui.selectable_value(&mut self.manuscript.layout, Layout::Cards, "卡片");
                ui.checkbox(&mut self.manuscript.reader_open, "阅读预览");
            });
            ui.label(theme::muted("书稿只决定阅读顺序，独立于世界时间与事件控制流；正文编辑另行应用。"));
            for diagnostic in &index.diagnostics { ui.colored_label(theme::ERROR(), format!("{}：{}", diagnostic.code, diagnostic.message)); }
            if let Some(error) = &error { ui.colored_label(theme::ERROR(), format!("书稿草稿：{error}")); }
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!index.read_only, egui::Button::new("插入分节")).clicked() { insert_entry(&mut local, ManuscriptEntryKind::Section); }
                if ui.add_enabled(!index.read_only, egui::Button::new("插入章节")).clicked() { insert_entry(&mut local, ManuscriptEntryKind::Chapter); }
                if ui.add_enabled(!index.read_only && local.changed && error.is_none(), egui::Button::new("应用书稿")).clicked() { apply_book = true; }
                if ui.add_enabled(local.changed, egui::Button::new("恢复书稿草稿")).on_hover_text("丢弃未应用编排，保留正文文件草稿").clicked() { discard_book = true; }
                if local.changed { ui.label(theme::muted("编排尚未应用")); }
            });
            ui.separator();
            if !self.personal.settings.focus && ui.available_width() >= 760.0 {
                egui::SidePanel::left("manuscript-outline").default_width(240.0).width_range(200.0..=360.0).resizable(true).show_inside(ui, |ui| {
                    outline::draw(ui, self.manuscript.layout, &mut local, &preview, &mut self.manuscript.status_filter, &mut self.manuscript.pov_filter, objects);
                });
            } else if !self.personal.settings.focus {
                egui::CollapsingHeader::new("章节").id_salt(("manuscript-narrow-outline", &book_id)).show(ui, |ui| {
                    outline::draw(ui, self.manuscript.layout, &mut local, &preview, &mut self.manuscript.status_filter, &mut self.manuscript.pov_filter, objects);
                });
            }
            let mut scroll = egui::ScrollArea::vertical().id_salt(("manuscript-main", &book_id, &local.selected_entry));
            if let Some(offset) = self.manuscript.pending_scroll.take() { scroll = scroll.vertical_scroll_offset(offset); }
            let output = scroll.show(ui, |ui| {
                let selected = local.selected_entry.as_ref().and_then(|id| local.draft.entries.iter().find(|entry| &entry.id == id)).cloned();
                if let Some(entry) = &selected { ui.heading(&entry.title); }
                let metadata_default = selected.as_ref().is_some_and(|entry| entry.kind == ManuscriptEntryKind::Section || entry.target_ref.is_none());
                egui::CollapsingHeader::new("编排与来源").id_salt(("manuscript-metadata", &book_id, &local.selected_entry)).default_open(metadata_default).show(ui, |ui| {
                    ui.add_enabled_ui(!index.read_only, |ui| {
                        ui.horizontal(|ui| { ui.label("书名"); local.changed |= ui.text_edit_singleline(&mut local.draft.title).changed(); });
                        editing::draw_entry_editor(ui, &mut local, &catalog, &preview, &mut self.manuscript.pending_remove);
                    });
                });
                if let Some(entry) = selected.filter(|entry| entry.kind == ManuscriptEntryKind::Chapter) {
                    if let Some(target) = &entry.target_ref {
                        body_action = self.draw_manuscript_writing(ui, &book_id, &entry.id, target, index.read_only);
                    } else { ui.label("此章节尚未选择来源；请展开“编排与来源”明确选择。"); }
                }
                if self.manuscript.reader_open {
                    ui.separator();
                    ui.heading("阅读预览");
                    ui.label(theme::muted("按书稿章节顺序展示 core 编译的静态文本；分支按源码顺序列出，不模拟世界时间或事件控制流。"));
                    let mut repair = None;
                    super::preview::draw_reader_preview(self, ui, &preview, &mut repair);
                    if let Some(id) = repair { local.selected_entry = Some(id); }
                }
            });
            self.manuscript.scroll_y = output.state.offset.y;
        });
        if discard_book {
            local.draft = ManuscriptDraft::from_index(&index);
            local.original = local.draft.clone();
            local.baseline = self.project.content_baseline();
            local.changed = false;
        }
        local.changed = local.draft != local.original;
        self.manuscript.books.insert(book_id.clone(), local);
        if let Some((path, action)) = body_action {
            if let Some(error) = action.error {
                self.io_error = Some(error);
            }
            if action.discard {
                self.manuscript.writing_buffers.remove(&path);
            } else if action.apply {
                self.apply_manuscript_body(&path, action.source_mode);
            }
        }
        if apply_book {
            self.apply_manuscript_book(&book_id);
        }
    }

    fn draw_manuscript_writing(
        &mut self,
        ui: &mut egui::Ui,
        book: &str,
        chapter: &str,
        target: &TargetRef,
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
                    return None;
                }
            }
        };
        let buffer = self.manuscript.writing_buffers.get_mut(&path)?;
        let typography = super::super::writing_workspace::Typography {
            size: self.personal.settings.body_size,
            spacing: self.personal.settings.line_spacing,
            width: self.personal.settings.reading_width,
        };
        let action = ui
            .add_enabled_ui(!read_only, |ui| {
                super::super::writing_workspace::draw(
                    ui,
                    &self.project,
                    buffer,
                    target,
                    &mut self.manuscript.writing_view,
                    typography,
                )
            })
            .inner;
        Some((path, action))
    }
}

fn insert_entry(local: &mut LocalBook, kind: ManuscriptEntryKind) {
    let parent_id = editing::selected_section(local);
    let id = unique_id(
        &local.draft.entries,
        if kind == ManuscriptEntryKind::Section {
            "section"
        } else {
            "chapter"
        },
    );
    local.draft.entries.push(ManuscriptEntryDraft {
        id: id.clone(),
        kind,
        parent_id,
        title: if kind == ManuscriptEntryKind::Section {
            "新分节"
        } else {
            "新章节"
        }
        .into(),
        summary: None,
        pov: None,
        status: Some("draft".into()),
        goal: None,
        target_ref: None,
    });
    local.selected_entry = Some(id);
    local.changed = true;
}
