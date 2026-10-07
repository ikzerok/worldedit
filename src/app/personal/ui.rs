use super::*;
impl WorldeditApp {
    pub(in crate::app) fn focus_style(&self) -> bool {
        self.personal.appearance().style == crate::theme::StylePreset::Focus
    }

    /// 自适应折叠只影响本帧，不覆盖设备上选择的导航/参考宽度。
    pub(in crate::app) fn compact_reference_navigation(&self, ctx: &egui::Context) -> bool {
        let s = &self.personal.settings;
        s.references_visible
            && s.dock_references
            && !self.reading_panels.ids().is_empty()
            && ctx.screen_rect().width() - s.navigation_width - s.reference_width < 512.0
    }

    /// 收起辅助栏以保护当前内容；只改变这一帧，不覆写作者保存的宽度。
    pub(in crate::app) fn compact_workspace_navigation(&self, ctx: &egui::Context) -> bool {
        let content_min = 540.0;
        self.focus_style()
            || ctx.screen_rect().width() - self.personal.settings.navigation_width < content_min
            || self.compact_reference_navigation(ctx)
    }
    pub(in crate::app) fn compact_navigation_menu(&mut self, ui: &mut egui::Ui) {
        if self.focus_style() {
            if crate::chrome::quiet_button(ui, "导航")
                .on_hover_text("导航与文件 · Ctrl/Cmd+Shift+E；关闭临时窗口返回正文")
                .clicked()
            {
                self.open_navigation_drawer(ui.ctx());
            }
            return;
        }
        if self.personal.settings.navigation
            && !self.personal.settings.focus
            && !self.compact_workspace_navigation(ui.ctx())
        {
            return;
        }
        crate::chrome::quiet_menu(ui, "导航", |ui| {
            ui.label("工作区导航 · 保留当前内容的可用空间");
            for tab in [
                Tab::Manuscript,
                Tab::Overview,
                Tab::Edit,
                Tab::World,
                Tab::Characters,
                Tab::Catalog,
                Tab::CatalogImport,
                Tab::Wiki,
                Tab::Map,
                Tab::Network,
                Tab::Timeline,
                Tab::Graph,
                Tab::Review,
                Tab::Templates,
                Tab::Localization,
                Tab::CheckpointHistory,
                Tab::Play,
            ] {
                if ui.selectable_label(self.tab == tab, tab.title()).clicked() {
                    self.switch_tab(tab);
                    ui.close();
                }
            }
        });
    }

    pub(in crate::app) fn workspace_view_menu(&mut self, ui: &mut egui::Ui) {
        crate::chrome::quiet_menu(ui, "视图", |ui| {
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
            ui.checkbox(&mut self.personal.settings.diagnostics, "显示工程问题");
            ui.checkbox(&mut self.personal.settings.focus, "正文专注模式");
            ui.checkbox(&mut self.personal.settings.source_wrap, "源码自动换行")
                .on_hover_text("按编辑区宽度显示长段落；不修改源文件换行或内容");
            ui.checkbox(
                &mut self.personal.settings.references_visible,
                "显示固定参考",
            );
            ui.checkbox(
                &mut self.personal.settings.dock_references,
                "停靠参考（不覆盖正文）",
            );
            if ui
                .button("字体、行距与主题…")
                .on_hover_text("新版配色、工作台风格、密度与界面缩放")
                .clicked()
            {
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
}
