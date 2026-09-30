use super::*;
use std::path::Path;
impl WorkbenchState {
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
        let baseline = app.project.content_baseline();
        if !app.commit("共享查询定义已保存", move |project| {
            project.save_saved_query(draft, &baseline).map(|_| ())
        }) {
            self.error = app.io_error.clone();
        } else {
            self.error = None;
            self.page = None;
            self.todo_cache = None;
        }
    }

    pub(super) fn toggle_favorite(&mut self, root: &Path, id: String) {
        let favorites = self.local_favorites.entry(root.to_path_buf()).or_default();
        if !favorites.remove(&id) {
            favorites.insert(id);
        }
    }
}
