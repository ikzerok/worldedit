impl super::super::WorldeditApp {
    pub(super) fn map_overview_panel(
        &mut self,
        ui: &mut egui::Ui,
        map_summaries: &[(String, String)],
        back_requested: &mut bool,
    ) {
        ui.heading("地图浏览");
        if ui.button("展示预设…").clicked() {
            self.open_preset_editor(None);
        }
        ui.label(crate::theme::muted("地图选择、图层和标记信息"));
        if let Some(map_navigation) = self.map_navigation.as_ref() {
            let breadcrumbs = map_navigation.breadcrumbs();
            if map_navigation.history_len() > 0 {
                ui.separator();
                ui.label(egui::RichText::new("访问路径").strong());
                ui.horizontal_wrapped(|ui| {
                    for (index, breadcrumb) in breadcrumbs.iter().enumerate() {
                        if index > 0 {
                            ui.label(crate::theme::muted("›"));
                        }
                        if index + 1 == breadcrumbs.len() {
                            ui.label(egui::RichText::new(&breadcrumb.title).strong());
                        } else {
                            ui.label(&breadcrumb.title);
                        }
                    }
                });
                if ui.small_button("返回上一级").clicked() {
                    *back_requested = true;
                }
            }
        }
        ui.separator();
        ui.label(egui::RichText::new("已注册地图").strong());
        if map_summaries.is_empty() {
            ui.label(crate::theme::muted("当前工程没有注册地图。"));
        } else {
            egui::ScrollArea::vertical()
                .id_salt("map-list")
                .max_height(150.0)
                .show(ui, |ui| {
                    for (id, title) in map_summaries {
                        let label = format!("{}  ·  {}", title, id);
                        if ui
                            .add(egui::Button::selectable(
                                self.map_selection.as_deref() == Some(id.as_str()),
                                label,
                            ))
                            .clicked()
                            && !self.map_navigation_blocked()
                        {
                            self.map_selection = Some(id.clone());
                        }
                    }
                });
        }

        self.map_creation_panel(ui);
    }
}
