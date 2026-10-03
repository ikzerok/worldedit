//! 一个当前来源锚点与温和背景；按真实galley行绘制，不占用作者文本选区。
use crate::theme;
use std::ops::Range;

pub(super) fn shapes(
    galley: &egui::Galley,
    origin: egui::Pos2,
    gutter: egui::Rect,
    clip: egui::Rect,
    range: Range<usize>,
) -> Vec<egui::Shape> {
    if range.start > range.end || range.end > galley.end().index {
        return Vec::new();
    }
    let start = galley.layout_from_cursor(egui::text::CCursor { index: range.start, prefer_next_row: true });
    let end = galley.layout_from_cursor(egui::text::CCursor { index: range.end, prefer_next_row: false });
    let mut result = Vec::new();
    let first_visible = galley.rows.partition_point(|row| origin.y + row.max_y() < clip.top());
    let last_visible = galley.rows.partition_point(|row| origin.y + row.min_y() <= clip.bottom());
    for index in start.row.max(first_visible)..=end.row.min(last_visible.saturating_sub(1)) {
        let Some(row) = galley.rows.get(index) else { break };
        let y = origin.y + row.min_y();
        let bottom = origin.y + row.max_y();
        if bottom < clip.top() || y > clip.bottom() { continue; }
        let left = if index == start.row { row.x_offset(start.column) } else { row.pos.x };
        let right = if index == end.row { row.x_offset(end.column) } else { row.rect().right() };
        let rect = egui::Rect::from_min_max(
            egui::pos2(origin.x + left, y), egui::pos2(origin.x + right.max(left + 2.), bottom),
        ).intersect(clip);
        if rect.is_positive() {
            result.push(egui::Shape::rect_filled(rect, 2., theme::problem_source_background()));
            result.push(egui::Shape::line_segment(
                [rect.left_bottom(), rect.right_bottom()], egui::Stroke::new(1_f32, theme::ACCENT()),
            ));
        }
        if index == start.row {
            let anchor = egui::Rect::from_min_max(
                egui::pos2(gutter.right() + 3., y + 2.), egui::pos2(gutter.right() + 6., bottom - 2.),
            ).intersect(clip);
            if anchor.is_positive() {
                result.push(egui::Shape::rect_filled(anchor, 1., theme::ACCENT()));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_follows_actual_wrapped_rows_and_preserves_source_and_selection() {
        for font in [16., 28.] {
            for wrap in [false, true] {
                let ctx = egui::Context::default();
                let text = format!("\r\n  {}目标👩‍🚀é suffix\r\n", "长中文".repeat(40));
                let start = text[..text.find("目标").unwrap()].chars().count();
                let range = start..start + "目标👩‍🚀é".chars().count();
                let _ = ctx.run(Default::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let job = super::super::layout::job(&text, font, font * 1.7,
                            worldline_core::LanguageVersion::V1_13, wrap, 240.);
                        let galley = ui.fonts(|fonts| fonts.layout_job(job));
                        let origin = egui::pos2(60., 10.);
                        let clip = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(10000., 10000.));
                        let painted = shapes(&galley, origin, egui::Rect::from_min_size(egui::pos2(20., 0.), egui::vec2(20., 0.)), clip, range.clone());
                        let expected = galley.pos_from_cursor(egui::text::CCursor { index: range.start, prefer_next_row: true });
                        assert!(painted.iter().any(|shape| matches!(shape, egui::Shape::Rect(rect)
                            if (rect.rect.top() - (origin.y + expected.top())).abs() < 0.5)));
                        assert_eq!(galley.job.text, text);
                        assert!(shapes(&galley, origin, clip, clip, 0..usize::MAX).is_empty());
                    });
                });
            }
        }
    }
}
