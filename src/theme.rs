//! 作者工作台的统一色彩、间距与控件样式。
use egui::{Color32, FontId, RichText, Stroke, TextStyle, Vec2};

mod sources;
pub use sources::{relative_source, source_caption, source_path, technical_value};

pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 12.0;
pub const SPACE_LG: f32 = 16.0;
pub const SPACE_XL: f32 = 24.0;
pub const CONTROL_HEIGHT: f32 = 32.0;
pub const INDEX_WIDTH: f32 = 232.0;
pub const INSPECTOR_WIDTH: f32 = 288.0;
pub const BODY_SIZE: f32 = 14.0;
pub const META_SIZE: f32 = 12.0;
pub const HEADING_SIZE: f32 = 22.0;

thread_local! { static LIGHT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

#[allow(non_snake_case)]
pub fn BG() -> Color32 {
    if LIGHT.get() {
        Color32::from_rgb(248, 249, 252)
    } else {
        Color32::from_rgb(22, 23, 27)
    }
}
#[allow(non_snake_case)]
pub fn PANEL() -> Color32 {
    if LIGHT.get() {
        Color32::from_rgb(242, 244, 248)
    } else {
        Color32::from_rgb(29, 30, 35)
    }
}
#[allow(non_snake_case)]
pub fn CARD() -> Color32 {
    if LIGHT.get() {
        Color32::from_rgb(255, 255, 255)
    } else {
        Color32::from_rgb(38, 40, 46)
    }
}
#[allow(non_snake_case)]
pub fn BORDER() -> Color32 {
    if LIGHT.get() {
        Color32::from_rgb(190, 197, 209)
    } else {
        Color32::from_rgb(53, 55, 63)
    }
}
#[allow(non_snake_case)]
pub fn TEXT() -> Color32 {
    if LIGHT.get() {
        Color32::from_rgb(31, 38, 50)
    } else {
        Color32::from_rgb(237, 239, 245)
    }
}
#[allow(non_snake_case)]
pub fn MUTED() -> Color32 {
    if LIGHT.get() {
        Color32::from_rgb(79, 91, 110)
    } else {
        Color32::from_rgb(156, 160, 172)
    }
}
#[allow(non_snake_case)]
pub fn ACCENT() -> Color32 {
    if LIGHT.get() {
        Color32::from_rgb(27, 81, 158)
    } else {
        Color32::from_rgb(143, 183, 248)
    }
}
#[allow(non_snake_case)]
pub fn BLUE() -> Color32 {
    if LIGHT.get() {
        Color32::from_rgb(36, 87, 152)
    } else {
        Color32::from_rgb(151, 179, 226)
    }
}
#[allow(non_snake_case)]
pub fn GOLD() -> Color32 {
    if LIGHT.get() {
        Color32::from_rgb(113, 75, 13)
    } else {
        Color32::from_rgb(217, 188, 142)
    }
}
#[allow(non_snake_case)]
pub fn ERROR() -> Color32 {
    if LIGHT.get() {
        Color32::from_rgb(166, 38, 48)
    } else {
        Color32::from_rgb(235, 143, 150)
    }
}

/// 警告与运行范围说明；必须同时保留具体文字原因。
#[allow(non_snake_case)]
pub fn WARNING() -> Color32 {
    GOLD()
}

/// 已正常结束等成功结果；不能仅用颜色表示完成。
#[allow(non_snake_case)]
pub fn SUCCESS() -> Color32 {
    if LIGHT.get() {
        Color32::from_rgb(30, 109, 48)
    } else {
        Color32::from_rgb(130, 220, 130)
    }
}

/// 运行锚点记录，保留 ◆、记录种类与名称作为非颜色标识。
#[allow(non_snake_case)]
pub fn ANCHOR() -> Color32 {
    if LIGHT.get() {
        Color32::from_rgb(14, 105, 81)
    } else {
        Color32::from_rgb(120, 220, 190)
    }
}

/// 错误文字与关闭悬停共用的可读背景；亮色不沿用深红底。
pub fn error_background() -> Color32 {
    if LIGHT.get() {
        Color32::from_rgb(255, 232, 235)
    } else {
        Color32::from_rgb(65, 36, 44)
    }
}

/// 当前来源的温和强调，与用户TextEdit选区分别绘制。
pub fn problem_source_background() -> Color32 {
    if LIGHT.get() { Color32::from_rgb(233, 241, 251) }
    else { Color32::from_rgb(34, 40, 47) }
}

pub fn is_light() -> bool {
    LIGHT.get()
}

pub fn configure(ctx: &egui::Context, mode: ThemeMode) {
    let light = match mode {
        ThemeMode::Dark => false,
        ThemeMode::Light => true,
        ThemeMode::System => ctx.system_theme() == Some(egui::Theme::Light),
    };
    let previous = LIGHT.replace(light);
    let key = egui::Id::new("worldedit.theme.installed");
    let installed = ctx.data(|data| data.get_temp::<bool>(key)).unwrap_or(false);
    if previous != light || !installed {
        install(ctx);
        ctx.data_mut(|data| data.insert_temp(key, true));
    }
}

pub fn install(ctx: &egui::Context) {
    let light = LIGHT.get();
    ctx.set_theme(if light {
        egui::Theme::Light
    } else {
        egui::Theme::Dark
    });
    let mut style = (*ctx.style()).clone();
    style.visuals = if light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    style.visuals.override_text_color = Some(TEXT());
    style.visuals.panel_fill = PANEL();
    style.visuals.window_fill = PANEL();
    style.visuals.extreme_bg_color = BG();
    style.visuals.faint_bg_color = CARD();
    style.visuals.window_corner_radius = 14.into();
    style.visuals.window_stroke = Stroke::new(1.0_f32, BORDER());
    style.visuals.selection.bg_fill = if light {
        Color32::from_rgb(207, 224, 249)
    } else {
        Color32::from_rgb(39, 51, 70)
    };
    style.visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT());
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.noninteractive,
    ] {
        widget.corner_radius = 8.into();
        widget.bg_stroke = Stroke::new(1.0_f32, BORDER());
        widget.fg_stroke = Stroke::new(1.0_f32, TEXT());
        widget.expansion = 0.0;
    }
    style.visuals.widgets.inactive.bg_fill = CARD();
    style.visuals.widgets.inactive.weak_bg_fill = CARD();
    // 未选checkbox与卡片同色时仍须有轮廓；不改变整个工作台的边框色。
    let control_outline = if light {
        Color32::from_rgb(105, 117, 136)
    } else {
        Color32::from_rgb(128, 136, 153)
    };
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, control_outline);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, control_outline);
    style.visuals.disabled_alpha = 0.65;
    style.visuals.widgets.hovered.bg_fill = if light {
        Color32::from_rgb(226, 233, 244)
    } else {
        Color32::from_rgb(49, 52, 61)
    };
    style.visuals.widgets.hovered.weak_bg_fill = style.visuals.widgets.hovered.bg_fill;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, ACCENT());
    style.visuals.widgets.active.bg_fill = if light {
        Color32::from_rgb(208, 222, 245)
    } else {
        Color32::from_rgb(39, 51, 70)
    };
    style.visuals.widgets.active.weak_bg_fill = style.visuals.widgets.active.bg_fill;
    style.visuals.widgets.active.bg_stroke = Stroke::new(2.0_f32, ACCENT());
    style.visuals.hyperlink_color = ACCENT();
    style.animation_time = 0.12;
    style.spacing.item_spacing = Vec2::splat(SPACE_SM);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.interact_size = Vec2::new(36.0, CONTROL_HEIGHT);
    style.spacing.window_margin = egui::Margin::same(20);
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(BODY_SIZE));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(BODY_SIZE));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(HEADING_SIZE));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(META_SIZE));
    ctx.set_style(style);
}

pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text).color(MUTED()).size(META_SIZE)
}
pub fn primary(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).color(BG()).strong()).fill(ACCENT())
}
pub fn panel() -> egui::Frame {
    egui::Frame::new().fill(PANEL()).inner_margin(18)
}
pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD())
        .corner_radius(12)
        .inner_margin(SPACE_LG as i8)
        .stroke(Stroke::new(1.0_f32, BORDER()))
}

/// 集合索引与文档/画布表面分开，窄栏采用较小内边距。
pub fn index_panel() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL())
        .inner_margin(SPACE_MD as i8)
}

pub fn canvas_background() -> Color32 {
    BG()
}

pub fn document_background() -> Color32 {
    CARD()
}

/// 同一节奏用于功能页标题；说明自动换行，不挤占右侧操作空间。
pub fn page_heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.heading(title);
    if !subtitle.is_empty() {
        ui.add(egui::Label::new(muted(subtitle)).wrap());
    }
    ui.add_space(SPACE_SM);
}

/// 密集工具区与阅读区分开，窄面板时自然换行。
pub fn toolbar<R>(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let result = ui
        .horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            contents(ui)
        })
        .inner;
    ui.add_space(SPACE_SM);
    result
}

#[cfg(test)]
mod tests;
