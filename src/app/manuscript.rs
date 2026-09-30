//! 注册书稿工作台：结构和统计来自 core，正文始终读取原有源码。
mod editing;
mod outline;
mod preview;
mod transactions;
mod workbench;

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

struct ReaderPart {
    text: String,
    target: Option<TargetRef>,
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
    new_id: String,
    new_title: String,
    create_touched: bool,
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
            new_id: String::new(),
            new_title: String::new(),
            create_touched: false,
        }
    }
}

impl WorkbenchState {
    pub(super) fn has_unsubmitted_work(&self) -> bool {
        (self.create_touched && (!self.new_id.is_empty() || !self.new_title.is_empty()))
            || self.books.values().any(|book| book.changed)
            || self.writing_buffers.values().any(WritingBuffer::is_changed)
    }

    pub(super) fn rebase_clean(&mut self, project: &worldline_core::project::Project) {
        let indices = project.manuscript_indices();
        self.books
            .retain(|id, local| indices.contains_key(id) || local.changed);
        for (id, local) in &mut self.books {
            if local.changed {
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
        self.writing_buffers.retain(|_, buffer| buffer.is_changed());
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

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(in crate::app) struct ManuscriptSession {
    pub manuscript_id: Option<String>,
    pub selected_id: Option<String>,
    pub scroll_y: f32,
    pub cursor: Option<super::writing_workspace::WritingCursor>,
    pub mode: super::writing_workspace::Mode,
}

impl super::WorldeditApp {
    pub(in crate::app) fn manuscript_session(&self) -> ManuscriptSession {
        ManuscriptSession {
            manuscript_id: self.manuscript.selected_book.clone(),
            mode: self.manuscript.writing_view.session_mode(),
            scroll_y: self.manuscript.scroll_y,
            cursor: self
                .manuscript
                .writing_view
                .session_cursor()
                .filter(|cursor| {
                    self.manuscript
                        .selected_book
                        .as_ref()
                        .and_then(|id| self.manuscript.books.get(id))
                        .and_then(|book| {
                            book.selected_entry.as_ref().and_then(|id| {
                                book.draft.entries.iter().find(|entry| &entry.id == id)
                            })
                        })
                        .and_then(|entry| entry.target_ref.as_ref())
                        == Some(&cursor.target)
                }),
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
