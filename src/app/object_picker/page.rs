use worldline_core::catalog::Catalog;
use worldline_core::object_search::{ObjectSearchFilter, ObjectSearchOptions, ObjectSearchPage};

#[derive(Clone, Default)]
pub(in crate::app) struct CandidatePage {
    pub query: String,
    pub serial: u64,
    filter: ObjectSearchFilter,
    revision: String,
    pub options: ObjectSearchOptions,
    loaded_options: Option<ObjectSearchOptions>,
    pub result: Option<Result<ObjectSearchPage, String>>,
}
impl CandidatePage {
    /// 返回是否因来源变化丢弃旧候选；查询/过滤变化也回首页但不修改作者引用。
    pub fn refresh(
        &mut self,
        catalog: &Catalog,
        query: &str,
        filter: &ObjectSearchFilter,
        revision: &str,
    ) -> bool {
        let stale = !self.revision.is_empty() && self.revision != revision;
        if self.query != query || self.filter != *filter || self.revision != revision {
            self.serial = self.serial.wrapping_add(1);
            self.query = query.into();
            self.filter = filter.clone();
            self.revision = revision.into();
            self.options.offset = 0;
            self.result = None;
        }
        if self.result.is_none() || self.loaded_options != Some(self.options) {
            if self.loaded_options != Some(self.options) {
                self.serial = self.serial.wrapping_add(1);
            }
            self.result = Some(
                catalog
                    .search_objects_filtered_page(query, filter, self.options)
                    .map_err(|error| error.to_string()),
            );
            self.loaded_options = Some(self.options);
        }
        stale
    }
    pub fn turn(&mut self, previous: bool) -> bool {
        let Some(Ok(page)) = &self.result else {
            return false;
        };
        let offset = if previous {
            page.offset
                .checked_sub(page.limit)
                .or((page.offset > 0).then_some(0))
        } else {
            page.next_offset
        };
        if let Some(offset) = offset {
            self.options.offset = offset;
            true
        } else {
            false
        }
    }
    pub fn controls(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        match self.result.clone() {
            Some(Ok(page)) => {
                ui.horizontal_wrapped(|ui| {
                    ui.label(if page.items.is_empty() {
                        format!("{} 个匹配对象 · 本页为空", page.total)
                    } else {
                        format!(
                            "{} 个匹配对象 · {}–{}",
                            page.total,
                            page.offset + 1,
                            page.offset + page.items.len()
                        )
                    });
                    if crate::theme::add_enabled(ui, page.offset > 0, egui::Button::new("上一页"))
                        .clicked()
                    {
                        changed |= self.turn(true);
                    }
                    if crate::theme::add_enabled(
                        ui,
                        page.next_offset.is_some(),
                        egui::Button::new("下一页"),
                    )
                    .clicked()
                    {
                        changed |= self.turn(false);
                    }
                });
            }
            Some(Err(error)) => {
                ui.colored_label(crate::theme::ERROR(), error);
            }
            None => {
                ui.label("对象目录尚未就绪");
            }
        }
        changed
    }
}
