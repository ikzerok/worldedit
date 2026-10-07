//! 可读且可辨的不可用状态；原 Ui 和 egui 的身份/交互路径保持不变。
use super::{resolved, Colors};
use egui::{InnerResponse, Response, Style, Ui, Widget};
use std::sync::Arc;

struct RestoreStyle<'a> {
    ui: &'a mut Ui,
    original: Arc<Style>,
}
impl Drop for RestoreStyle<'_> {
    fn drop(&mut self) {
        self.ui.set_style(self.original.clone());
    }
}
fn unavailable_style(ui: &mut Ui, colors: Colors) {
    let v = ui.visuals_mut();
    // egui keeps a disabled widget's click Sense and normally only fades its
    // Painter. Its noninteractive slot therefore is not a disabled-state slot.
    v.override_text_color = Some(colors.disabled);
    v.weak_text_color = Some(colors.disabled);
    v.hyperlink_color = colors.disabled;
    v.disabled_alpha = 1.0;
    for widget in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
        &mut v.widgets.noninteractive,
    ] {
        widget.fg_stroke.color = colors.disabled;
        widget.bg_fill = colors.hover;
        widget.weak_bg_fill = colors.hover;
        widget.bg_stroke.color = colors.control_border;
    }
    // Explicit RichText colors, selection, typography and author-painted content
    // are not rewritten. TextEdit also keeps its established document surface.
}
fn guard(ui: &mut Ui, colors: Colors) -> RestoreStyle<'_> {
    let original = ui.style().clone();
    unavailable_style(ui, colors);
    RestoreStyle { ui, original }
}

pub fn add_enabled(ui: &mut Ui, enabled: bool, widget: impl Widget) -> Response {
    if enabled && ui.is_enabled() {
        return ui.add_enabled(enabled, widget);
    }
    let colors = resolved(ui.ctx()).colors;
    add_enabled_with_colors(ui, enabled, widget, colors)
}
/// A same-frame preview may resolve new colors before the frame Context changes.
pub fn add_enabled_with_colors(
    ui: &mut Ui,
    enabled: bool,
    widget: impl Widget,
    colors: Colors,
) -> Response {
    if enabled && ui.is_enabled() {
        return ui.add_enabled(enabled, widget);
    }
    let guard = guard(ui, colors);
    guard.ui.add_enabled(enabled, widget)
}
pub fn add_enabled_ui<R>(
    ui: &mut Ui,
    enabled: bool,
    contents: impl FnOnce(&mut Ui) -> R,
) -> InnerResponse<R> {
    if enabled && ui.is_enabled() {
        return ui.add_enabled_ui(enabled, contents);
    }
    let colors = resolved(ui.ctx()).colors;
    let guard = guard(ui, colors);
    // The original API already creates exactly one child. No wrapper scope is added.
    guard.ui.add_enabled_ui(enabled, contents)
}
/// Match Ui::disable for the remainder of this existing Ui, without a new scope.
pub fn disable(ui: &mut Ui) {
    let colors = resolved(ui.ctx()).colors;
    unavailable_style(ui, colors);
    ui.disable();
}
