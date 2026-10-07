//! 纯布局计算；不改变个人布局/固定对象，也不读取线程快照。
use super::{ResolvedTheme, StylePreset};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DocumentGeometry {
    pub outer_width: f32,
    pub inset_x: f32,
    pub inset_y: f32,
    pub content_width: f32,
    pub rail_width: f32,
    pub rail_gap: f32,
    pub padding: egui::Vec2,
}
pub fn document_geometry(
    theme: &ResolvedTheme,
    available_width: f32,
    max_width: f32,
) -> DocumentGeometry {
    let available = if available_width.is_finite() {
        available_width.max(1.0)
    } else {
        1.0
    };
    let desired = if max_width.is_finite() {
        max_width.max(1.0)
    } else {
        available
    };
    let style = theme.preferences.style;
    let centered = matches!(style, StylePreset::Manuscript | StylePreset::Focus);
    let stage = theme.metrics.stage_margin.min((available * 0.04).max(0.0));
    let width = if centered {
        desired.min((available - stage * 2.0).max(1.0))
    } else if style == StylePreset::Technical {
        available
    } else {
        desired.min(available)
    };
    let horizontal = theme
        .metrics
        .document_margin
        .x
        .min((width * 0.07).max(4.0))
        .min((width - 1.0) * 0.5)
        .max(0.0)
        .floor();
    let padding = egui::vec2(horizontal, theme.metrics.document_margin.y);
    let inner_width = (width - horizontal * 2.0).max(1.0);
    let (rail_width, rail_gap) =
        if style == StylePreset::Ledger && inner_width >= 112.0 + 16.0 + 480.0 {
            (112.0, 16.0)
        } else {
            (0.0, 0.0)
        };
    DocumentGeometry {
        outer_width: width,
        inset_x: if centered {
            (available - width) * 0.5
        } else {
            0.0
        },
        inset_y: match style {
            StylePreset::Manuscript => 12.0,
            StylePreset::Focus => 16.0,
            _ => 0.0,
        },
        content_width: (inner_width - rail_width - rail_gap).max(1.0),
        rail_width,
        rail_gap,
        padding,
    }
}
