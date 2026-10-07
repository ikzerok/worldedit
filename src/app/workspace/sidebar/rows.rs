use crate::app::Tab;
use crate::theme;
use egui::{pos2, vec2, Color32, FontId, Sense, Stroke};

pub(super) fn navigation_row(ui: &mut egui::Ui, tab: Tab, selected: bool) -> egui::Response {
    let height = theme::metrics()
        .row_height
        .max(ui.text_style_height(&egui::TextStyle::Body) + 10.0);
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            selected,
            tab.title(),
        )
    });
    if ui.is_rect_visible(rect) {
        let technical = theme::style_preset() == theme::StylePreset::Technical;
        let background = if selected {
            theme::SELECTION()
        } else if response.hovered() {
            theme::HOVER()
        } else {
            Color32::TRANSPARENT
        };
        ui.painter()
            .rect_filled(rect, theme::shapes().row, background);
        if selected {
            let marker = egui::Rect::from_min_size(
                rect.left_top() + vec2(0.0, if technical { 0.0 } else { 6.0 }),
                vec2(3.0, if technical { height } else { height - 12.0 }),
            );
            ui.painter().rect_filled(marker, 0, theme::ACCENT());
        }
        let icon_ink = if selected {
            theme::ACCENT()
        } else {
            theme::MUTED()
        };
        super::icons::paint(
            ui.painter(),
            rect.left_center() + vec2(18.0, 0.0),
            tab,
            icon_ink,
        );
        let mut job = egui::text::LayoutJob::simple(
            tab.title().into(),
            FontId::proportional(14.0),
            theme::TEXT(),
            (rect.width() - 42.0).max(1.0),
        );
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        let galley = ui.fonts(|fonts| fonts.layout_job(job));
        ui.painter().galley(
            pos2(rect.left() + 36.0, rect.center().y - galley.size().y * 0.5),
            galley,
            theme::TEXT(),
        );
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect.shrink(1.0),
                theme::shapes().row,
                Stroke::new(theme::focus_width(), theme::FOCUS()),
                egui::StrokeKind::Inside,
            );
        }
    }
    response.on_hover_text(tab.title())
}
