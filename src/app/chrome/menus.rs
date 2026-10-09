use super::*;

impl WorldeditApp {
    pub(super) fn project_menu(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        crate::chrome::quiet_menu(ui, "工程", |ui| {
            if ui.button("语言与资料能力…").clicked() {
                self.open_capabilities();
                ui.close();
            }
            if ui.button("管理工程模板").clicked() {
                self.tab = Tab::Templates;
                ui.close();
            }
            if ui.button("本地化工作台").clicked() {
                self.tab = Tab::Localization;
                ui.close();
            }
            if ui.button("检查点历史…").clicked() {
                self.tab = Tab::CheckpointHistory;
                ui.close();
            }
            if ui.button("新建世界").clicked() {
                self.request_action(Pending::New, &ctx);
                ui.close();
            }
            if ui.button("打开文件夹…").clicked() {
                self.open_dialog(&ctx, true);
                ui.close();
            }
            if ui.button("选择工作区…  Ctrl+O").clicked() {
                self.open_dialog(&ctx, false);
                ui.close();
            }
            if ui.button("另存工程…").clicked() {
                self.directory_dialog(false);
                ui.close();
            }
            if ui.button("世界资料导入…").clicked() {
                self.open_catalog_import();
                ui.close();
            }
            if ui.button("导入 Markdown…").clicked() {
                ui.close();
                self.markdown_import_wizard = Some(markdown_import_ui::Wizard::default());
            }
            if ui.button("从磁盘重新载入").clicked() {
                self.request_action(Pending::Open(self.project.entry.clone()), &ctx);
                ui.close();
            }
            #[cfg(not(target_arch = "wasm32"))]
            if ui.button("查看冲突差异").clicked() {
                self.conflict_view.open_or_capture(&self.project);
                ui.close();
            }
        });
    }
}
