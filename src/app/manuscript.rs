//! 注册书稿工作台：结构和统计来自 core，正文始终读取原有源码。
mod creation;
mod creation_form;
mod creation_plan;
#[cfg(test)]
mod creation_tests;
mod editing;
mod focus_controls;
mod layout;
mod outline;
mod preview;
mod recovery;
mod review_navigation;
mod review_render;
mod runtime_drafts;
mod search_navigation;
mod session;
mod transactions;
mod workbench;
mod writing;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use worldline_core::catalog::{CatalogObject, TargetRef};
use worldline_core::manuscript::{
    ManuscriptDraft, ManuscriptEntryDraft, ManuscriptEntryKind, ManuscriptReferenceStatus,
    WritingBuffer,
};
use worldline_core::presentation_commands::Revision;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Layout {
    #[default]
    Tree,
    List,
    Cards,
}

struct LocalBook {
    draft: ManuscriptDraft,
    original: ManuscriptDraft,
    baseline: String,
    revision: Revision,
    selected_entry: Option<String>,
    changed: bool,
    collapsed: HashSet<String>,
}

pub(super) struct WorkbenchState {
    selected_book: Option<String>,
    creating_new: bool,
    books: BTreeMap<String, LocalBook>,
    writing_buffers: HashMap<PathBuf, WritingBuffer>,
    chapter_sources: HashMap<(String, String), (TargetRef, PathBuf)>,
    writing_view: super::writing_workspace::ViewState,
    status_filter: String,
    pov_filter: String,
    pending_remove: Option<String>,
    pending_session: Option<ManuscriptSession>,
    scroll_y: f32,
    pending_scroll: Option<f32>,
    layout: Layout,
    reader_open: bool,
    reader_whole_book: bool,
    narrow_preview: bool,
    focus_management: bool,
    preview_cache: preview::PreviewCache,
    review_navigation: review_navigation::ReviewNavigation,
    review_scroll_y: f32,
    review_page_offset: usize,
    pending_review_scroll: Option<f32>,
    review_focus: bool,
    creation: Option<creation::CreationState>,
    creation_dismissed: bool,
    orphan_discard_confirm: Option<PathBuf>,
}

impl Default for WorkbenchState {
    fn default() -> Self {
        Self {
            selected_book: None,
            creating_new: false,
            books: BTreeMap::new(),
            writing_buffers: HashMap::new(),
            chapter_sources: HashMap::new(),
            writing_view: Default::default(),
            status_filter: String::new(),
            pov_filter: String::new(),
            pending_remove: None,
            pending_session: None,
            scroll_y: 0.0,
            pending_scroll: None,
            layout: Layout::Tree,
            reader_open: true,
            reader_whole_book: false,
            narrow_preview: false,
            focus_management: false,
            preview_cache: Default::default(),
            review_navigation: Default::default(),
            review_scroll_y: 0.0,
            review_page_offset: 0,
            pending_review_scroll: None,
            review_focus: false,
            creation: None,
            creation_dismissed: false,
            orphan_discard_confirm: None,
        }
    }
}

impl WorkbenchState {
    pub(in crate::app) fn writing_buffers(&self) -> Vec<WritingBuffer> {
        self.writing_buffers.values().cloned().collect()
    }

    pub(in crate::app) fn writing_buffer_mut(
        &mut self,
        path: &std::path::Path,
    ) -> Option<&mut WritingBuffer> {
        self.writing_buffers.get_mut(path)
    }

    pub(in crate::app) fn comment_selection_is_current_mode(&self) -> bool {
        self.writing_view.selection_is_current_mode()
    }

    pub(in crate::app) fn active_writing_target(&self) -> Option<(TargetRef, PathBuf)> {
        let book = self.selected_book.as_ref()?;
        let local = self.books.get(book)?;
        let chapter = local.selected_entry.as_ref()?;
        self.chapter_sources
            .get(&(book.clone(), chapter.clone()))
            .cloned()
    }

    pub(in crate::app) fn restore_writing_buffers(&mut self, buffers: &[WritingBuffer]) {
        for buffer in buffers {
            self.writing_buffers
                .insert(buffer.path().to_owned(), buffer.clone());
        }
    }

    pub(in crate::app) fn clear_applied_writing_buffers(&mut self, paths: &[PathBuf]) {
        self.writing_buffers.retain(|path, _| !paths.contains(path));
        self.chapter_sources
            .retain(|_, (_, path)| !paths.contains(path));
    }

    pub(super) fn unapplied_sources(&self) -> Vec<String> {
        let mut sources: Vec<_> = self
            .writing_buffers
            .values()
            .filter(|buffer| buffer.is_changed())
            .map(|buffer| format!("正文 · {}", buffer.path().display()))
            .chain(
                self.books
                    .iter()
                    .filter(|(_, book)| book.changed)
                    .map(|(id, _)| format!("书稿编排 · manuscript:{id}")),
            )
            .collect();
        if let Some(form) = self.creation.as_ref().filter(|form| form.touched) {
            sources.push(format!("新建章节 · {}", form.chapter_title));
        }
        if self.writing_view.has_retained_input() {
            sources.push("未插入的保留正文输入".into());
        }
        sources.sort();
        sources
    }

    pub(super) fn has_unsubmitted_work(&self) -> bool {
        self.creation.as_ref().is_some_and(|form| form.touched)
            || self.writing_view.has_retained_input()
            || self.books.values().any(|book| book.changed)
            || self.writing_buffers.values().any(WritingBuffer::is_changed)
    }

    pub(super) fn rebase_clean(&mut self, project: &worldline_core::project::Project) {
        self.rebase_clean_preserving(project, None);
    }

    pub(super) fn rebase_clean_preserving(
        &mut self,
        project: &worldline_core::project::Project,
        receiver_book: Option<&str>,
    ) {
        let indices = project.manuscript_indices();
        self.books.retain(|id, local| {
            indices.contains_key(id) || local.changed || receiver_book == Some(id.as_str())
        });
        for (id, local) in &mut self.books {
            if local.changed || receiver_book == Some(id.as_str()) {
                continue;
            }
            let Some(index) = indices.get(id) else {
                continue;
            };
            local.draft = ManuscriptDraft::from_index(index);
            local.original = local.draft.clone();
            local.baseline = project.content_baseline();
            local.selected_entry = local
                .selected_entry
                .take()
                .filter(|id| local.draft.entries.iter().any(|entry| &entry.id == id))
                .or_else(|| {
                    local
                        .draft
                        .entries
                        .iter()
                        .find(|entry| entry.kind == ManuscriptEntryKind::Chapter)
                        .or_else(|| local.draft.entries.first())
                        .map(|entry| entry.id.clone())
                });
        }
        self.writing_buffers.retain(|_, buffer| {
            buffer.is_changed() || self.writing_view.has_retained_for(buffer.path())
        });
        self.chapter_sources
            .retain(|_, (_, path)| self.writing_buffers.contains_key(path));
    }
}

fn copy_draft(draft: &ManuscriptDraft) -> ManuscriptDraft {
    ManuscriptDraft {
        id: draft.id.clone(),
        title: draft.title.clone(),
        entries: draft
            .entries
            .iter()
            .map(|entry| ManuscriptEntryDraft {
                id: entry.id.clone(),
                kind: entry.kind,
                parent_id: entry.parent_id.clone(),
                title: entry.title.clone(),
                summary: entry.summary.clone(),
                pov: entry.pov.clone(),
                status: entry.status.clone(),
                goal: entry.goal.clone(),
                target_ref: entry.target_ref.clone(),
            })
            .collect(),
    }
}

fn unique_id(entries: &[ManuscriptEntryDraft], base: &str) -> String {
    let used = |candidate: &str| entries.iter().any(|entry| entry.id == candidate);
    if !used(base) {
        return base.into();
    }
    (2..)
        .map(|number| format!("{base}_{number}"))
        .find(|candidate| !used(candidate))
        .unwrap_or_else(|| base.into())
}

fn move_entry(entries: &mut [ManuscriptEntryDraft], id: &str, delta: isize) -> bool {
    let Some(index) = entries.iter().position(|entry| entry.id == id) else {
        return false;
    };
    let parent = entries[index].parent_id.clone();
    let siblings: Vec<_> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| (entry.parent_id == parent).then_some(index))
        .collect();
    let Some(position) = siblings.iter().position(|sibling| *sibling == index) else {
        return false;
    };
    let target = position as isize + delta;
    if !(0..siblings.len() as isize).contains(&target) {
        return false;
    }
    entries.swap(index, siblings[target as usize]);
    true
}

fn target_label(object: &CatalogObject) -> String {
    format!(
        "{} · {}:{}",
        object.display, object.target.kind, object.target.id
    )
}

fn source_status_text(status: ManuscriptReferenceStatus) -> &'static str {
    match status {
        ManuscriptReferenceStatus::Resolved => "来源已解析",
        ManuscriptReferenceStatus::Missing => "来源缺失，可在此修复引用",
        ManuscriptReferenceStatus::Unresolved => "编译错误或语言版本限制使来源暂不可确认",
        ManuscriptReferenceStatus::Invalid => "引用类型无效",
    }
}

pub(in crate::app) use session::ManuscriptSession;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod review_tests;
