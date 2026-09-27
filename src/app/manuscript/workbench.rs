use super::*;
use crate::theme;
use egui::RichText;
use std::path::Path;
use worldline_core::manuscript::{ManuscriptCommand, ManuscriptIndex};
impl super::super::WorldeditApp {
    pub(in crate::app) fn manuscript_tab(&mut self, ctx: &egui::Context) {
        if self.manuscript.creating_new {
            self.manuscript_creation_tab(ctx);
            return;
        }
        let indices = self.project.manuscript_indices();
        if self.manuscript.selected_book.is_none() {
            self.manuscript.selected_book = indices.keys().next().cloned();
        }
        if let Some(id) = self.manuscript.selected_book.clone() {
            if !indices.contains_key(&id) {
                self.manuscript.selected_book = indices.keys().next().cloned();
            }
        }
        let Some(book_id) = self.manuscript.selected_book.clone() else {
            self.manuscript_creation_tab(ctx);
            return;
        };
        let Some(index) = indices.get(&book_id).cloned() else {
            self.manuscript_creation_tab(ctx);
            return;
        };
        if !self.manuscript.books.contains_key(&book_id) {
            let draft = ManuscriptDraft::from_index(&index);
            let selected_entry = draft
                .entries
                .iter()
                .find(|entry| entry.kind == ManuscriptEntryKind::Chapter)
                .or_else(|| draft.entries.first())
                .map(|entry| entry.id.clone());
            self.manuscript.books.insert(
                book_id.clone(),
                LocalBook {
                    draft,
                    baseline: self.project.content_baseline(),
                    revision: Revision::default(),
                    selected_entry,
                    changed: false,
                },
            );
        }
        let read_only = index.read_only;
        let current_baseline = self.project.content_baseline();
        let (revision, expected_baseline, draft_snapshot) = {
            let local = self.manuscript.books.get(&book_id).unwrap();
            (
                local.revision,
                local.baseline.clone(),
                copy_draft(&local.draft),
            )
        };
        let command = ManuscriptCommand {
            expected_revision: revision,
            expected_baseline: expected_baseline.clone(),
            original: Some(book_id.clone()),
            draft: draft_snapshot,
        };
        let (preview_index, preview_error) = if read_only {
            (index.clone(), None)
        } else {
            match self.project.preview_manuscript(revision, &command) {
                Ok(preview) => (preview, None),
                Err(error) => (index.clone(), Some(error)),
            }
        };
        let objects = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.result.analysis.catalog.objects.clone())
            .unwrap_or_default();
        let mut local = self.manuscript.books.remove(&book_id).unwrap();

        let mut apply_book = false;
        let mut load_body_for = None;
        let mut apply_body_for = None;
        let mut create_chapter = false;
        let mut create_section = false;
        let mut repair_chapter = None;
        egui::CentralPanel::default()
            .frame(theme::panel())
            .show(ctx, |ui| {
                ui.heading("书稿工作台");
                ui.label(theme::muted(
                    "书稿只决定阅读顺序，独立于世界时间与事件控制流；不改变事件出口或运行指纹。",
                ));
                ui.horizontal_wrapped(|ui| {
                    if ui.button("新建书稿").clicked() {
                        self.manuscript.creating_new = true;
                    }
                    if indices.len() > 1 {
                        egui::ComboBox::from_id_salt("manuscript-book-picker")
                            .selected_text(index.title.as_deref().unwrap_or(&book_id))
                            .show_ui(ui, |ui| {
                                for (id, candidate) in &indices {
                                    ui.selectable_value(
                                        &mut self.manuscript.selected_book,
                                        Some(id.clone()),
                                        format!(
                                            "{} · {}",
                                            candidate.title.as_deref().unwrap_or(id),
                                            id
                                        ),
                                    );
                                }
                            });
                    } else {
                        ui.label(
                            RichText::new(index.title.as_deref().unwrap_or(&book_id)).strong(),
                        );
                    }
                    ui.selectable_value(&mut self.manuscript.layout, Layout::Tree, "章节树");
                    ui.selectable_value(&mut self.manuscript.layout, Layout::List, "列表");
                    ui.selectable_value(&mut self.manuscript.layout, Layout::Cards, "卡片");
                    ui.checkbox(&mut self.manuscript.reader_open, "阅读预览");
                    if read_only {
                        ui.label(theme::muted("只读书稿"));
                    }
                });
                if !index.diagnostics.is_empty() {
                    for diagnostic in &index.diagnostics {
                        ui.colored_label(
                            theme::ERROR,
                            format!("{}：{}", diagnostic.code, diagnostic.message),
                        );
                    }
                }
                if let Some(error) = &preview_error {
                    ui.colored_label(
                        theme::ERROR,
                        format!("书稿基线不可用：{error}。草稿已保留。"),
                    );
                } else if expected_baseline != current_baseline {
                    ui.colored_label(
                        theme::ERROR,
                        "工程内容已变化；保留当前书稿草稿，应用前需解决过期基线。",
                    );
                }
                if local.changed {
                    ui.label(theme::muted("有未应用的书稿草稿；切换视图会保留输入。"));
                }
                ui.horizontal(|ui| {
                    ui.label("书名");
                    if ui.text_edit_singleline(&mut local.draft.title).changed() {
                        local.changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!read_only, egui::Button::new("插入分节"))
                        .clicked()
                    {
                        create_section = true;
                    }
                    if ui
                        .add_enabled(!read_only, egui::Button::new("插入章节"))
                        .clicked()
                    {
                        create_chapter = true;
                    }
                    if ui
                        .add_enabled(
                            !read_only && local.changed && preview_error.is_none(),
                            egui::Button::new("应用书稿"),
                        )
                        .clicked()
                    {
                        apply_book = true;
                    }
                });

                if create_section {
                    let parent_id = super::editing::selected_section(&local);
                    let id = unique_id(&local.draft.entries, "section");
                    local.draft.entries.push(ManuscriptEntryDraft {
                        id: id.clone(),
                        kind: ManuscriptEntryKind::Section,
                        parent_id,
                        title: "新分节".into(),
                        summary: None,
                        pov: None,
                        status: None,
                        goal: None,
                        target_ref: None,
                    });
                    local.selected_entry = Some(id);
                    local.changed = true;
                }
                if create_chapter {
                    let parent_id = super::editing::selected_section(&local);
                    let id = unique_id(&local.draft.entries, "chapter");
                    let target_ref = objects
                        .iter()
                        .find(|object| {
                            matches!(object.target.kind.as_str(), "event" | "scene" | "entity")
                        })
                        .map(|object| object.target.clone());
                    local.draft.entries.push(ManuscriptEntryDraft {
                        id: id.clone(),
                        kind: ManuscriptEntryKind::Chapter,
                        parent_id,
                        title: "新章节".into(),
                        summary: None,
                        pov: None,
                        status: Some("draft".into()),
                        goal: None,
                        target_ref,
                    });
                    local.selected_entry = Some(id);
                    local.changed = true;
                }

                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("manuscript-workbench-content")
                    .show(ui, |ui| {
                let layout = self.manuscript.layout;
                let entries: Vec<_> = local
                    .draft
                    .entries
                    .iter()
                    .map(|entry| {
                        (
                            entry.id.clone(),
                            entry.kind,
                            entry.title.clone(),
                            entry.parent_id.as_ref().map(|parent| {
                                local
                                    .draft
                                    .entries
                                    .iter()
                                    .find(|candidate| &candidate.id == parent)
                                    .map(|candidate| candidate.title.clone())
                                    .unwrap_or_else(|| parent.clone())
                            }),
                            entry_depth(&local.draft.entries, entry),
                        )
                    })
                    .collect();
                let editor_context = super::editing::EntryEditorContext {
                    book_id: &book_id,
                    objects: &objects,
                    preview_index: &preview_index,
                    read_only,
                };
                if ui.available_width() >= 820.0 {
                    ui.columns(2, |columns| {
                        super::editing::draw_entry_list(&mut columns[0], layout, &mut local, &entries);
                        super::editing::draw_entry_editor(
                            self,
                            &mut columns[1],
                            &mut local,
                            &editor_context,
                            &mut load_body_for,
                            &mut apply_body_for,
                        );
                    });
                } else {
                    ui.heading("章节");
                    let selected_label = local
                        .draft
                        .entries
                        .iter()
                        .find(|entry| {
                            local.selected_entry.as_deref() == Some(entry.id.as_str())
                        })
                        .map(|entry| format!("{} · {}", entry.title, entry.id))
                        .unwrap_or_else(|| "选择章节".into());
                    egui::ComboBox::from_id_salt(("manuscript-entry-narrow", &book_id))
                        .selected_text(selected_label)
                        .show_ui(ui, |ui| {
                            for (id, kind, title, parent, depth) in &entries {
                                let label = match layout {
                                    Layout::Tree => format!(
                                        "{}{}{} · {}",
                                        "  ".repeat(*depth),
                                        if *kind == ManuscriptEntryKind::Section {
                                            "▾ "
                                        } else {
                                            ""
                                        },
                                        title,
                                        id
                                    ),
                                    Layout::List => {
                                        if *kind != ManuscriptEntryKind::Chapter {
                                            continue;
                                        }
                                        let section = parent
                                            .as_deref()
                                            .map(|parent| format!("{parent} / "))
                                            .unwrap_or_default();
                                        format!("{section}{title} · {id}")
                                    }
                                    Layout::Cards => {
                                        if *kind != ManuscriptEntryKind::Chapter {
                                            continue;
                                        }
                                        format!("{title} · {id}")
                                    }
                                };
                                ui.selectable_value(
                                    &mut local.selected_entry,
                                    Some(id.clone()),
                                    label,
                                );
                            }
                        });
                    ui.separator();
                    super::editing::draw_entry_editor(
                        self,
                        ui,
                        &mut local,
                        &editor_context,
                        &mut load_body_for,
                        &mut apply_body_for,
                    );
                }

                if self.manuscript.reader_open {
                    ui.separator();
                    ui.heading("阅读预览");
                ui.label(theme::muted(
                    "按书稿章节顺序展示 core 编译的静态文本；分支按源码顺序列出，不模拟世界时间或事件控制流。",
                ));
                    egui::ScrollArea::vertical()
                        .id_salt("manuscript-reader-preview")
                        .max_height(420.0)
                        .show(ui, |ui| {
                            super::preview::draw_reader_preview(self, ui, &preview_index, &mut repair_chapter);
                        });
                }
                });
            });

        if let Some(chapter) = repair_chapter {
            local.selected_entry = Some(chapter);
        }
        self.manuscript.books.insert(book_id.clone(), local);

        if let Some((book, chapter)) = load_body_for {
            self.load_manuscript_body(&book, &chapter, &preview_index);
        }
        if let Some((book, chapter)) = apply_body_for {
            self.apply_manuscript_body(&book, &chapter);
        }
        if apply_book {
            self.apply_manuscript_book(&book_id);
        }
    }

    fn manuscript_creation_tab(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(theme::panel())
            .show(ctx, |ui| {
                ui.heading("新建书稿");
                ui.label("只有清单注册的书稿会显示在这里；创建时由 core 同步更新注册和书稿文档。");
                if self.manuscript.selected_book.is_some() && ui.button("返回当前书稿").clicked()
                {
                    self.manuscript.creating_new = false;
                }
                ui.horizontal(|ui| {
                    ui.label("稳定 ID");
                    self.manuscript.create_touched |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.manuscript.new_id)
                                .hint_text("例如 novel"),
                        )
                        .changed();
                });
                ui.horizontal(|ui| {
                    ui.label("书名");
                    self.manuscript.create_touched |= ui
                        .add(
                            egui::TextEdit::singleline(&mut self.manuscript.new_title)
                                .hint_text("书稿名称"),
                        )
                        .changed();
                });
                if ui
                    .add_enabled(
                        !self.manuscript.new_id.trim().is_empty()
                            && !self.manuscript.new_title.trim().is_empty(),
                        egui::Button::new("创建并打开书稿"),
                    )
                    .clicked()
                {
                    self.create_manuscript();
                }
            });
    }

    fn create_manuscript(&mut self) {
        let draft = ManuscriptDraft {
            id: self.manuscript.new_id.trim().to_owned(),
            title: self.manuscript.new_title.trim().to_owned(),
            entries: Vec::new(),
        };
        let mut revision = Revision::default();
        let command = ManuscriptCommand {
            expected_revision: revision,
            expected_baseline: self.project.content_baseline(),
            original: None,
            draft,
        };
        if let Err(error) = self.project.preview_manuscript(revision, &command) {
            self.io_error = Some(error);
            return;
        }
        let before = self.project.clone();
        match self.project.apply_manuscript(&mut revision, command) {
            Ok(_) => {
                self.remember(before);
                let id = self.manuscript.new_id.trim().to_owned();
                if let Ok(index) = self.project.manuscript_index(&id) {
                    self.manuscript.books.insert(
                        id.clone(),
                        LocalBook {
                            draft: ManuscriptDraft::from_index(&index),
                            baseline: self.project.content_baseline(),
                            revision,
                            selected_entry: None,
                            changed: false,
                        },
                    );
                    self.manuscript.selected_book = Some(id);
                }
                self.manuscript.create_touched = false;
                self.manuscript.creating_new = false;
                self.recompile();
                self.message = Some("书稿已创建；书稿与注册由 core 一次写入".into());
                self.io_error = None;
            }
            Err(error) => self.io_error = Some(error),
        }
    }

    fn apply_manuscript_book(&mut self, id: &str) {
        let Some(local) = self.manuscript.books.get(id) else {
            return;
        };
        let mut revision = local.revision;
        let previous_baseline = local.baseline.clone();
        let command = ManuscriptCommand {
            expected_revision: revision,
            expected_baseline: previous_baseline.clone(),
            original: Some(id.into()),
            draft: copy_draft(&local.draft),
        };
        if let Err(error) = self.project.preview_manuscript(revision, &command) {
            self.io_error = Some(format!("书稿未应用，输入已保留：{error}"));
            return;
        }
        let before = self.project.clone();
        match self.project.apply_manuscript(&mut revision, command) {
            Ok(_) => {
                let new_baseline = self.project.content_baseline();
                self.remember(before);
                if let Some(local) = self.manuscript.books.get_mut(id) {
                    if let Ok(index) = self.project.manuscript_index(id) {
                        local.draft = ManuscriptDraft::from_index(&index);
                    }
                    local.baseline = new_baseline.clone();
                    local.revision = revision;
                    local.changed = false;
                }
                for ((book, _), body) in &mut self.manuscript.body_drafts {
                    if book == id && body.baseline == previous_baseline {
                        body.baseline = new_baseline.clone();
                    }
                }
                self.recompile();
                self.message = Some("书稿编排已应用；运行内容指纹不变".into());
                self.io_error = None;
            }
            Err(error) => self.io_error = Some(format!("书稿未应用，输入已保留：{error}")),
        }
    }

    fn load_manuscript_body(&mut self, book: &str, chapter: &str, index: &ManuscriptIndex) {
        let Some(location) = index
            .entries
            .iter()
            .find(|entry| entry.id == chapter)
            .and_then(|entry| entry.source.as_ref())
            .and_then(|source| source.location.as_ref())
        else {
            self.io_error = Some("章节来源尚未解析，不能打开正文草稿".into());
            return;
        };
        let path = super::super::workspace_source_path(&self.project, Path::new(&location.file));
        match self.project.document(&path) {
            Ok(source) => {
                self.manuscript.body_drafts.insert(
                    (book.into(), chapter.into()),
                    BodyDraft {
                        path,
                        original: source.to_owned(),
                        text: source.to_owned(),
                        baseline: self.project.content_baseline(),
                        open: true,
                    },
                );
                self.io_error = None;
            }
            Err(error) => self.io_error = Some(error),
        }
    }

    fn apply_manuscript_body(&mut self, book: &str, chapter: &str) {
        let key = (book.to_owned(), chapter.to_owned());
        let Some(body) = self.manuscript.body_drafts.get(&key) else {
            return;
        };
        if body.baseline != self.project.content_baseline() {
            self.io_error =
                Some("正文草稿基线已过期；输入已保留，请检查外部修改后重新打开来源。".into());
            return;
        }
        if body.text == body.original {
            self.io_error = Some("正文没有未提交的修改".into());
            return;
        }
        let path = body.path.clone();
        let text = body.text.clone();
        let previous_baseline = body.baseline.clone();
        let before = self.project.clone();
        match self.project.edit(|project| project.set_text(&path, text)) {
            Ok(()) => {
                let new_baseline = self.project.content_baseline();
                self.remember(before);
                self.manuscript.body_drafts.remove(&key);
                if let Some(local) = self.manuscript.books.get_mut(book) {
                    if local.baseline == previous_baseline {
                        local.baseline = new_baseline;
                    }
                }
                self.recompile();
                self.message = Some("正文来源已应用；此操作可撤销".into());
                self.io_error = None;
            }
            Err(error) => self.io_error = Some(error),
        }
    }
}
