//! 作者工作台视觉系统：石墨底色、低噪声层级与克制的青绿强调。
use egui::{Color32, FontId, RichText, Stroke, TextStyle, Vec2};

pub const BG: Color32 = Color32::from_rgb(23, 27, 33);
pub const SIDEBAR: Color32 = Color32::from_rgb(17, 21, 27);
pub const PANEL: Color32 = Color32::from_rgb(28, 33, 41);
pub const CARD: Color32 = Color32::from_rgb(33, 40, 49);
pub const BORDER: Color32 = Color32::from_rgb(49, 59, 71);
pub const TEXT: Color32 = Color32::from_rgb(235, 239, 245);
pub const MUTED: Color32 = Color32::from_rgb(166, 178, 193);
pub const ACCENT: Color32 = Color32::from_rgb(153, 205, 188);
pub const BLUE: Color32 = Color32::from_rgb(151, 182, 242);
pub const GOLD: Color32 = Color32::from_rgb(222, 189, 138);
pub const ERROR: Color32 = Color32::from_rgb(247, 155, 164);
pub const SELECTED: Color32 = Color32::from_rgb(36, 57, 57);
pub const HOVER: Color32 = Color32::from_rgb(42, 50, 61);

pub fn install(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.panel_fill = PANEL;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = SIDEBAR;
    style.visuals.faint_bg_color = CARD;
    style.visuals.window_corner_radius = 10.into();
    style.visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.selection.bg_fill = SELECTED;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = 6.into();
        widget.bg_stroke = Stroke::new(1.0_f32, BORDER);
        widget.fg_stroke = Stroke::new(1.0_f32, TEXT);
        widget.expansion = 0.0;
    }
    style.visuals.widgets.inactive.bg_fill = CARD;
    style.visuals.widgets.inactive.weak_bg_fill = CARD;
    style.visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    style.visuals.widgets.hovered.bg_fill = HOVER;
    style.visuals.widgets.hovered.weak_bg_fill = HOVER;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, BORDER);
    style.visuals.widgets.active.bg_fill = SELECTED;
    style.visuals.widgets.active.weak_bg_fill = SELECTED;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    style.visuals.widgets.open.bg_fill = SELECTED;
    style.visuals.widgets.open.weak_bg_fill = SELECTED;
    style.visuals.hyperlink_color = ACCENT;
    style.animation_time = 0.10;
    style.spacing.item_spacing = Vec2::new(8.0, 7.0);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.interact_size = Vec2::new(32.0, 30.0);
    style.spacing.window_margin = egui::Margin::same(20);
    for (text_style, size) in [
        (TextStyle::Body, 14.0),
        (TextStyle::Button, 14.0),
        (TextStyle::Heading, 24.0),
        (TextStyle::Small, 12.0),
    ] {
        style
            .text_styles
            .insert(text_style, FontId::proportional(size));
    }
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(13.0));
    ctx.set_style(style);
}

pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text).color(MUTED).size(12.5)
}
pub fn primary(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(SIDEBAR).strong())
        .fill(ACCENT)
        .min_size(Vec2::new(0.0, 32.0))
}
pub fn panel() -> egui::Frame {
    egui::Frame::new().fill(PANEL).inner_margin(18)
}
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD)
        .corner_radius(9)
        .inner_margin(18)
        .stroke(Stroke::new(1.0_f32, BORDER))
}

/// 标签只表达状态，不占用主要动作的视觉权重。
pub fn badge(ui: &mut egui::Ui, text: &str, color: Color32) {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.10))
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(7, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.0).color(color));
        });
}

pub fn page_heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).strong().size(24.0).color(TEXT));
    ui.add_space(2.0);
    ui.label(muted(subtitle));
    ui.add_space(12.0);
}

/// 阅读列居中且限制行长；空间不足时自然缩小，不撑出视口。
pub fn reading_column<R>(ui: &mut egui::Ui, draw: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let width = ui.available_width().min(880.0);
    let inset = ((ui.available_width() - width) * 0.5).max(0.0);
    ui.horizontal_top(|ui| {
        ui.add_space(inset);
        ui.allocate_ui_with_layout(
            Vec2::new(width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            draw,
        )
        .inner
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;
    fn luminance(c: Color32) -> f32 {
        let linear = |v: u8| {
            let s = f32::from(v) / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
    }
    #[test]
    fn normal_text_and_primary_action_have_readable_contrast() {
        for surface in [BG, SIDEBAR, PANEL, CARD, SELECTED] {
            for ink in [TEXT, MUTED] {
                let ratio = (luminance(ink) + 0.05) / (luminance(surface) + 0.05);
                assert!(ratio >= 4.5, "text contrast {ratio:.2}");
            }
        }
        assert!((luminance(ACCENT) + 0.05) / (luminance(SIDEBAR) + 0.05) >= 4.5);
    }
    #[test]
    fn reading_column_fits_narrow_and_wide_content_areas() {
        for width in [340.0, 700.0, 1400.0] {
            let ctx = egui::Context::default();
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(width, 760.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let bounds = ui.max_rect();
                        reading_column(ui, |ui| {
                            assert!(ui.available_width() <= 880.1);
                            assert!(bounds.contains_rect(ui.max_rect()));
                        });
                    });
                },
            );
        }
    }
}
