//! 独立结构共享内容与交互语义；不以换色或圆角代替布局。
use super::*;

pub fn panel() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL())
        .inner_margin(metrics().panel_margin as i8)
}
pub fn chrome() -> egui::Frame {
    egui::Frame::new()
        .fill(CHROME())
        .inner_margin(egui::Margin::symmetric(
            metrics().navigation_margin as i8,
            6,
        ))
}
pub fn popup() -> egui::Frame {
    egui::Frame::new()
        .fill(CARD())
        .corner_radius(shapes().popup)
        .stroke(Stroke::new(1.0_f32, CONTROL_BORDER()))
        .inner_margin(12)
}
/// Compatibility surface: no pill, nested shadow or universal card outline.
pub fn card() -> egui::Frame {
    let theme = installation::current();
    let frame = egui::Frame::new().inner_margin(theme.metrics.panel_margin as i8);
    match theme.preferences.style {
        StylePreset::Studio => frame.fill(theme.colors.panel),
        StylePreset::Manuscript => frame
            .fill(theme.colors.document)
            .corner_radius(theme.shapes.document),
        StylePreset::Technical => frame
            .fill(theme.colors.panel)
            .stroke(Stroke::new(1.0_f32, theme.colors.subtle_border)),
        StylePreset::Focus => frame.fill(Color32::TRANSPARENT),
        StylePreset::Ledger => frame.fill(theme.colors.document),
    }
}
pub fn index_panel() -> egui::Frame {
    egui::Frame::new()
        .fill(NAVIGATION())
        .inner_margin(metrics().navigation_margin as i8)
}

/// The original three layouts keep their geometry. Focus is borderless; Ledger is
/// a real side-label column that stacks above the content when space is insufficient.
pub(super) fn document_surface_titled<R>(
    ui: &mut egui::Ui,
    label: &str,
    max_width: f32,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let theme = resolved(ui.ctx());
    let geometry = document_geometry(&theme, ui.available_width(), max_width);
    // A short viewport reserves space for content before decorative vertical gaps.
    let height = ui.available_height();
    let constrained = height < 150.0;
    let inset_y = if constrained {
        geometry.inset_y.min(height * 0.06)
    } else {
        geometry.inset_y
    };
    let padding_y = if constrained {
        geometry.padding.y.min(height * 0.08)
    } else {
        geometry.padding.y
    };
    let origin = ui.cursor().min + egui::vec2(geometry.inset_x, inset_y);
    let trailing_space = if theme.preferences.style == StylePreset::Manuscript {
        12.0 + ui.spacing().item_spacing.y
    } else {
        0.0
    };
    let rect = egui::Rect::from_min_size(
        origin,
        egui::vec2(
            geometry.outer_width,
            (height - inset_y - trailing_space).max(0.0),
        ),
    );
    let margin = egui::Margin::symmetric(geometry.padding.x as i8, padding_y as i8);
    let response = ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.set_width(geometry.outer_width);
        egui::Frame::new()
            .fill(if theme.preferences.style == StylePreset::Focus {
                Color32::TRANSPARENT
            } else {
                theme.colors.document
            })
            .corner_radius(theme.shapes.document)
            .inner_margin(margin)
            .show(ui, |ui| {
                ui.set_width((geometry.outer_width - margin.sum().x).max(1.0));
                if theme.preferences.style == StylePreset::Ledger {
                    ledger_content(ui, label, &theme, &geometry, contents)
                } else {
                    if theme.preferences.style == StylePreset::Technical {
                        ui.visuals_mut().faint_bg_color = theme.colors.hover;
                    }
                    contents(ui)
                }
            })
            .inner
    });
    if theme.preferences.style == StylePreset::Manuscript {
        let rect = response.response.rect;
        ui.painter().line_segment(
            [rect.left_top(), rect.left_bottom()],
            Stroke::new(1.0_f32, theme.colors.subtle_border),
        );
        ui.add_space(12.0);
    }
    response
}
fn ledger_label(ui: &mut egui::Ui, label: &str, theme: &ResolvedTheme) {
    egui::Frame::new()
        .fill(theme.colors.chrome)
        .inner_margin(8)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.add(egui::Label::new(RichText::new(label).size(16.0).strong()).wrap());
        });
}
fn ledger_content<R>(
    ui: &mut egui::Ui,
    label: &str,
    theme: &ResolvedTheme,
    geometry: &DocumentGeometry,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    if geometry.rail_width > 0.0 {
        let start = ui.cursor().min;
        let row = ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = geometry.rail_gap;
            ui.vertical(|ui| {
                ui.set_width(geometry.rail_width);
                ledger_label(ui, label, theme);
            });
            ui.vertical(|ui| {
                ui.set_width(geometry.content_width);
                contents(ui)
            })
            .inner
        });
        let x = start.x + geometry.rail_width + geometry.rail_gap * 0.5;
        ui.painter().vline(
            x,
            start.y..=row.response.rect.bottom(),
            Stroke::new(1.0_f32, theme.colors.control_border),
        );
        row.inner
    } else {
        ledger_label(ui, label, theme);
        ui.add_space(8.0);
        contents(ui)
    }
}

pub fn panel_header(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    let theme = resolved(ui.ctx());
    match theme.preferences.style {
        StylePreset::Studio => {
            ui.label(
                RichText::new(title)
                    .size(HEADING_SIZE)
                    .strong()
                    .color(theme.colors.text),
            );
            if !subtitle.is_empty() {
                ui.add(egui::Label::new(muted(subtitle)).wrap());
            }
            ui.add_space(theme.metrics.section_gap);
        }
        StylePreset::Manuscript => {
            ui.add_space(8.0);
            ui.label(
                RichText::new(title)
                    .font(body_font(HEADING_SIZE + 2.0))
                    .strong(),
            );
            if !subtitle.is_empty() {
                ui.add(egui::Label::new(muted(subtitle)).wrap());
            }
            let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 12.0), egui::Sense::hover());
            ui.painter().line_segment(
                [rect.left_center(), rect.right_center()],
                Stroke::new(1.0_f32, theme.colors.control_border),
            );
            ui.add_space(8.0);
        }
        StylePreset::Technical => {
            egui::Frame::new()
                .fill(theme.colors.chrome)
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(RichText::new(title).size(16.0).strong());
                    if !subtitle.is_empty() {
                        ui.add(egui::Label::new(muted(subtitle)).wrap());
                    }
                });
            ui.separator();
        }
        StylePreset::Focus => {
            ui.vertical_centered(|ui| {
                ui.add(
                    egui::Label::new(RichText::new(title).font(body_font(HEADING_SIZE)).strong())
                        .wrap(),
                );
                if !subtitle.is_empty() {
                    ui.add(egui::Label::new(muted(subtitle)).wrap());
                }
            });
            ui.add_space(theme.metrics.section_gap);
        }
        StylePreset::Ledger => {
            let geometry = document_geometry(&theme, ui.available_width(), ui.available_width());
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(geometry.padding.x as i8, 0))
                .show(ui, |ui| {
                    ledger_content(ui, title, &theme, &geometry, |ui| {
                        if !subtitle.is_empty() {
                            ui.add(egui::Label::new(muted(subtitle)).wrap());
                        }
                    });
                });
            ui.add_space(theme.metrics.section_gap);
        }
    }
}
pub fn page_heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    panel_header(ui, title, subtitle);
}
pub fn toolbar<R>(ui: &mut egui::Ui, contents: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let result = ui
        .horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
            contents(ui)
        })
        .inner;
    ui.add_space(if style_preset() == StylePreset::Technical {
        4.0
    } else {
        8.0
    });
    result
}
/// Paint only persistent selection markers and focus; call after content without obscuring it.
pub fn selection_frame(ui: &egui::Ui, response: &egui::Response, selected: bool) {
    let theme = resolved(ui.ctx());
    if selected
        && matches!(
            theme.preferences.style,
            StylePreset::Technical | StylePreset::Ledger
        )
    {
        ui.painter().line_segment(
            [response.rect.left_top(), response.rect.left_bottom()],
            Stroke::new(3.0_f32, theme.colors.accent),
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.shrink(1.0),
            theme.shapes.row,
            Stroke::new(theme.focus_width, theme.colors.focus),
            egui::StrokeKind::Inside,
        );
    }
}
