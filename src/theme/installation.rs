//! Context 是主题真源。兼容零参数 helper 只借用显式渲染作用域的快照。
use super::{resolve, AppearancePreferences, ResolvedTheme};
use egui::{Color32, Stroke, TextStyle, Vec2};
use std::{cell::RefCell, marker::PhantomData, rc::Rc};

thread_local! {
    static FRAME_THEMES: RefCell<Vec<(Rc<()>, ResolvedTheme)>> = const { RefCell::new(Vec::new()) };
}

/// 必须保持到本次 UI 绘制结束；不可跨线程移动。Drop 也覆盖提前 return/嵌套 Context。
#[must_use = "Keep this guard alive for the complete UI frame"]
pub struct FrameThemeGuard {
    identity: Rc<()>,
    _thread_bound: PhantomData<Rc<()>>,
}
impl Drop for FrameThemeGuard {
    fn drop(&mut self) {
        FRAME_THEMES.with(|frames| {
            frames
                .borrow_mut()
                .retain(|(id, _)| !Rc::ptr_eq(id, &self.identity));
        });
    }
}
fn key() -> egui::Id {
    egui::Id::new("worldedit.resolved-theme.v2")
}
pub fn resolved(ctx: &egui::Context) -> ResolvedTheme {
    ctx.data(|data| data.get_temp::<ResolvedTheme>(key()))
        .unwrap_or_else(|| resolve(&AppearancePreferences::default(), ctx.system_theme()))
}
pub(super) fn current() -> ResolvedTheme {
    FRAME_THEMES
        .with(|frames| frames.borrow().last().map(|(_, theme)| theme.clone()))
        .unwrap_or_else(|| resolve(&AppearancePreferences::default(), None))
}
/// 只用于旧调用的显式模式适配。返回的 guard 与 configure_appearance 相同。
#[cfg(test)]
pub fn configure(ctx: &egui::Context, mode: super::ThemeMode) -> FrameThemeGuard {
    configure_appearance(
        ctx,
        &AppearancePreferences {
            theme: mode,
            ..Default::default()
        },
    )
}
pub fn configure_appearance(ctx: &egui::Context, p: &AppearancePreferences) -> FrameThemeGuard {
    let next = resolve(p, ctx.system_theme());
    let changed = ctx.data(|data| data.get_temp::<ResolvedTheme>(key()).as_ref() != Some(&next));
    if changed {
        install(ctx, &next);
        ctx.data_mut(|data| data.insert_temp(key(), next.clone()));
    }
    let identity = Rc::new(());
    FRAME_THEMES.with(|frames| frames.borrow_mut().push((identity.clone(), next)));
    FrameThemeGuard {
        identity,
        _thread_bound: PhantomData,
    }
}
fn install(ctx: &egui::Context, theme: &ResolvedTheme) {
    let c = theme.colors;
    let m = theme.metrics;
    let s = theme.shapes;
    ctx.set_theme(if theme.effective_mode == super::ThemeMode::Dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    let mut style = (*ctx.style()).clone();
    style.visuals = if theme.light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    let v = &mut style.visuals;
    v.override_text_color = Some(c.text);
    v.weak_text_color = Some(c.secondary);
    v.panel_fill = c.panel;
    v.window_fill = c.raised;
    v.extreme_bg_color = c.document;
    v.text_edit_bg_color = Some(c.document);
    v.code_bg_color = c.document;
    v.faint_bg_color = c.hover;
    v.window_corner_radius = s.window.into();
    v.menu_corner_radius = s.popup.into();
    v.window_stroke = Stroke::new(1.0_f32, c.control_border);
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;
    v.selection.bg_fill = c.selection;
    v.selection.stroke = Stroke::new(1.0_f32, c.selection_text);
    for widget in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
        &mut v.widgets.noninteractive,
    ] {
        widget.corner_radius = s.control.into();
        widget.bg_stroke = Stroke::new(1.0_f32, c.control_border);
        widget.fg_stroke = Stroke::new(1.0_f32, c.text);
        widget.expansion = 0.0;
    }
    v.widgets.inactive.bg_fill = c.document;
    // Resting buttons share their enclosing surface; boundaries remain only where needed.
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.noninteractive.bg_fill = c.panel;
    v.widgets.noninteractive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.hovered.bg_fill = c.hover;
    v.widgets.hovered.weak_bg_fill = c.hover;
    v.widgets.hovered.bg_stroke = Stroke::new(1.5_f32, c.control_border);
    v.widgets.active.bg_fill = c.selection;
    v.widgets.active.weak_bg_fill = c.selection;
    v.widgets.active.bg_stroke = Stroke::new(theme.focus_width, c.focus);
    v.widgets.open.bg_fill = c.selection;
    v.widgets.open.weak_bg_fill = c.selection;
    v.widgets.open.bg_stroke = Stroke::new(1.0_f32, c.control_border);
    v.hyperlink_color = c.accent;
    v.error_fg_color = c.danger;
    v.warn_fg_color = c.warning;
    v.text_cursor.stroke = Stroke::new(2.0_f32, c.focus);
    v.text_cursor.blink = !theme.preferences.reduce_motion;
    // Keep disabled text and outlines readable on the real surface. egui carries the
    // disabled interaction semantics; unavailable actions should also give their reason.
    v.disabled_alpha = if theme.preferences.high_contrast {
        1.0
    } else {
        0.99
    };
    v.collapsing_header_frame = theme.preferences.style == super::StylePreset::Technical;
    v.indent_has_left_vline = theme.preferences.style == super::StylePreset::Technical;
    v.striped = theme.preferences.style == super::StylePreset::Technical;
    style.animation_time = if theme.preferences.reduce_motion {
        0.0
    } else {
        0.1
    };
    style.scroll_animation = if theme.preferences.reduce_motion {
        egui::style::ScrollAnimation::none()
    } else {
        egui::style::ScrollAnimation::default()
    };
    style.spacing.item_spacing = Vec2::new(8.0, (m.row_height - 22.0) * 0.5);
    style.spacing.button_padding = m.button_padding;
    style.spacing.interact_size = Vec2::new(30.0, m.control_height);
    style.spacing.window_margin = egui::Margin::same(m.panel_margin as i8);
    style.spacing.menu_margin = egui::Margin::same(8);
    style
        .text_styles
        .insert(TextStyle::Body, theme.type_roles.ui.clone());
    style
        .text_styles
        .insert(TextStyle::Button, theme.type_roles.ui.clone());
    style
        .text_styles
        .insert(TextStyle::Heading, theme.type_roles.heading.clone());
    style
        .text_styles
        .insert(TextStyle::Small, theme.type_roles.meta.clone());
    style
        .text_styles
        .insert(TextStyle::Monospace, theme.type_roles.source.clone());
    ctx.set_style(style);
    // Application shortcuts update the same preferences, including an open preview.
    ctx.options_mut(|options| options.zoom_with_keyboard = false);
    ctx.set_zoom_factor(theme.preferences.ui_scale);
    #[cfg(target_arch = "wasm32")]
    super::web_shell::synchronize(theme);
}
