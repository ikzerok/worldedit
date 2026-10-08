use super::*;
use egui::emath::GuiRounding;
impl WorldeditApp {
    pub(in crate::app) fn source_jump_heading(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        relative: &str,
    ) -> egui::Response {
        self.source_position_heading(ctx, ui, relative, false)
    }

    pub(in crate::app) fn source_jump_compact_heading(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        relative: &str,
    ) -> egui::Response {
        self.source_position_heading(ctx, ui, relative, true)
    }

    fn source_position_heading(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        relative: &str,
        compact: bool,
    ) -> egui::Response {
        let position = self.source_jump_position(ctx);
        let caption = position
            .as_ref()
            .map(|position| format!("第 {} 行 · 第 {} 列", position.line, position.column))
            .unwrap_or_else(|_| "行列暂不可用".into());
        let button_caption = if compact {
            position
                .as_ref()
                .map(|position| format!("{}:{}", position.line, position.column))
                .unwrap_or_else(|_| "行列…".into())
        } else {
            caption.clone()
        };
        let hint = format!("{caption}\n{}", position.err().unwrap_or_else(|| "跳转到行列 · Ctrl+G（Mac 为 Control+G）\n行列从 1 开始；列按 Unicode 标量计数，Tab 为 1 列".into()));
        // 始终预留同一入口宽度；位数、不可用提示与长文件名不挤走正文。
        let width = (ui.text_style_height(&egui::TextStyle::Body)
            * if compact { 6.0 } else { 14.0 })
        .min(ui.available_width() * 0.58);
        let height = ui
            .text_style_height(&egui::TextStyle::Heading)
            .max(ui.spacing().interact_size.y);
        // 固定父布局预算，子标题的像素取整或截断不能反向扩宽这一行。
        let height = height.round_to_pixels(ui.pixels_per_point());
        let width = width.round_to_pixels(ui.pixels_per_point());
        let (row, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), height),
            egui::Sense::hover(),
        );
        let position_rect = egui::Rect::from_min_max(
            egui::pos2(row.right() - width, row.top()),
            row.right_bottom(),
        );
        let title_rect = egui::Rect::from_min_max(
            row.min,
            egui::pos2(
                (position_rect.left() - ui.spacing().item_spacing.x).max(row.left()),
                row.bottom(),
            ),
        );
        let mut title_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("source-jump-filename")
                .max_rect(title_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        title_ui.set_clip_rect(ui.clip_rect().intersect(title_rect));
        title_ui
            .add(egui::Label::new(egui::RichText::new(relative).heading()).truncate())
            .on_hover_text(relative);
        let response = ui
            .place(position_rect, egui::Button::new(button_caption).truncate())
            .on_hover_text(hint);
        if response.clicked() {
            self.open_source_jump(ctx);
        }
        response
    }
}
