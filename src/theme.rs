//! v0.30 语义色、结构、密度与排版。设备偏好经每 Context 解析后使用。
use egui::{Color32, FontId, RichText, Stroke, Vec2};
mod availability;
mod geometry;
mod installation;
mod palette;
mod preferences;
mod resolved;
mod sources;
mod surfaces;
#[cfg(target_arch = "wasm32")]
mod web_shell;
#[cfg(test)]
pub use installation::configure;
pub use installation::{resolved, FrameThemeGuard};
pub fn configure_appearance(ctx: &egui::Context, p: &AppearancePreferences) -> FrameThemeGuard {
    installation::configure_appearance(ctx, p)
}
pub use availability::{add_enabled, add_enabled_ui, add_enabled_with_colors, disable};
pub use geometry::{document_geometry, DocumentGeometry};
pub use palette::{Colors, SyntaxPalette};
pub(crate) use preferences::bounded;
pub use preferences::{
    AccentChoice, AppearancePreferences, BodyFamily, Density, PaletteId, PaletteModeSupport,
    StylePreset, ThemeMode,
};
pub use resolved::{resolve, Metrics, ResolvedTheme, Shapes};
pub use sources::{relative_source, source_caption, source_path, technical_value};
pub use surfaces::{
    card, chrome, index_panel, page_heading, panel, panel_header, popup, selection_frame, toolbar,
};

pub fn document_surface<R>(
    ui: &mut egui::Ui,
    max_width: f32,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    document_surface_titled(ui, "内容", max_width, contents)
}
pub fn document_surface_titled<R>(
    ui: &mut egui::Ui,
    label: &str,
    max_width: f32,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    surfaces::document_surface_titled(ui, label, max_width, contents)
}

pub const SPACE_XS: f32 = 4.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 12.0;
pub const SPACE_LG: f32 = 16.0;
pub const SPACE_XL: f32 = 24.0;
#[cfg(test)]
pub const CONTROL_HEIGHT: f32 = 30.0;
pub const INDEX_WIDTH: f32 = 232.0;
pub const INSPECTOR_WIDTH: f32 = 288.0;
pub const BODY_SIZE: f32 = 14.0;
pub const META_SIZE: f32 = 13.0;
pub const HEADING_SIZE: f32 = 22.0;

macro_rules! color_role {
    ($($name:ident => $field:ident),+ $(,)?) => {$(
        #[allow(non_snake_case)]
        pub fn $name() -> Color32 { installation::current().colors.$field }
    )+};
}
color_role! {
    BG => workspace, CHROME => chrome, NAVIGATION => chrome, PANEL => panel,
    DOCUMENT => document, CARD => raised, TEXT => text, MUTED => secondary,
    BORDER => subtle_border, CONTROL_BORDER => control_border, ACCENT => accent,
    HOVER => hover, SELECTION => selection, FOCUS => focus,
    BLUE => info, GOLD => warning, ERROR => danger, WARNING => warning,
    SUCCESS => success, ANCHOR => success,
}
#[cfg(test)]
pub fn is_light() -> bool {
    installation::current().light
}
pub fn metrics() -> Metrics {
    installation::current().metrics
}
pub fn focus_width() -> f32 {
    installation::current().focus_width
}
pub fn shapes() -> Shapes {
    installation::current().shapes
}
pub fn style_preset() -> StylePreset {
    installation::current().preferences.style
}
pub fn syntax_palette() -> SyntaxPalette {
    installation::current().syntax
}
pub fn body_font(size: f32) -> FontId {
    let mut font = installation::current().type_roles.body;
    font.size = size;
    font
}
pub fn source_font(size: f32) -> FontId {
    let mut font = installation::current().type_roles.source;
    font.size = size;
    font
}
pub fn canvas_background() -> Color32 {
    BG()
}
pub fn document_background() -> Color32 {
    DOCUMENT()
}
pub fn error_background() -> Color32 {
    installation::current().colors.invalid_background
}
pub fn problem_source_background() -> Color32 {
    HOVER()
}
pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text).color(MUTED()).size(META_SIZE)
}
pub fn primary(text: &str) -> impl egui::Widget + '_ {
    move |ui: &mut egui::Ui| {
        let theme = resolved(ui.ctx());
        let c = theme.colors;
        let enabled = ui.is_enabled();
        let response = ui
            .scope(|ui| {
                let visuals = ui.visuals_mut();
                for (state, fill) in [
                    (&mut visuals.widgets.inactive, c.accent),
                    (&mut visuals.widgets.hovered, c.accent_hover),
                    (&mut visuals.widgets.active, c.accent_hover),
                    (&mut visuals.widgets.open, c.accent),
                ] {
                    state.bg_fill = if enabled { fill } else { c.hover };
                    state.weak_bg_fill = state.bg_fill;
                    state.bg_stroke =
                        Stroke::new(1.0_f32, if enabled { fill } else { c.control_border });
                    state.fg_stroke =
                        Stroke::new(1.0_f32, if enabled { c.on_accent } else { c.disabled });
                }
                ui.add(
                    egui::Button::new(
                        RichText::new(text)
                            .color(if enabled { c.on_accent } else { c.disabled })
                            .strong(),
                    )
                    .corner_radius(theme.shapes.control)
                    .frame(true),
                )
            })
            .inner;
        if response.has_focus() {
            ui.painter().rect_stroke(
                response.rect.shrink(2.0),
                theme.shapes.control,
                Stroke::new(theme.focus_width, c.on_accent),
                egui::StrokeKind::Inside,
            );
        }
        response
    }
}
/// egui 0.32 shares the rail color with inactive widget fill; scope that override
/// to sliders so checkbox/input surfaces retain their own contrast relationships.
pub fn slider(ui: &mut egui::Ui, slider: egui::Slider<'_>) -> egui::Response {
    let theme = resolved(ui.ctx());
    let response = ui
        .scope(|ui| {
            ui.visuals_mut().widgets.inactive.bg_fill = theme.colors.control_border;
            ui.spacing_mut().slider_rail_height = 4.0;
            ui.add(slider)
        })
        .inner;
    selection_frame(ui, &response, false);
    if response.gained_focus() {
        response.scroll_to_me(Some(egui::Align::Center));
    }
    response
}
#[cfg(test)]
mod contract_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod extension_tests;
#[cfg(test)]
mod state_render_tests;

#[cfg(test)]
mod reviewed_tokens_tests;
#[cfg(test)]
mod stylized_tests;

#[cfg(test)]
mod availability_tests;
