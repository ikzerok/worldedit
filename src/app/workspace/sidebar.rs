//! 全局工作领域与工程文件：平面导航，不复制领域内部的对象树。
mod drawer;
mod files;
mod icons;
mod rows;

use super::super::{Tab, WorldeditApp};
use crate::theme;
use egui::{RichText, Stroke};

const GROUPS: &[(&str, &[Tab])] = &[
    ("写作与阅读", &[Tab::Manuscript, Tab::Overview, Tab::Edit]),
    (
        "世界资料",
        &[
            Tab::World,
            Tab::Characters,
            Tab::Catalog,
            Tab::CatalogImport,
            Tab::Wiki,
            Tab::Map,
            Tab::Network,
        ],
    ),
    (
        "结构与审阅",
        &[Tab::Timeline, Tab::Graph, Tab::Review, Tab::Play],
    ),
    (
        "工程工具",
        &[Tab::Templates, Tab::Localization, Tab::CheckpointHistory],
    ),
];

impl WorldeditApp {
    pub(in crate::app) fn sidebar(&mut self, ctx: &egui::Context) {
        let technical = theme::style_preset() == theme::StylePreset::Technical;
        let frame =
            egui::Frame::new()
                .fill(theme::NAVIGATION())
                .inner_margin(egui::Margin::symmetric(
                    if technical { 8 } else { 12 },
                    if technical { 8 } else { 14 },
                ));
        let panel = egui::SidePanel::left("project")
            .resizable(true)
            .default_width(self.personal.settings.navigation_width)
            .width_range(180.0..=320.0)
            .frame(frame)
            .show(ctx, |ui| self.sidebar_content(ui));
        self.personal.settings.navigation_width = panel.response.rect.width();
    }

    fn sidebar_content(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let metrics = theme::metrics();
        let technical = theme::style_preset() == theme::StylePreset::Technical;
        let title = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.result.analysis.world.as_ref())
            .map(|world| world.display.as_str())
            .unwrap_or("我的作品");
        if technical {
            egui::Frame::new()
                .fill(theme::CHROME())
                .inner_margin(6)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("工作区导航").strong().size(13.0));
                });
        } else {
            ui.label(theme::muted("工作区"));
        }
        ui.add(egui::Label::new(RichText::new(title).strong().size(16.0)).truncate())
            .on_hover_text(title);
        ui.label(theme::muted(format!(
            "{} 个源码文件",
            self.project.documents.len()
        )));
        ui.add_space(metrics.section_gap * 0.6);
        let previous =
            ctx.data(|data| data.get_temp::<Tab>(egui::Id::new("workbench-navigation-tab")));
        let entering = previous != Some(self.tab);
        let current_tab = self.tab;
        let navigation_height = (ui.available_height() - 148.0).max(120.0);
        egui::ScrollArea::vertical()
            .id_salt("sidebar-tabs")
            .max_height(navigation_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (index, (group, tabs)) in GROUPS.iter().enumerate() {
                    let id = ui.make_persistent_id(("workbench-nav-group", group));
                    if entering && tabs.contains(&current_tab) {
                        let mut state =
                            egui::collapsing_header::CollapsingState::load_with_default_open(
                                ui.ctx(),
                                id,
                                true,
                            );
                        state.set_open(true);
                        state.store(ui.ctx());
                    }
                    let response = egui::CollapsingHeader::new(
                        RichText::new(*group).size(13.0).color(theme::MUTED()),
                    )
                    .id_salt(("workbench-nav-group", group))
                    .default_open(index == 0 || tabs.contains(&current_tab))
                    .show_background(technical)
                    .show_unindented(ui, |ui| {
                        for &tab in *tabs {
                            let selected = self.tab == tab;
                            let response = rows::navigation_row(ui, tab, selected);
                            if response.clicked() {
                                self.switch_tab(tab);
                            }
                            if entering && selected {
                                response.scroll_to_me(None);
                            }
                        }
                    });
                    if technical {
                        ui.painter().hline(
                            response.header_response.rect.x_range(),
                            response.header_response.rect.bottom(),
                            Stroke::new(1.0_f32, theme::BORDER()),
                        );
                    }
                    ui.add_space(if technical { 3.0 } else { 7.0 });
                }
            });
        ctx.data_mut(|data| {
            data.insert_temp(egui::Id::new("workbench-navigation-tab"), current_tab);
        });
        ui.add_space(metrics.section_gap * 0.5);
        ui.separator();
        self.sidebar_files(ui);
    }
}
