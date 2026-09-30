//! 固定参考分配侧栏空间；窄窗保留单份标签而非覆盖主正文。
use crate::app::WorldeditApp;
impl WorldeditApp {
    pub(in crate::app) fn docked_reading(&mut self, ctx: &egui::Context) {
        if !self.personal.settings.references_visible || !self.personal.settings.dock_references {
            return;
        }
        let ids = self.reading_panels.ids();
        if ids.is_empty() {
            return;
        }
        if !ids.contains(&self.selected_reading_panel.unwrap_or(u64::MAX)) {
            self.selected_reading_panel = ids.first().copied();
        }
        let dual = ids.len() == 2 && ctx.available_rect().width() >= 1450.0;
        let width = if dual {
            self.personal.settings.reference_width * 2.0
        } else {
            self.personal.settings.reference_width
        };
        let output = egui::SidePanel::right("docked-reading")
            .resizable(true)
            .default_width(width)
            .width_range(if dual { 520.0..=880.0 } else { 260.0..=440.0 })
            .show(ctx, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.strong("固定参考");
                    if ui.small_button("浮动").clicked() {
                        self.personal.settings.dock_references = false;
                    }
                    if ui.small_button("收起").clicked() {
                        self.personal.settings.references_visible = false;
                    }
                    if !dual {
                        for (i, id) in ids.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.selected_reading_panel,
                                Some(*id),
                                format!("参考 {}", i + 1),
                            );
                        }
                    }
                });
                ui.separator();
                if dual {
                    ui.columns(2, |columns| {
                        for (index, id) in ids.iter().enumerate() {
                            self.docked_reading_content(&mut columns[index], *id);
                        }
                    });
                } else if let Some(id) = self.selected_reading_panel {
                    self.docked_reading_content(ui, id);
                }
            });
        self.personal.settings.reference_width = if dual {
            output.response.rect.width() / 2.0
        } else {
            output.response.rect.width()
        };
    }
    fn docked_reading_content(&mut self, ui: &mut egui::Ui, id: u64) {
        let Some(panel) = self.reading_panels.get(id) else {
            return;
        };
        let target = panel.target.clone();
        let can_back = !panel.history.is_empty();
        self.active_reading_panel = Some(id);
        ui.push_id(("dock-reference", id), |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(can_back, egui::Button::new("← 上一条"))
                    .clicked()
                {
                    self.reading_panels.back(id);
                }
                if ui.button("关闭参考").clicked() {
                    self.reading_panels.close(id);
                }
            });
            egui::ScrollArea::vertical()
                .id_salt(("reference-body", id))
                .show(ui, |ui| {
                    ui.set_max_width(
                        self.personal
                            .settings
                            .reading_width
                            .min(ui.available_width()),
                    );
                    ui.spacing_mut().item_spacing.y = (self.personal.settings.body_size
                        * (self.personal.settings.line_spacing - 1.0))
                        .max(4.0);
                    self.reading_content(ui, target);
                });
        });
        self.active_reading_panel = None;
    }
}
