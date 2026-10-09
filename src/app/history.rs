//! Project history owns each operation's draft recovery payload and stable endpoints.
use super::{Project, WorldeditApp};
use std::{collections::BTreeSet, path::PathBuf, sync::Arc};
use worldline_core::manuscript::WritingBuffer;

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(super) struct NodeId(u64);

#[derive(Default)]
pub(super) struct HistoryState {
    pub(super) current: NodeId,
    next: u64,
    draft_guard: Option<(NodeId, Arc<Project>)>,
}

impl HistoryState {
    fn fresh(&mut self) -> NodeId {
        self.next = self
            .next
            .checked_add(1)
            .expect("history identity exhausted");
        NodeId(self.next)
    }
}

pub(super) struct HistoryEntry {
    pub(super) snapshot: Project,
    before: NodeId,
    after: NodeId,
    writing: Option<WritingEdit>,
}

struct WritingEdit {
    buffers: Vec<WritingBuffer>,
    paths: Vec<PathBuf>,
}

impl WorldeditApp {
    pub(super) fn remember(&mut self, before: Project) {
        self.remember_history(before, None);
    }

    pub(super) fn remember_writing_project_edit(
        &mut self,
        before: Project,
        buffers: Vec<WritingBuffer>,
        paths: Vec<PathBuf>,
    ) {
        self.remember_history(before, Some(WritingEdit { buffers, paths }));
    }

    fn remember_history(&mut self, snapshot: Project, writing: Option<WritingEdit>) {
        self.manuscript
            .handoff_unchanged_writing_buffers(&snapshot, &self.project);
        self.allow_close = false;
        let before = self.history_state.current;
        let after = self.history_state.fresh();
        self.history.push(HistoryEntry {
            snapshot,
            before,
            after,
            writing,
        });
        if self.history.len() > 40 {
            self.history.remove(0);
        }
        self.history_state.current = after;
        self.history_state.draft_guard = None;
        self.redo.clear();
        self.search_state.redo.clear();
        self.retain_reachable_draft_history();
    }

    /// Entries for other files keep their shared witness; the cache need not own it.
    pub(super) fn clear_draft_history_guard(&mut self) {
        self.history_state.draft_guard = None;
    }

    /// Used when an external refresh/replacement invalidates the graph, not for travel.
    pub(super) fn clear_edit_history(&mut self) {
        self.history.clear();
        self.redo.clear();
        self.search_state.undo.clear();
        self.search_state.redo.clear();
        self.history_state.current = self.history_state.fresh();
        self.history_state.draft_guard = None;
    }

    pub(super) fn retain_reachable_draft_history(&mut self) {
        let mut nodes = BTreeSet::from([self.history_state.current]);
        for edge in self.history.iter().chain(&self.redo) {
            nodes.extend([edge.before, edge.after]);
        }
        self.search_state
            .undo
            .retain(|entry| nodes.contains(&entry.node));
        self.search_state
            .redo
            .retain(|entry| nodes.contains(&entry.node));
    }

    /// Draft-only operations share the node's core restore guard, never its content hash.
    pub(super) fn draft_history_guard(&mut self) -> Arc<Project> {
        let node = self.history_state.current;
        if let Some((id, guard)) = &self.history_state.draft_guard {
            if *id == node {
                return Arc::clone(guard);
            }
        }
        let guard = self
            .search_state
            .undo
            .iter()
            .chain(&self.search_state.redo)
            .find(|entry| entry.node == node)
            .map(|entry| Arc::clone(&entry.guard))
            .unwrap_or_else(|| Arc::new(self.project.clone()));
        self.history_state.draft_guard = Some((node, Arc::clone(&guard)));
        guard
    }

    /// Validate everything on private candidates before consuming an edge or any input.
    pub(super) fn restore_history_step(&mut self, forward: bool) -> Result<bool, String> {
        let stack = if forward { &self.redo } else { &self.history };
        let Some(edge) = stack.last() else {
            return Ok(false);
        };
        let expected = if forward { edge.before } else { edge.after };
        let target = if forward { edge.after } else { edge.before };
        if self.history_state.current != expected {
            return Err("撤销操作已不属于当前工程历史，未改变当前工程".into());
        }
        let mut candidate = self.project.clone();
        if !candidate.restore(edge.snapshot.clone()) {
            return Err("撤销快照已因外部刷新失效，未改变当前工程".into());
        }
        let current = self.manuscript.writing_buffers();
        let preserved: Vec<_> = current
            .iter()
            .filter_map(|buffer| {
                let mut checked = buffer.clone();
                checked
                    .rebase_unchanged_source(&candidate)
                    .ok()
                    .map(|()| checked)
            })
            .collect();
        let mut restored = Vec::new();
        if let Some(edit) = &edge.writing {
            let unsafe_draft = current
                .iter()
                .filter(|buffer| {
                    buffer.is_changed() && edit.paths.contains(&buffer.path().to_owned())
                })
                .any(|buffer| {
                    !forward
                        || !edit.buffers.iter().any(|original| {
                            original.path() == buffer.path()
                                && original.source() == buffer.source()
                                && original.generation() == buffer.generation()
                        })
                });
            if unsafe_draft {
                return Err("事务后又有未应用输入，撤销或重做不能覆盖草稿".into());
            }
            for buffer in &edit.buffers {
                let mut checked = buffer.clone();
                checked
                    .rebase_unchanged_source(if forward { &self.project } else { &candidate })
                    .map_err(|_| "关联草稿的原文已变化，未改变工程或消费撤销记录".to_owned())?;
                restored.push(checked);
            }
        }
        // The original project moves to the opposite stack; no extra full current clone.
        let previous = std::mem::replace(&mut self.project, candidate);
        let mut edge = if forward {
            self.redo.pop()
        } else {
            self.history.pop()
        }
        .unwrap();
        edge.snapshot = previous;
        self.history_state.current = target;
        self.history_state.draft_guard = None;
        // Discard obsolete clean views before restoring full verified working buffers.
        self.manuscript.rebase_clean(&self.project);
        self.manuscript.restore_writing_buffers(&preserved);
        if let Some(edit) = &edge.writing {
            if forward {
                self.manuscript.clear_applied_writing_buffers(&edit.paths);
            } else {
                self.manuscript.restore_writing_buffers(&restored);
            }
            self.search_state.clear_applied_operation();
        }
        if forward {
            self.history.push(edge);
        } else {
            self.redo.push(edge);
        }
        self.manuscript.rebase_unchanged_manuscripts(&self.project);
        Ok(true)
    }
}
