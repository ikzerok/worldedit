use super::*;
use std::path::Path;
impl WorkbenchState {
    pub(super) fn load_saved_query(
        &mut self,
        project: &worldline_core::project::Project,
        draft: SavedQueryDraft,
    ) {
        let index = project.saved_query_index();
        let Some(document) = index.queries.get(&draft.id) else {
            self.error = Some("查询定义不可用或格式不受支持，保留原文。".into());
            return;
        };
        if document.read_only || document.draft != draft {
            self.error = Some("查询定义只读或已经变化，请重新检查原文。".into());
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(running) = &mut self.running {
            running
                .cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
            running.cancel_requested = true;
        }
        // 载入不选择新版本；只有显式编辑条件才调用 core 的版本同步。
        self.query = draft.query;
        self.saved_query_id = draft.id;
        self.saved_query_name = draft.name;
        self.saved_query_baseline = Some((self.saved_query_id.clone(), project.content_baseline()));
        self.inputs = FilterInputs::default();
        self.page = None;
        self.error = None;
    }

    pub(in crate::app) fn unapplied_saved_query(
        &self,
        project: &worldline_core::project::Project,
    ) -> Option<String> {
        if self.saved_query_id.trim().is_empty() && self.saved_query_name.trim().is_empty() {
            return None;
        }
        let draft = SavedQueryDraft {
            id: self.saved_query_id.clone(),
            name: self.saved_query_name.clone(),
            query: self.query.clone(),
        };
        let index = project.saved_query_index();
        index
            .queries
            .get(&self.saved_query_id)
            .is_none_or(|saved| saved.draft != draft)
            .then(|| format!("saved_query:{}", self.saved_query_id))
    }

    pub(in crate::app) fn restore_favorites(&mut self, storage: Option<&dyn eframe::Storage>) {
        let Some(saved) = storage.and_then(|storage| storage.get_string(FAVORITES_STORAGE_KEY))
        else {
            return;
        };
        if let Ok(favorites) = serde_json::from_str::<BTreeMap<PathBuf, BTreeSet<String>>>(&saved) {
            self.local_favorites = favorites;
        }
    }

    pub(in crate::app) fn save_favorites(&self, storage: &mut dyn eframe::Storage) {
        if let Ok(saved) = serde_json::to_string(&self.local_favorites) {
            storage.set_string(FAVORITES_STORAGE_KEY, saved);
        }
    }

    pub(in crate::app) fn reset_for_workspace(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(running) = &self.running {
            running
                .cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let favorites = std::mem::take(&mut self.local_favorites);
        let columns = std::mem::take(&mut self.personal_columns);
        *self = Self::default();
        self.local_favorites = favorites;
        self.personal_columns = columns;
    }
    pub(super) fn save_query(&mut self, app: &mut WorldeditApp) {
        let draft = SavedQueryDraft {
            id: self.saved_query_id.clone(),
            name: self.saved_query_name.clone(),
            query: self.query.clone(),
        };
        let baseline = self
            .saved_query_baseline
            .as_ref()
            .filter(|(id, _)| id == &self.saved_query_id)
            .map(|(_, baseline)| baseline.clone())
            .unwrap_or_else(|| app.project.content_baseline());
        if !app.commit("共享查询定义已保存", move |project| {
            project.save_saved_query(draft, &baseline).map(|_| ())
        }) {
            self.error = app.io_error.clone();
        } else {
            self.error = None;
            self.page = None;
            self.todo_cache = None;
            self.saved_query_baseline =
                Some((self.saved_query_id.clone(), app.project.content_baseline()));
        }
    }

    pub(super) fn toggle_favorite(&mut self, root: &Path, id: String) {
        let favorites = self.local_favorites.entry(root.to_path_buf()).or_default();
        if !favorites.remove(&id) {
            favorites.insert(id);
        }
    }
}
