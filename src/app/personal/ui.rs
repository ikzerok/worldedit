use super::*;
impl WorldeditApp {
    /// 自适应折叠只影响本帧，不覆盖设备上选择的导航/参考宽度。
    pub(in crate::app) fn compact_reference_navigation(&self, ctx: &egui::Context) -> bool {
        let s = &self.personal.settings;
        s.references_visible
            && s.dock_references
            && !self.reading_panels.ids().is_empty()
            && ctx.screen_rect().width() - s.navigation_width - s.reference_width < 512.0
    }
    pub(in crate::app) fn compact_navigation_menu(&mut self, ui: &mut egui::Ui) {
        if !self.compact_reference_navigation(ui.ctx()) {
            return;
        }
        ui.menu_button("导航", |ui| {
            ui.label("固定参考展开时暂收侧栏");
            for tab in [
                Tab::Manuscript,
                Tab::Overview,
                Tab::Edit,
                Tab::World,
                Tab::Characters,
                Tab::Catalog,
                Tab::Wiki,
                Tab::Map,
                Tab::Network,
                Tab::Timeline,
                Tab::Graph,
                Tab::Review,
                Tab::Templates,
                Tab::Localization,
                Tab::CheckpointHistory,
            ] {
                if ui.selectable_label(self.tab == tab, tab.title()).clicked() {
                    self.switch_tab(tab);
                    ui.close();
                }
            }
        });
    }

    pub(in crate::app) fn workspace_view_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("视图", |ui| {
            if ui.button("快速切换 · Ctrl/Cmd+P").clicked() {
                self.open_commands(ui.ctx(), false);
                ui.close();
            }
            if ui.button("任务命令 · Ctrl/Cmd+Shift+P").clicked() {
                self.open_commands(ui.ctx(), true);
                ui.close();
            }
            ui.separator();
            ui.checkbox(&mut self.personal.settings.navigation, "显示导航");
            ui.checkbox(&mut self.personal.settings.diagnostics, "显示诊断栏");
            ui.checkbox(&mut self.personal.settings.focus, "正文专注模式");
            ui.checkbox(
                &mut self.personal.settings.references_visible,
                "显示固定参考",
            );
            ui.checkbox(
                &mut self.personal.settings.dock_references,
                "停靠参考（不覆盖正文）",
            );
            if ui.button("字体、行距与主题…").clicked() {
                self.personal.preferences_open = true;
                ui.close();
            }
            if ui.button("重置布局").clicked() {
                let s = &mut self.personal.settings;
                s.navigation = true;
                s.diagnostics = false;
                s.focus = false;
                s.dock_references = true;
                s.references_visible = true;
                s.navigation_width = 212.0;
                s.reference_width = 330.0;
                ui.ctx().memory_mut(|memory| memory.reset_areas());
                self.message = Some("个人布局已重置；作品未改变".into());
                ui.close();
            }
        });
    }
    pub(in crate::app) fn preferences_window(&mut self, ctx: &egui::Context) {
        if !self.personal.preferences_open {
            return;
        }
        let mut open = true;
        egui::Window::new("阅读与外观 · 仅此设备").open(&mut open).collapsible(false)
            .default_width(420.0).show(ctx,|ui| {
                let s=&mut self.personal.settings;
                ui.horizontal_wrapped(|ui| {
                    ui.label("主题");
                    ui.selectable_value(&mut s.theme,crate::theme::ThemeMode::Dark,"暗色");
                    ui.selectable_value(&mut s.theme,crate::theme::ThemeMode::Light,"亮色");
                    ui.selectable_value(&mut s.theme,crate::theme::ThemeMode::System,"跟随系统");
                });
                ui.add(egui::Slider::new(&mut s.body_size,12.0..=28.0).text("正文字号"));
                ui.add(egui::Slider::new(&mut s.line_spacing,1.0..=2.0).text("行距倍数"));
                ui.add(egui::Slider::new(&mut s.reading_width,480.0..=1400.0).text("阅读宽度"));
                ui.separator();
                ui.label(egui::RichText::new("潮水涨起时，留在纸上的名字仍然清晰。Aa 0123").size(s.body_size));
                ui.label("仅影响显示，不写入作品或共享查询。Ctrl/Cmd+P 切换对象；Ctrl/Cmd+Shift+P 任务命令；Alt+← 返回。");
                if ui.button("关闭设置").clicked() { self.personal.preferences_open=false; }
            });
        if !open {
            self.personal.preferences_open = false;
        }
    }
}
