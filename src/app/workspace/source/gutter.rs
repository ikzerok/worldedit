//! 行号跟随正文实际排版；物理行由换行符定义，不能以固定行高推算。
use crate::theme::MUTED;

pub(super) fn reserve(ui: &mut egui::Ui, text: &str) -> egui::Rect {
    let last = text.split('\n').count().to_string();
    let width = ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(last, egui::FontId::monospace(14.0), MUTED())
            .size()
            .x
    });
    ui.allocate_exact_size(egui::vec2(width, 0.0), egui::Sense::hover())
        .0
}

pub(super) fn paint(ui: &egui::Ui, gutter: egui::Rect, galley: &egui::Galley, origin: egui::Pos2) {
    let mut physical_line = 1;
    let mut starts_line = true;
    for row in &galley.rows {
        if starts_line {
            let y = origin.y + row.rect().center().y;
            if y >= ui.clip_rect().top() && y <= ui.clip_rect().bottom() {
                ui.painter().text(
                    egui::pos2(gutter.right(), y),
                    egui::Align2::RIGHT_CENTER,
                    physical_line.to_string(),
                    egui::FontId::monospace(14.0),
                    MUTED(),
                );
            }
        }
        starts_line = row.ends_with_newline;
        if starts_line {
            physical_line += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_rows_do_not_create_extra_physical_line_numbers() {
        for size in [11.0, 22.0] {
            let ctx = egui::Context::default();
            let text = "ASCII 中文 repeated repeated repeated\n\nlast line\n";
            let mut row_count = 0;
            let mut numbers = Vec::new();
            let output = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut job = crate::highlight::layout_job(
                        text,
                        size,
                        worldline_core::LanguageVersion::V1_10,
                    );
                    job.wrap.max_width = 60.0;
                    let galley = ui.fonts(|fonts| fonts.layout_job(job));
                    row_count = galley.rows.len();
                    let gutter = reserve(ui, text);
                    paint(ui, gutter, &galley, egui::pos2(40.0, 20.0));
                });
            });
            for shape in output.shapes {
                if let egui::Shape::Text(shape) = shape.shape {
                    numbers.push(shape.galley.job.text.clone());
                }
            }
            assert!(row_count > 4, "必须确实发生视觉折行");
            assert_eq!(numbers, ["1", "2", "3", "4"]);
        }
    }
}
