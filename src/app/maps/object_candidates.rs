use worldline_core::catalog::{Catalog, CatalogObject};
use worldline_core::object_search::{
    ObjectSearchOptions, ObjectSearchPage, MAX_OBJECT_SEARCH_CANDIDATES,
};

#[derive(Clone, Default)]
struct CandidateSearch {
    query: String,
    revision: u64,
    options: ObjectSearchOptions,
    loaded_options: Option<ObjectSearchOptions>,
    page: Option<Result<ObjectSearchPage, String>>,
}

impl CandidateSearch {
    fn refresh(&mut self, catalog: &Catalog, query: &str, revision: u64) {
        if self.query != query || self.revision != revision {
            self.query = query.into();
            self.revision = revision;
            self.options.offset = 0;
            self.page = None;
        }
        if self.page.is_none() || self.loaded_options != Some(self.options) {
            self.page = Some(
                catalog
                    .search_objects_page(query, self.options)
                    .map_err(|error| error.to_string()),
            );
            self.loaded_options = Some(self.options);
        }
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        catalog: &Catalog,
        query: &str,
        revision: u64,
    ) -> Vec<CatalogObject> {
        ui.horizontal_wrapped(|ui| {
            ui.label("候选上限");
            if ui
                .add(
                    egui::DragValue::new(&mut self.options.max_candidates)
                        .range(1..=MAX_OBJECT_SEARCH_CANDIDATES)
                        .speed(100),
                )
                .changed()
            {
                self.options.offset = 0;
            }
        });
        self.refresh(catalog, query, revision);
        if let Some(Ok(page)) = &self.page {
            let previous = page.offset.saturating_sub(page.limit);
            let next = page.next_offset;
            ui.horizontal_wrapped(|ui| {
                if crate::theme::add_enabled(ui, page.offset > 0, egui::Button::new("上一页候选"))
                    .clicked()
                {
                    self.options.offset = previous;
                }
                if crate::theme::add_enabled(ui, next.is_some(), egui::Button::new("下一页候选"))
                    .clicked()
                {
                    self.options.offset = next.unwrap_or_default();
                }
            });
        }
        self.refresh(catalog, query, revision);
        match &self.page {
            Some(Ok(page)) => {
                if page.total == 0 {
                    ui.label("0 个匹配对象；没有匹配对象。");
                } else {
                    ui.label(format!(
                        "{} 个匹配对象 · 显示 {}–{}{}",
                        page.total,
                        page.offset + 1,
                        page.offset + page.items.len(),
                        if page.truncated {
                            " · 分页显示"
                        } else {
                            ""
                        }
                    ));
                }
                if page.truncated {
                    ui.label(crate::theme::muted(
                        "可继续翻页，或输入更完整的名称、ID、类型或别名收窄结果。",
                    ));
                }
                page.items.clone()
            }
            Some(Err(error)) => {
                ui.colored_label(crate::theme::ERROR(), error);
                Vec::new()
            }
            None => Vec::new(),
        }
    }
}

impl super::super::WorldeditApp {
    pub(super) fn map_object_candidates(
        &self,
        ui: &mut egui::Ui,
        purpose: &str,
        query: &str,
    ) -> Vec<CatalogObject> {
        let Some(snapshot) = &self.snapshot else {
            ui.label("资料目录尚未就绪。");
            return Vec::new();
        };
        if snapshot.result.has_errors() {
            ui.colored_label(
                crate::theme::GOLD(),
                "源码存在错误，以下总数仅来自当前可解析目录；请在问题面板检查诊断。",
            );
        }
        // 浏览页只存入临时 UI 状态；工程、用途和地图隔离，重编译后回到首页。
        let id = ui.make_persistent_id((
            "map-object-candidates",
            &self.project.root,
            &self.map_selection,
            purpose,
        ));
        let mut search = ui
            .data_mut(|data| data.get_temp::<CandidateSearch>(id))
            .unwrap_or_default();
        let limit = if purpose == "reverse-lookup" { 20 } else { 8 };
        if search.options.limit != limit {
            search.options.limit = limit;
            search.options.offset = 0;
            search.page = None;
        }
        if query.trim().is_empty() {
            search.query = query.into();
            search.options.offset = 0;
            search.page = None;
            ui.data_mut(|data| data.insert_temp(id, search));
            ui.label(crate::theme::muted("输入名称、ID、类型或别名开始查找。"));
            return Vec::new();
        }
        let items = ui
            .push_id(id, |ui| {
                search.render(ui, &snapshot.result.analysis.catalog, query, self.version)
            })
            .inner;
        ui.data_mut(|data| data.insert_temp(id, search));
        items
    }
}

#[cfg(test)]
#[path = "tests/object_candidates.rs"]
mod tests;
