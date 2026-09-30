//! 资料索引：空间不足时使用可关闭抽屉，主编辑区保留可读宽度。
use super::{kind_label, WorldeditApp};
use egui::RichText;
use worldline_core::catalog::Catalog;
impl WorldeditApp {
    pub(super) fn catalog_index_ui(&mut self, ui: &mut egui::Ui, catalog: &Catalog) -> bool {
        let mut selected_object = false;
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
        egui::ScrollArea::vertical()
            .id_salt("catalog-items")
            .show(ui, |ui| {
                for object in &catalog.search_objects(&self.catalog_query) {
                    if !self.catalog_filter.is_empty() && object.target.kind != self.catalog_filter
                    {
                        continue;
                    }
                    let selected = self.catalog_target.as_ref() == Some(&object.target);
                    if ui
                        .add_sized(
                            [ui.available_width(), 48.0],
                            egui::Button::selectable(
                                selected,
                                format!(
                                    "{}\n{} · {}",
                                    object.display,
                                    kind_label(&object.target.kind),
                                    object.target.id
                                ),
                            ),
                        )
                        .on_hover_text(format!(
                            "{} · {}",
                            kind_label(&object.target.kind),
                            object.target.id
                        ))
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
