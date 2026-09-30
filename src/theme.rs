//! 作者工作台的统一色彩、间距与控件样式。
use egui::{Color32, FontId, RichText, Stroke, TextStyle, Vec2};

pub const BG: Color32 = Color32::from_rgb(22, 23, 27);
pub const PANEL: Color32 = Color32::from_rgb(29, 30, 35);
pub const CARD: Color32 = Color32::from_rgb(38, 40, 46);
pub const BORDER: Color32 = Color32::from_rgb(53, 55, 63);
pub const TEXT: Color32 = Color32::from_rgb(237, 239, 245);
pub const MUTED: Color32 = Color32::from_rgb(156, 160, 172);
pub const ACCENT: Color32 = Color32::from_rgb(143, 183, 248);
pub const BLUE: Color32 = Color32::from_rgb(151, 179, 226);
pub const GOLD: Color32 = Color32::from_rgb(217, 188, 142);
pub const ERROR: Color32 = Color32::from_rgb(235, 143, 150);

pub fn install(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.panel_fill = PANEL;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = BG;
    style.visuals.faint_bg_color = CARD;
    style.visuals.window_corner_radius = 14.into();
    style.visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.selection.bg_fill = Color32::from_rgb(49, 66, 92);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.noninteractive,
    ] {
        widget.corner_radius = 8.into();
        widget.bg_stroke = Stroke::new(1.0_f32, BORDER);
        widget.fg_stroke = Stroke::new(1.0_f32, TEXT);
        widget.expansion = 0.0;
    }
    style.visuals.widgets.inactive.bg_fill = CARD;
    style.visuals.widgets.inactive.weak_bg_fill = CARD;
    style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(49, 52, 61);
    style.visuals.widgets.hovered.weak_bg_fill = style.visuals.widgets.hovered.bg_fill;
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(51, 64, 84);
    style.visuals.widgets.active.weak_bg_fill = style.visuals.widgets.active.bg_fill;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    style.visuals.hyperlink_color = ACCENT;
    style.animation_time = 0.12;
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.interact_size = Vec2::new(36.0, 32.0);
    style.spacing.window_margin = egui::Margin::same(20);
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(22.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));
    ctx.set_style(style);
}

pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text).color(MUTED).size(12.0)
}
pub fn primary(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(BG).strong()).fill(ACCENT)
}
pub fn panel() -> egui::Frame {
    egui::Frame::new().fill(PANEL).inner_margin(18)
}
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .corner_radius(12)
        .inner_margin(16)
        .stroke(Stroke::new(1.0_f32, BORDER))
}

/// 同一节奏用于功能页标题；说明自动换行，不挤占右侧操作空间。
pub fn page_heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.heading(title);
    if !subtitle.is_empty() {
        ui.add(egui::Label::new(muted(subtitle)).wrap());
    }
    ui.add_space(8.0);
}

/// 密集工具区与阅读区分开，窄面板时自然换行。
pub fn toolbar<R>(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let result = ui
        .horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            contents(ui)
        })
        .inner;
    ui.add_space(8.0);
    result
}
