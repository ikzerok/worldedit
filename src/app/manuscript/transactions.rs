use super::*;
use worldline_core::manuscript::ManuscriptCommand;

impl super::super::WorldeditApp {
    pub(super) fn discard_manuscript_body(&mut self, path: &std::path::Path) {
        self.manuscript.writing_buffers.remove(path);
        self.manuscript.writing_view.discard_retained_for(path);
        self.discard_writing_draft_history(path);
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
                self.recompile();
                self.message = Some("书稿编排已应用；运行内容指纹不变".into());
                self.io_error = None;
            }
            Err(error) => self.io_error = Some(format!("书稿未应用，输入已保留：{error}")),
        }
    }

    pub(super) fn apply_manuscript_body(&mut self, path: &std::path::Path, source_mode: bool) {
        if self.manuscript.writing_view.has_retained_for(path) {
            self.io_error = Some("此文件仍有未插入的对白或组合输入，请先处理；应用未执行".into());
            return;
        }
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
        let applied_buffer = body.clone();
        let before = self.project.clone();
        let applied = if source_mode {
            self.project.apply_source_writing_buffer(body).map(|_| ())
        } else {
            self.project.apply_writing_buffer(body)
        };
        match applied {
            Ok(()) => {
                self.remember_writing_project_edit(
                    before,
                    vec![applied_buffer],
                    vec![path.to_owned()],
                );
                self.manuscript.writing_buffers.remove(path);
                let new_baseline = self.project.content_baseline();
                for local in self.manuscript.books.values_mut() {
                    if local.baseline == previous_baseline {
                        local.baseline = new_baseline.clone();
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
