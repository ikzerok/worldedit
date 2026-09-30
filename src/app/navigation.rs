//! 仅管理个人导航；选择页面不写入 Project，也不建立内容关系。
use super::{Tab, WorldeditApp};
use crate::theme::{self, *};
use egui::{Align2, FontId, Response, Stroke, Vec2};

pub(super) const GROUPS: &[(&str, &[Tab])] = &[
    (
        "创作",
        &[Tab::Overview, Tab::Timeline, Tab::Graph, Tab::Play],
    ),
    (
        "世界资料",
        &[Tab::Characters, Tab::World, Tab::Catalog, Tab::Wiki],
    ),
    (
        "探索与协作",
        &[Tab::Map, Tab::Network, Tab::Review, Tab::Edit],
    ),
];

pub(super) fn row(ui: &mut egui::Ui, tab: Tab, selected: bool) -> Response {
    let response = ui.add_sized(
        [ui.available_width(), 32.0],
        egui::Button::new("")
            .fill(if selected {
                SELECTED
            } else {
                egui::Color32::TRANSPARENT
            })
            .stroke(Stroke::NONE),
    );
    let rect = response.rect;
    if !selected && response.hovered() {
        ui.painter().rect_filled(rect, 6, HOVER);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            6,
            Stroke::new(1.0_f32, ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    if selected {
        let mark = egui::Rect::from_center_size(
            rect.left_center() + Vec2::new(3.0, 0.0),
            Vec2::new(2.0, 16.0),
        );
        ui.painter().rect_filled(mark, 1, ACCENT);
    }
    super::workspace::nav_icon(
        ui.painter(),
        rect.left_center() + Vec2::new(20.0, 0.0),
        tab,
        if selected { ACCENT } else { MUTED },
    );
    ui.painter().text(
        rect.left_center() + Vec2::new(40.0, 0.0),
        Align2::LEFT_CENTER,
        tab.title(),
        FontId::proportional(14.0),
        if selected { ACCENT } else { TEXT },
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tab.title()));
    response.on_hover_text(tab.title())
}

impl WorldeditApp {
    pub(super) fn navigation_groups(&mut self, ui: &mut egui::Ui) {
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            for (index, (title, tabs)) in GROUPS.iter().enumerate() {
                if index > 0 {
                    ui.add_space(12.0);
                }
                ui.label(theme::muted(*title));
                ui.add_space(2.0);
                for tab in *tabs {
                    if row(ui, *tab, self.tab == *tab).clicked() {
                        self.tab = *tab;
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_existing_view_has_one_navigation_entry() {
        let tabs: Vec<_> = GROUPS.iter().flat_map(|(_, tabs)| tabs.iter()).collect();
        assert_eq!(tabs.len(), 12);
        for tab in &tabs {
            assert_eq!(tabs.iter().filter(|item| item == &tab).count(), 1);
        }
    }
}
