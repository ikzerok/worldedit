use super::*;

impl ReaderPublishState {
    pub(super) fn page_review_ui(
        &mut self,
        ui: &mut egui::Ui,
        height: f32,
        keyboard_allowed: bool,
    ) -> Option<PublishAction> {
        let top = ui.cursor().top();
        self.review_summary(ui);
        let Some(reviewed) = &self.reviewed else {
            return None;
        };
        if self.page_directory.visible {
            self.page_directory.directory_ui(
                ui,
                &reviewed.preview,
                (height - (ui.cursor().top() - top)).max(80.0),
                keyboard_allowed,
            );
            return None;
        }
        if ui.button("返回页面目录").clicked() {
            self.page_directory.show();
        }
        ui.label("下面是核心公开文本投影，静态条件和效果仅供阅读，不执行故事。");
        let count = reviewed.preview.content.len();
        let mut index = self.page_directory.opened_index().unwrap_or(0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(index > 0, egui::Button::new("上一阅读页"))
                .clicked()
            {
                index -= 1;
            }
            ui.label(format!(
                "阅读页 {} / {count}（全部公开页面的原顺序）",
                if count == 0 { 0 } else { index + 1 }
            ));
            if ui
                .add_enabled(index + 1 < count, egui::Button::new("下一阅读页"))
                .clicked()
            {
                index += 1;
            }
        });
        self.page_directory.open_in_public_order(index);
        #[cfg(not(target_arch = "wasm32"))]
        let mut action = None;
        #[cfg(target_arch = "wasm32")]
        let action = None;
        egui::ScrollArea::vertical()
            .id_salt(("reader-public-page", &self.page_directory.opened_path))
            .max_height((height - (ui.cursor().top() - top)).max(40.0))
            .auto_shrink([false, true])
            .show(ui, |ui| {
                if let Some(page) = reviewed.preview.content.get(index) {
                    ui.add(egui::Label::new(egui::RichText::new(&page.title).heading()).wrap());
                    ui.add(egui::Label::new(&page.output_path).wrap());
                    #[cfg(not(target_arch = "wasm32"))]
                    if ui.button("实际页面临时预览").clicked() {
                        action = Some(PublishAction::BrowserPreview(page.output_path.clone()));
                    }
                    if page.empty_content {
                        ui.colored_label(crate::theme::WARNING(), "此页没有静态阅读正文。");
                    } else {
                        ui.add(egui::Label::new(&page.text).wrap());
                    }
                    if let Some(bytes) = reviewed.files.get(std::path::Path::new(&page.output_path))
                    {
                        egui::CollapsingHeader::new("核对实际HTML文件").show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(String::from_utf8_lossy(bytes)).monospace(),
                                )
                                .wrap(),
                            );
                        });
                    }
                }
            });
        action
    }
}
