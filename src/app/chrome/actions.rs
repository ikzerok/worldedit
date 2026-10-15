use super::*;
use crate::chrome::action_layout::record;
use layout::{ActionLayout, ChromeLayoutBudget};

impl WorldeditApp {
    pub(super) fn workbench_actions(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing = egui::vec2(7.0, 5.0);
        let navigation = self.focus_style()
            || !self.personal.settings.navigation
            || self.personal.settings.focus
            || self.compact_workspace_navigation(ui.ctx());
        let layout = ChromeLayoutBudget::measure(ui, navigation, "输入待应用 · 工程待保存")
            .choose(self.focus_style());
        ui.horizontal(|ui| {
            // A stable slot also preserves later automatic button IDs when navigation folds.
            ui.push_id("global-navigation-slot", |ui| {
                self.compact_navigation_menu(ui)
            });
            if layout == ActionLayout::Stacked {
                let height = ui.fonts(|fonts| {
                    fonts
                        .layout_no_wrap(
                            "输入待应用\n工程待保存".into(),
                            egui::TextStyle::Button.resolve(ui.style()),
                            theme::TEXT(),
                        )
                        .size()
                        .y
                }) + ui.spacing().button_padding.y * 2.0;
                // Changing draft/IME state must not move the source editor below this bar.
                ui.set_min_height(height.max(ui.spacing().interact_size.y));
            }
            let save = ui
                .push_id("global-save", |ui| {
                    ui.add(theme::primary("保存全部"))
                        .on_hover_text("Ctrl/Cmd+S · 保存工程中全部已应用修改；不自动应用草稿")
                })
                .inner;
            record(ui, "global/save", &save);
            if save.clicked() {
                self.save();
            }
            self.save_state_menu(ui, layout == ActionLayout::Stacked);
            if layout == ActionLayout::Full {
                self.play_search_actions(ui, false);
                self.extended_actions(ui, false);
            } else {
                let more = ui
                    .push_id("global-more", |ui| {
                        crate::chrome::quiet_menu(ui, "更多", |ui| {
                            ui.set_max_width((ui.ctx().screen_rect().width() - 48.0).max(160.0));
                            egui::ScrollArea::vertical()
                                .max_height((ui.ctx().screen_rect().height() - 40.0).max(100.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| self.play_search_actions(ui, true));
                                    ui.separator();
                                    self.extended_actions(ui, true);
                                });
                        })
                    })
                    .inner;
                record(ui, "global/more", &more.response);
            }
        });
    }

    fn play_search_actions(&mut self, ui: &mut egui::Ui, in_menu: bool) {
        let play = crate::chrome::quiet_button(ui, "▶ 试玩");
        record(ui, "global/play", &play);
        if play.clicked() {
            self.switch_tab(Tab::Play);
            if in_menu {
                ui.close();
            }
        }
        let search = crate::chrome::quiet_button(ui, "搜索")
            .on_hover_text("查找正文与源文件 · Ctrl/Cmd+Shift+F");
        record(ui, "global/search", &search);
        if search.clicked() {
            self.open_search(ui.ctx(), true, false);
            if in_menu {
                ui.close();
            }
        }
    }

    fn extended_actions(&mut self, ui: &mut egui::Ui, in_menu: bool) {
        self.export_publish_menu(ui);
        self.project_menu(ui);
        self.edit_menu(ui);
        self.workspace_view_menu(ui);
        let appearance = crate::chrome::quiet_button(ui, "外观")
            .on_hover_text("配色、工作台风格、密度与文字；仅此设备");
        record(ui, "global/appearance", &appearance);
        if appearance.clicked() {
            self.personal.preferences_open = true;
            if in_menu {
                ui.close();
            }
        }
    }
}
