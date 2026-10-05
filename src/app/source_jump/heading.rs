use super::*;
impl WorldeditApp {
    pub(in crate::app) fn source_jump_heading(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        relative: &str,
    ) -> egui::Response {
        let position = self.source_jump_position(ctx);
        let caption = position
            .as_ref()
            .map(|position| format!("第 {} 行 · 第 {} 列", position.line, position.column))
            .unwrap_or_else(|_| "行列暂不可用".into());
        let hint = format!("{caption}\n{}", position.err().unwrap_or_else(|| "跳转到行列 · Ctrl+G（Mac 为 Control+G）\n行列从 1 开始；列按 Unicode 标量计数，Tab 为 1 列".into()));
        // 始终预留同一入口宽度；位数、不可用提示与长文件名不挤走正文。
        let width =
            (ui.text_style_height(&egui::TextStyle::Body) * 14.0).min(ui.available_width() * 0.58);
        let height = ui
            .text_style_height(&egui::TextStyle::Heading)
            .max(ui.spacing().interact_size.y);
        ui.horizontal(|ui| {
            let title_width = (ui.available_width() - width - ui.spacing().item_spacing.x).max(0.0);
            ui.allocate_ui_with_layout(
                egui::vec2(title_width, height),
                egui::Layout::left_to_right(egui::Align::Center).with_main_align(egui::Align::Min),
                |ui| {
                    ui.set_min_size(egui::vec2(title_width, height));
                    ui.add(egui::Label::new(egui::RichText::new(relative).heading()).truncate())
                        .on_hover_text(relative);
                },
            );
            let response = ui
                .add_sized([width, height], egui::Button::new(caption).truncate())
                .on_hover_text(hint);
            if response.clicked() {
                self.open_source_jump(ctx);
            }
            response
        })
        .inner
    }
}
