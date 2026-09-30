use super::*;
use crate::theme;
use worldline_core::manuscript::ManuscriptCommand;

impl super::super::WorldeditApp {
    pub(super) fn manuscript_creation_tab(&mut self, ctx: &egui::Context) {
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
                            original: ManuscriptDraft::from_index(&index),
                            baseline: self.project.content_baseline(),
                            revision,
                            selected_entry: None,
                            changed: false,
                            collapsed: HashSet::new(),
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

    pub(super) fn apply_manuscript_book(&mut self, id: &str) {
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
                        local.original = local.draft.clone();
                    }
                    local.baseline = new_baseline.clone();
                    local.revision = revision;
                    local.changed = false;
                }
                for body in self.manuscript.writing_buffers.values_mut() {
                    if body.baseline() == previous_baseline {
                        let _ = body.rebase_unchanged_source(&self.project);
                    }
                }
                self.recompile();
                self.message = Some("书稿编排已应用；运行内容指纹不变".into());
                self.io_error = None;
            }
            Err(error) => self.io_error = Some(format!("书稿未应用，输入已保留：{error}")),
        }
    }

    pub(super) fn apply_manuscript_body(&mut self, path: &std::path::Path, source_mode: bool) {
        let Some(body) = self.manuscript.writing_buffers.get(path) else {
            return;
        };
        if !body.is_changed() {
            return;
        }
        let preview = if source_mode {
            self.project.preview_source_writing_buffer(body).map(|_| ())
        } else {
            self.project.preview_writing_buffer(body)
        };
        if let Err(error) = preview {
            self.io_error = Some(format!("正文未应用，输入已保留：{error}"));
            return;
        }
        let previous_baseline = body.baseline().to_owned();
        let before = self.project.clone();
        let applied = if source_mode {
            self.project.apply_source_writing_buffer(body).map(|_| ())
        } else {
            self.project.apply_writing_buffer(body)
        };
        match applied {
            Ok(()) => {
                self.remember(before);
                self.manuscript.writing_buffers.remove(path);
                let new_baseline = self.project.content_baseline();
                for local in self.manuscript.books.values_mut() {
                    if local.baseline == previous_baseline {
                        local.baseline = new_baseline.clone();
                    }
                }
                for body in self.manuscript.writing_buffers.values_mut() {
                    if body.baseline() == previous_baseline {
                        let _ = body.rebase_unchanged_source(&self.project);
                    }
                }
                self.recompile();
                self.message = Some("正文已应用；可撤销，尚需保存工程".into());
                self.io_error = None;
            }
            Err(error) => self.io_error = Some(format!("正文未应用，输入已保留：{error}")),
        }
    }
}
