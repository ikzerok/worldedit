//! 资料索引：空间不足时使用可关闭抽屉，主编辑区保留可读宽度。
use super::WorldeditApp;
use egui::RichText;
use worldline_core::catalog::Catalog;
impl WorldeditApp {
    pub(super) fn catalog_index_ui(&mut self, ui: &mut egui::Ui, catalog: &Catalog) -> bool {
        let mut selected_object = false;
        if ui.button("持续资料约束").clicked() {
            self.open_schema_editor(ui.ctx());
        }
        ui.label(RichText::new("世界资料索引").strong().size(17.0));
        ui.horizontal_wrapped(|ui| {
            for (id, name) in [
                ("entity", "通用资料"),
                ("relation", "独立关系"),
                ("tag", "标签"),
                ("state", "状态"),
                ("anchor", "锚点"),
                ("asset", "素材"),
                ("", "全部"),
            ] {
                ui.selectable_value(&mut self.catalog_filter, id.into(), name);
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.catalog_query)
                .hint_text("名称、ID 或别名")
                .desired_width(f32::INFINITY),
        );
        ui.add_space(8.0);
        let allowed = if self.catalog_filter.is_empty() {
            Vec::new()
        } else {
            vec![self.catalog_filter.as_str()]
        };
        let objects = self.applied_object_candidates(
            ui,
            catalog,
            "catalog-index",
            &self.catalog_query,
            &allowed,
        );
        egui::ScrollArea::vertical()
            .id_salt("catalog-items")
            .show(ui, |ui| {
                for object in &objects {
                    let selected = self.catalog_target.as_ref() == Some(&object.target);
                    if super::super::object_picker::candidate_row_at_revision(
                        ui,
                        object,
                        Some(&self.project.root),
                        selected,
                        (self.version, &self.catalog_query, &self.catalog_filter),
                    )
                    .clicked()
                    {
                        if self.prevent_catalog_switch() {
                            continue;
                        }
                        self.catalog_target = Some(object.target.clone());
                        selected_object = true;
                        self.tag_editor = None;
                        self.state_editor = None;
                        self.anchor_editor = None;
                    }
                }
            });
        selected_object
    }
}
