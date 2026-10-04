//! 当前稿感知查找、受控替换及字节位置到编辑选区的统一入口。
mod navigation;
mod objects;
mod selection;
mod selection_origin;
mod transactions;
mod window;
use super::WorldeditApp;
pub(crate) use selection::{
    clear_pending_selection, editor_selection, mark_pending_selection_programmatic,
    record_editor_selection, record_navigation_focus, request_diagnostic_selection,
    request_selection, request_writing_selection, restore_editor_selection,
    restore_writing_selection, scroll_editor_selection, selection_is_representable,
};
pub(crate) use selection_origin::{
    observe_manual_selection, restore_origin, selection_is_diagnostic,
};
use std::{collections::BTreeSet, path::PathBuf};
use worldline_core::{manuscript::WritingBuffer, search_replace::*};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Scope {
    #[default]
    Current,
    Selection,
    Project,
}
#[derive(Default)]
pub(super) struct SearchState {
    scope: Scope,
    source: bool,
    options: SearchOptions,
    replacement: String,
    files: BTreeSet<PathBuf>,
    current: Option<selection::EditorSelection>,
    current_version: Option<(String, u64)>,
    previous_focus: Option<egui::Id>,
    selected: usize,
    located: Option<SearchMatch>,
    navigation_basis: Option<navigation::NavigationBasis>,
    plan: Option<ReplacePlan>,
    error: Option<String>,
    replace: bool,
    pub(super) undo: Vec<DraftUndo>,
    pub(super) redo: Vec<DraftUndo>,
    project_edits: Vec<ProjectEdit>,
}
pub(super) struct DraftUndo {
    before: WritingBuffer,
    after: WritingBuffer,
    depth: usize,
}
struct ProjectEdit {
    before: String,
    after: String,
    buffers: Vec<WritingBuffer>,
    paths: Vec<PathBuf>,
}
impl WorldeditApp {
    pub(in crate::app) fn play_search_signature(&self) -> String {
        let state = &self.search_state;
        format!(
            "{:?}",
            (
                &state.plan,
                &state.options,
                &state.replacement,
                &state.files
            )
        )
    }

    pub(in crate::app) fn unapplied_search_sources(&self) -> Vec<String> {
        self.search_state
            .plan
            .as_ref()
            .map(|plan| {
                plan.changes
                    .iter()
                    .map(|change| change.path.display().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(in crate::app) fn search_return_focus(&self) -> Option<egui::Id> {
        self.search_state.previous_focus
    }

    pub(super) fn open_search(&mut self, ctx: &egui::Context, project: bool, replace: bool) {
        self.sync_edit_layers(ctx);
        selection::clear_restored_focus(ctx);
        if !self.search_open {
            self.search_state.previous_focus = ctx.memory(|m| m.focused());
        }
        self.search_state.current = selection::editor_selection(ctx).filter(|selection| {
            if self.tab == super::Tab::Manuscript {
                self.manuscript.comment_selection_is_current_mode()
                    && self.manuscript_session().cursor.is_some()
                    && self
                        .manuscript
                        .active_writing_target()
                        .is_some_and(|(target, path)| {
                            path == selection.path && selection.target.as_ref() == Some(&target)
                        })
            } else {
                selection.path == self.active_file && selection.target.is_none()
            }
        });
        if self.search_state.current.is_none()
            || self
                .search_state
                .current
                .as_ref()
                .is_some_and(|s| self.project.document(&s.path).is_err())
        {
            let active = if self.tab == super::Tab::Manuscript {
                self.manuscript.active_writing_target()
            } else {
                None
            };
            let path = active
                .as_ref()
                .map(|(_, path)| path.clone())
                .unwrap_or(self.active_file.clone());
            let source = self
                .manuscript
                .writing_buffers()
                .into_iter()
                .find(|b| b.path() == path && b.is_changed())
                .map(|b| b.source().to_owned())
                .or_else(|| self.project.document(&path).ok().map(str::to_owned));
            if let Some(source) = source {
                self.search_state.current = Some(selection::EditorSelection {
                    path,
                    target: active.map(|(target, _)| target),
                    range: 0..0,
                    id: egui::Id::new("search-return"),
                    source,
                });
            }
        }
        self.search_state.current_version =
            self.search_state.current.as_ref().and_then(|current| {
                self.manuscript
                    .writing_buffers()
                    .iter()
                    .find(|buffer| buffer.path() == current.path)
                    .map(|buffer| (buffer.baseline().to_owned(), buffer.generation()))
            });
        if self.search_state.previous_focus.is_none() {
            self.search_state.previous_focus = self
                .search_state
                .current
                .as_ref()
                .map(|selection| selection.id);
        }
        self.search_state.scope = if project {
            Scope::Project
        } else {
            Scope::Current
        };
        self.search_state.files.clear();
        if let Some(current) = &self.search_state.current {
            self.search_state.files.insert(current.path.clone());
        }
        if project {
            self.search_state.files = self
                .project
                .documents
                .iter()
                .filter(|(_, d)| !d.is_deleted())
                .map(|(path, _)| path.clone())
                .collect();
        }
        self.search_state.replace = replace;
        self.search_state.plan = None;
        self.search_state.error = None;
        self.search_state.selected = 0;
        self.search_state.located = None;
        self.search_state.navigation_basis = None;
        self.search_open = true;
        self.search_focus = true;
    }
    fn search_request(&self) -> Result<SearchRequest, String> {
        let state = &self.search_state;
        let files = if state.scope == Scope::Project {
            state
                .files
                .iter()
                .map(|path| SearchFile {
                    path: path.clone(),
                    range: None,
                })
                .collect()
        } else {
            let current = state.current.as_ref().ok_or("请先打开正文或源码文稿")?;
            let range = if state.scope == Scope::Selection {
                if let Some((baseline, generation)) = &state.current_version {
                    let valid = self.manuscript.writing_buffers().iter().any(|buffer| {
                        buffer.path() == current.path
                            && buffer.baseline() == baseline
                            && buffer.generation() == *generation
                    });
                    if !valid {
                        return Err("选区对应草稿版本已变化，请重新选择".into());
                    }
                }
                let source = self
                    .manuscript
                    .writing_buffers()
                    .into_iter()
                    .find(|b| b.path() == current.path && b.is_changed())
                    .map(|b| b.source().to_owned())
                    .or_else(|| self.project.document(&current.path).ok().map(str::to_owned));
                if source.as_deref() != Some(current.source.as_str()) {
                    return Err("选区对应原稿已变化，请重新选择".into());
                }
                if current.range.is_empty() {
                    return Err("请先在编辑区选择文字，再打开查找".into());
                }
                Some(current.range.clone())
            } else if !state.source {
                if let Some(target) = &current.target {
                    let buffer = self
                        .manuscript
                        .writing_buffers()
                        .into_iter()
                        .find(|b| b.path() == current.path && b.is_changed())
                        .or_else(|| self.project.open_source_writing_buffer(&current.path).ok())
                        .ok_or("当前稿已关闭")?;
                    Some(self.project.project_writing_buffer(&buffer, target)?.range)
                } else {
                    None
                }
            } else {
                None
            };
            vec![SearchFile {
                path: current.path.clone(),
                range,
            }]
        };
        Ok(SearchRequest {
            query: self.project_query.clone(),
            replacement: state.replacement.clone(),
            options: state.options,
            scope: if state.source {
                SearchScope::Source
            } else {
                SearchScope::Prose
            },
            files,
        })
    }
    pub(in crate::app) fn current_search_hits(&self) -> Result<Vec<SearchMatch>, String> {
        self.project
            .search_drafts(&self.search_request()?, &self.manuscript.writing_buffers())
    }
    pub(in crate::app) fn refresh_search_return_focus(&mut self, ctx: &egui::Context) {
        if let Some(id) = selection::take_restored_focus(ctx) {
            self.search_state.previous_focus = Some(id);
            if let Some((_, focus)) = self
                .command_palette
                .focus_stack
                .iter_mut()
                .find(|(kind, _)| *kind == "search")
            {
                *focus = Some(id);
            }
        }
    }
    pub(super) fn close_search(&mut self, ctx: &egui::Context) {
        self.refresh_search_return_focus(ctx);
        self.search_open = false;
        self.search_state.plan = None;
        if let Some(id) = self.search_state.previous_focus {
            ctx.memory_mut(|m| m.request_focus(id));
        }
    }
}
#[cfg(test)]
mod tests;
