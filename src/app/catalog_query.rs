//! 目录组合查询和待办视图；筛选、分页、解释与待办语义仅由 worldline-core 提供。
mod actions;
mod filters;
mod persistence;
mod results;
mod view;

use super::WorldeditApp;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use worldline_core::catalog::TargetRef;
use worldline_core::queries::{
    CatalogQuery, CatalogQueryOptions, CatalogQueryPage, RelationDirection, SavedQueryDraft,
    TodoProjection, DEFAULT_CATALOG_QUERY_CANDIDATES, MAX_CATALOG_QUERY_CANDIDATES,
    MAX_CATALOG_QUERY_PAGE_SIZE,
};

const FAVORITES_STORAGE_KEY: &str = "worldedit.catalog.local_favorites.v1";

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum View {
    #[default]
    Query,
    Todos,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum ScalarInputKind {
    #[default]
    String,
    Number,
    Boolean,
}

#[derive(Default)]
struct FilterInputs {
    name_value: String,
    tag_value: String,
    property_key: String,
    property_value: String,
    property_kind: ScalarInputKind,
    property_bool: bool,
    relation_type: String,
    relation_related: String,
    relation_direction: RelationDirection,
    scope_file: String,
    missing_property: String,
    missing_relation: String,
}

pub(super) struct WorkbenchState {
    pub(super) open: bool,
    view: View,
    query: CatalogQuery,
    page_size: usize,
    max_candidates: usize,
    page: Option<CatalogQueryPage>,
    saved_query_id: String,
    saved_query_name: String,
    error: Option<String>,
    inputs: FilterInputs,
    local_favorites: BTreeMap<PathBuf, BTreeSet<String>>,
    todo_cache: Option<TodoProjection>,
    #[cfg(not(target_arch = "wasm32"))]
    running: Option<RunningQuery>,
}

impl Default for WorkbenchState {
    fn default() -> Self {
        Self {
            open: false,
            view: View::Query,
            query: CatalogQuery::default(),
            page_size: 50,
            max_candidates: DEFAULT_CATALOG_QUERY_CANDIDATES,
            page: None,
            saved_query_id: String::new(),
            saved_query_name: String::new(),
            error: None,
            inputs: FilterInputs::default(),
            local_favorites: BTreeMap::new(),
            todo_cache: None,
            #[cfg(not(target_arch = "wasm32"))]
            running: None,
        }
    }
}

fn current_options(state: &WorkbenchState) -> CatalogQueryOptions {
    CatalogQueryOptions {
        offset: 0,
        page_size: state.page_size.clamp(1, MAX_CATALOG_QUERY_PAGE_SIZE),
        max_candidates: state.max_candidates.clamp(1, MAX_CATALOG_QUERY_CANDIDATES),
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct RunningQuery {
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    receiver: std::sync::mpsc::Receiver<(String, Result<CatalogQueryPage, String>)>,
    query: CatalogQuery,
    options: CatalogQueryOptions,
    cancel_requested: bool,
}

impl WorldeditApp {
    pub(super) fn catalog_query_tab(&mut self, ctx: &egui::Context) {
        let mut workbench = std::mem::take(&mut self.catalog_workbench);
        workbench.render(self, ctx);
        self.catalog_workbench = workbench;
    }
}

enum Action {
    None,
    Run,
    Next,
    Previous(usize),
    Save,
    Load(SavedQueryDraft),
    Favorite(String),
    #[cfg(not(target_arch = "wasm32"))]
    Cancel,
    Navigate(TargetRef),
    Jump(String, u32, u32),
}

#[cfg(test)]
mod persistence_tests {
    use super::*;

    #[derive(Default)]
    struct MemoryStorage(BTreeMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.into(), value);
        }

        fn flush(&mut self) {}
    }

    #[test]
    fn local_favorites_survive_reopen_without_entering_project_documents() {
        let mut state = WorkbenchState::default();
        let root = PathBuf::from("sample-project");
        state.toggle_favorite(&root, "people".into());
        let mut storage = MemoryStorage::default();
        state.save_favorites(&mut storage);

        let mut reopened = WorkbenchState::default();
        reopened.restore_favorites(Some(&storage));
        assert!(reopened.local_favorites[&root].contains("people"));
        reopened.reset_for_workspace();
        assert!(reopened.local_favorites[&root].contains("people"));
    }
}
