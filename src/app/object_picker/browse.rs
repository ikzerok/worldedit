use super::*;

impl crate::app::WorldeditApp {
    /// 已应用目录的浏览/插链页；调用方仍负责明确选中后的既有表单事务。
    pub(in crate::app) fn applied_object_candidates(
        &self,
        ui: &mut Ui,
        catalog: &Catalog,
        purpose: &str,
        query: &str,
        allowed: &[&str],
    ) -> Vec<CatalogObject> {
        let id = ui.make_persistent_id(("applied-object-page", &self.project.root, purpose));
        let mut state = ui
            .data_mut(|data| data.get_temp::<CandidatePage>(id))
            .unwrap_or_default();
        let revision = format!("{:?}:{}", self.project.root, self.version);
        let filter = filter(allowed, None);
        let stale = state.refresh(catalog, query, &filter, &revision);
        ui.small("已应用目录");
        if self
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.result.has_errors())
        {
            ui.colored_label(crate::theme::GOLD(), "源码含错误，以下总数仅代表已解析目录");
        }
        if state.controls(ui) {
            state.refresh(catalog, query, &filter, &revision);
        }
        let items = state
            .result
            .as_ref()
            .and_then(|page| page.as_ref().ok())
            .map(|page| page.items.clone())
            .unwrap_or_default();
        ui.data_mut(|data| data.insert_temp(id, state));
        if stale {
            ui.small("目录已变化，请重新选择");
            Vec::new()
        } else {
            items
        }
    }
}
