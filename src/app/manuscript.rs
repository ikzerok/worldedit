//! 注册书稿工作台：结构和统计来自 core，正文始终读取原有源码。
mod editing;
mod preview;
mod workbench;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use worldline_core::catalog::{CatalogObject, TargetRef};
use worldline_core::manuscript::{
    ManuscriptDraft, ManuscriptEntryDraft, ManuscriptEntryKind, ManuscriptReferenceStatus,
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
    baseline: String,
    revision: Revision,
    selected_entry: Option<String>,
    changed: bool,
}

struct BodyDraft {
    path: PathBuf,
    original: String,
    text: String,
    baseline: String,
    open: bool,
}

struct ReaderPart {
    text: String,
    target: Option<TargetRef>,
}

pub(super) struct WorkbenchState {
    selected_book: Option<String>,
    creating_new: bool,
    books: BTreeMap<String, LocalBook>,
    body_drafts: HashMap<(String, String), BodyDraft>,
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
            body_drafts: HashMap::new(),
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
        self.create_touched
            || self.books.values().any(|book| book.changed)
            || self
                .body_drafts
                .values()
                .any(|draft| draft.text != draft.original)
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
            local.baseline = project.content_baseline();
            local.selected_entry = local
                .draft
                .entries
                .iter()
                .find(|entry| entry.kind == ManuscriptEntryKind::Chapter)
                .or_else(|| local.draft.entries.first())
                .map(|entry| entry.id.clone());
        }
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

fn entry_depth(entries: &[ManuscriptEntryDraft], entry: &ManuscriptEntryDraft) -> usize {
    let mut depth = 0;
    let mut parent = entry.parent_id.as_deref();
    while let Some(id) = parent {
        depth += 1;
        parent = entries
            .iter()
            .find(|candidate| candidate.id == id)
            .and_then(|candidate| candidate.parent_id.as_deref());
        if depth > entries.len() {
            break;
        }
    }
    depth
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
