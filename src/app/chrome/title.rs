use super::*;
use crate::chrome::action_layout::{button_width, record, text_width};

impl WorldeditApp {
    pub(super) fn title_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top")
            .frame(crate::chrome::title_frame(ctx))
            .show(ctx, |ui| {
                // 背景先登记，后画按钮；保留真实空白拖动区，不覆盖后续交互。
                crate::chrome::title_drag(ui);
                ui.horizontal(|ui| {
                    if ui.horizontal(crate::chrome::controls).inner {
                        self.request_action(Pending::Close, ctx);
                    }
                    self.title_identity(ui);
                });
            });
    }

    fn title_identity(&mut self, ui: &mut egui::Ui) {
        let gap = ui.spacing().item_spacing.x;
        let long = "查找对象 / 命令    Ctrl/Cmd+P";
        let search = if ui.available_width() > button_width(ui, long) + 220.0 {
            long
        } else if ui.available_width() > button_width(ui, "查找对象") + 100.0 {
            "查找对象"
        } else {
            "命令"
        };
        let search_width = button_width(ui, search);
        let brand = text_width(ui, "worldedit", egui::TextStyle::Small);
        let show_brand = ui.available_width() > search_width + brand + 140.0 + gap * 3.0;
        // push_id alone does not stabilize egui automatic IDs after conditional siblings.
        // Keep the brand slot itself present at every width, including its empty state.
        ui.push_id("global-brand-slot", |ui| {
            if show_brand {
                ui.label(egui::RichText::new("worldedit").size(13.0).color(MUTED()));
                ui.add_space(10.0);
            }
        });
        let builder = egui::UiBuilder::new()
            .id_salt("global-command-region")
            .layout(egui::Layout::right_to_left(egui::Align::Center));
        ui.scope_builder(builder, |ui| {
            let response = ui
                .push_id("global-command", |ui| {
                    ui.add_sized(
                        [search_width, theme::metrics().control_height],
                        egui::Button::new(search),
                    )
                    .on_hover_text(
                        "按名称、类型、别名或来源寻找对象；Ctrl/Cmd+P，命令可在面板内切换",
                    )
                })
                .inner;
            record(ui, "global/commands", &response);
            if response.clicked() {
                self.open_commands(ui.ctx(), false);
            }
            let title = self
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.result.analysis.world.as_ref())
                .map(|world| world.display.clone())
                .or_else(|| {
                    self.project
                        .root
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| "未命名作品".into());
            ui.add_sized(
                [
                    ui.available_width().max(0.0),
                    theme::metrics().control_height,
                ],
                egui::Label::new(egui::RichText::new(&title).strong().size(16.0)).truncate(),
            )
            .on_hover_text(format!("{}\n{}", title, self.project.root.display()));
        });
    }
}
