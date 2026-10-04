//! 先确认完整范围对应的 core 投影，再导航；不改写或重建现有文件草稿。
use super::*;
use crate::app::writing_workspace::Mode;
use std::{ops::Range, path::Path};
use worldline_core::{manuscript::WritingBlockKind, project::Project};

pub(in crate::app) struct WritingMatchNavigation {
    book: String,
    chapter: String,
    target: TargetRef,
    path: PathBuf,
    mode: Mode,
    pub reason: Option<String>,
}

impl WritingMatchNavigation {
    pub(super) fn into_source(mut self) -> Self {
        self.mode = Mode::Source;
        self
    }
}

impl WorkbenchState {
    pub(in crate::app) fn plan_writing_match(
        &self,
        path: &Path,
        range: &Range<usize>,
        project: &Project,
    ) -> Option<WritingMatchNavigation> {
        let opened;
        let buffer =
            if let Some(buffer) = self.writing_buffers.get(path).filter(|buffer| {
                buffer.is_changed() || buffer.baseline() == project.content_baseline()
            }) {
                buffer
            } else {
                opened = project.open_source_writing_buffer(path).ok()?;
                &opened
            };
        buffer.source().get(range.clone())?;
        let mut books: BTreeMap<_, _> = project
            .manuscript_indices()
            .into_iter()
            .map(|(id, index)| (id, ManuscriptDraft::from_index(&index)))
            .collect();
        for (id, local) in &self.books {
            if local.changed || local.baseline == project.content_baseline() {
                books.insert(id.clone(), local.draft.clone());
            }
        }
        let mut candidates = Vec::new();
        for (book, draft) in &books {
            for chapter in &draft.entries {
                let Some(target) = &chapter.target_ref else {
                    continue;
                };
                let same_file = self
                    .chapter_sources
                    .get(&(book.clone(), chapter.id.clone()))
                    .is_some_and(|(cached, source)| cached == target && source == path)
                    || project
                        .open_writing_buffer(target)
                        .is_ok_and(|source| source.path() == path);
                if same_file {
                    let current = self.selected_book.as_ref() == Some(book)
                        && self
                            .books
                            .get(book)
                            .and_then(|local| local.selected_entry.as_ref())
                            == Some(&chapter.id);
                    candidates.push((!current, book.clone(), chapter.id.clone(), target.clone()));
                }
            }
        }
        candidates.sort_by_key(|candidate| candidate.0);
        let candidate = candidates
            .iter()
            .find(|(_, _, _, target)| {
                project
                    .project_writing_buffer(buffer, target)
                    .is_ok_and(|projection| {
                        projection.range.start <= range.start && range.end <= projection.range.end
                    })
            })
            .or_else(|| candidates.first())?;
        let (_, book, chapter, target) = candidate;
        let current_mode = self.writing_view.session_mode();
        let reason = if current_mode == Mode::Source {
            None
        } else {
            match project.project_writing_buffer(buffer, target) {
                Err(error) => Some(format!("当前稿无法形成准确章节投影：{error}")),
                Ok(projection)
                    if range.start < projection.range.start || range.end > projection.range.end =>
                {
                    Some("命中跨越章节声明体或位于声明外".into())
                }
                Ok(projection) if current_mode == Mode::Structure => {
                    (!crate::app::search::selection_is_representable(
                        buffer.source(),
                        projection.range.start,
                        &projection.source,
                        range,
                    ))
                    .then(|| "命中无法准确映射到当前结构编辑区".into())
                }
                Ok(projection) => {
                    let representable = projection.blocks.iter().any(|block| {
                        block.kind == WritingBlockKind::Prose
                            && crate::app::search::selection_is_representable(
                                buffer.source(),
                                block.range.start,
                                &block.text,
                                range,
                            )
                    });
                    (!representable).then(|| "命中包含结构或跨越正文编辑块".into())
                }
            }
        };
        Some(WritingMatchNavigation {
            book: book.clone(),
            chapter: chapter.clone(),
            target: target.clone(),
            path: path.into(),
            mode: if reason.is_some() {
                Mode::Source
            } else {
                current_mode
            },
            reason,
        })
    }

    pub(in crate::app) fn apply_writing_match(
        &mut self,
        navigation: WritingMatchNavigation,
        project: &Project,
    ) -> Result<(), String> {
        if self
            .books
            .values()
            .any(|book| !book.changed && book.baseline != project.content_baseline())
            || self.writing_buffers.values().any(|buffer| {
                !buffer.is_changed() && buffer.baseline() != project.content_baseline()
            })
        {
            self.rebase_clean(project);
        }
        if !self.writing_buffers.contains_key(&navigation.path) {
            let buffer = project.open_source_writing_buffer(&navigation.path)?;
            self.writing_buffers.insert(navigation.path.clone(), buffer);
        }
        if !self.books.contains_key(&navigation.book) {
            let index = project.manuscript_index(&navigation.book)?;
            let draft = ManuscriptDraft::from_index(&index);
            self.books.insert(
                navigation.book.clone(),
                LocalBook {
                    original: draft.clone(),
                    draft,
                    baseline: project.content_baseline(),
                    revision: Revision::default(),
                    selected_entry: None,
                    changed: false,
                    collapsed: HashSet::new(),
                },
            );
        }
        self.selected_book = Some(navigation.book.clone());
        self.books.get_mut(&navigation.book).unwrap().selected_entry =
            Some(navigation.chapter.clone());
        self.chapter_sources.insert(
            (navigation.book, navigation.chapter),
            (navigation.target, navigation.path),
        );
        self.narrow_preview = false;
        self.pending_session = None;
        self.pending_scroll = None;
        self.writing_view.restore_mode(navigation.mode);
        Ok(())
    }
}
