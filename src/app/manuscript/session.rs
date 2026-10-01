use super::*;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(in crate::app) struct ManuscriptSession {
    pub manuscript_id: Option<String>,
    pub selected_id: Option<String>,
    pub scroll_y: f32,
    pub preview_open: Option<bool>,
    pub preview_whole_book: Option<bool>,
    pub preview_tab: Option<bool>,
    pub cursor: Option<crate::app::writing_workspace::WritingCursor>,
    pub mode: crate::app::writing_workspace::Mode,
    pub anchor: Option<WritingAnchor>,
    pub restore_offsets: Option<bool>,
}

impl crate::app::WorldeditApp {
    pub(in crate::app) fn manuscript_session(&self) -> ManuscriptSession {
        ManuscriptSession {
            manuscript_id: self.manuscript.selected_book.clone(),
            mode: self.manuscript.writing_view.session_mode(),
            scroll_y: self.manuscript.scroll_y,
            preview_open: Some(self.manuscript.reader_open),
            preview_whole_book: Some(self.manuscript.reader_whole_book),
            preview_tab: Some(self.manuscript.narrow_preview),
            cursor: self
                .manuscript
                .active_writing_target()
                .and_then(|(target, path)| {
                    self.manuscript
                        .writing_buffers
                        .get(&path)
                        .and_then(|buffer| {
                            self.manuscript
                                .writing_view
                                .cursor_for_buffer(buffer, &target)
                        })
                }),
            anchor: self
                .manuscript
                .active_writing_target()
                .and_then(|(target, path)| {
                    self.manuscript
                        .writing_buffers
                        .get(&path)
                        .map(|buffer| WritingAnchor {
                            path,
                            target,
                            source_baseline: crate::app::writing_workspace::fingerprint(
                                buffer.source(),
                            ),
                            buffer_baseline: buffer.baseline().into(),
                            generation: buffer.generation(),
                        })
                }),
            restore_offsets: Some(true),
            selected_id: self
                .manuscript
                .selected_book
                .as_ref()
                .and_then(|id| self.manuscript.books.get(id))
                .and_then(|book| book.selected_entry.clone()),
        }
    }

    pub(in crate::app) fn restore_manuscript_session(&mut self, session: ManuscriptSession) {
        self.manuscript.pending_session = Some(session);
    }
}

/// 元数据锚不持有可编辑原稿；导航必须仍指向同一文件与目标。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(in crate::app) struct WritingAnchor {
    pub path: PathBuf,
    pub target: TargetRef,
    pub source_baseline: String,
    pub buffer_baseline: String,
    pub generation: u64,
}

impl WorkbenchState {
    pub(in crate::app) fn validate_session(
        &self,
        project: &worldline_core::project::Project,
        session: &ManuscriptSession,
    ) -> Result<bool, String> {
        if session
            .anchor
            .as_ref()
            .is_some_and(|anchor| project.document(&anchor.path).is_err())
        {
            return Err("原来源文件已不存在，保留当前入口与草稿".into());
        }
        let Some(book) = session.manuscript_id.as_ref() else {
            return Ok(false);
        };
        let loaded;
        let draft = if let Some(local) = self
            .books
            .get(book)
            .filter(|local| local.changed || local.baseline == project.content_baseline())
        {
            &local.draft
        } else {
            loaded = ManuscriptDraft::from_index(
                &project
                    .manuscript_index(book)
                    .map_err(|_| "原书稿已不存在，保留当前入口与草稿")?,
            );
            &loaded
        };
        let Some(chapter) = session.selected_id.as_ref() else {
            return Ok(false);
        };
        let entry = draft
            .entries
            .iter()
            .find(|entry| &entry.id == chapter)
            .ok_or("原章节已不存在，保留当前入口与草稿")?;
        let Some(anchor) = &session.anchor else {
            return Ok(false);
        };
        if entry.target_ref.as_ref() != Some(&anchor.target) {
            return Err("原章节的来源目标已变化，保留当前入口与草稿".into());
        }
        let cached = self
            .chapter_sources
            .get(&(book.clone(), chapter.clone()))
            .is_some_and(|(target, path)| target == &anchor.target && path == &anchor.path);
        let resolved = project.open_writing_buffer(&anchor.target);
        if resolved
            .as_ref()
            .is_ok_and(|buffer| buffer.path() != anchor.path)
            || (!cached && resolved.as_ref().is_err())
        {
            return Err("原章节的来源文件无法确认，保留当前入口与草稿".into());
        }
        let opened;
        let buffer = if let Some(buffer) = self.writing_buffers.get(&anchor.path) {
            buffer
        } else {
            opened = project
                .open_source_writing_buffer(&anchor.path)
                .map_err(|_| "原来源文件已不存在，保留当前入口与草稿")?;
            &opened
        };
        Ok(
            anchor.source_baseline == crate::app::writing_workspace::fingerprint(buffer.source())
                && anchor.buffer_baseline == buffer.baseline()
                && anchor.buffer_baseline == project.content_baseline()
                && anchor.generation == buffer.generation(),
        )
    }
}
