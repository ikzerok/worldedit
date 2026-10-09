//! 只对同一完整字符串分页切片；不复制8MiB到每帧TextEdit，也不改变交付字节。
use crate::theme;
use std::ops::Range;
const PAGE_BYTES: usize = 16 * 1024;

fn window(text: &str, page: usize) -> Range<usize> {
    let page = page.min(text.len().saturating_sub(1) / PAGE_BYTES);
    let boundary = |mut offset: usize| {
        offset = offset.min(text.len());
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    };
    boundary(page * PAGE_BYTES)..boundary((page + 1).saturating_mul(PAGE_BYTES))
}

fn controls(ui: &mut egui::Ui, text: &str, page: &mut usize) -> Range<usize> {
    let total = text.len().div_ceil(PAGE_BYTES).max(1);
    *page = (*page).min(total - 1);
    ui.horizontal_wrapped(|ui| {
        if theme::add_enabled(ui, *page > 0, egui::Button::new("上一段原文")).clicked() {
            *page = (*page).saturating_sub(1);
        }
        if theme::add_enabled(ui, *page + 1 < total, egui::Button::new("下一段原文")).clicked()
        {
            *page += 1;
        }
        ui.label(format!("完整原文第 {} / {} 段", *page + 1, total));
    });
    let range = window(text, *page);
    ui.add(
        egui::Label::new(theme::muted(format!(
            "UTF-8字节 {}..{} / {}；按原字节连续分页，复制/导出始终使用完整原文",
            range.start,
            range.end,
            text.len()
        )))
        .wrap(),
    );
    range
}

fn fragment(ui: &mut egui::Ui, text: &str, range: Range<usize>) {
    let mut fragment = &text[range];
    ui.add(
        egui::TextEdit::multiline(&mut fragment)
            .font(egui::TextStyle::Monospace)
            .desired_width(ui.available_width())
            .interactive(false),
    );
}

pub(super) fn draw(ui: &mut egui::Ui, text: &str, snapshot: &str, page: &mut usize) {
    let range = controls(ui, text, page);
    egui::ScrollArea::both()
        .id_salt(("delivery-markdown", snapshot, *page))
        .min_scrolled_height(0.0)
        .show(ui, |ui| {
            fragment(ui, text, range);
        });
}

pub(super) fn draw_compact(ui: &mut egui::Ui, text: &str, snapshot: &str, page: &mut usize) {
    // 分段/字节说明也在同一个内容流里；极短视口仍可滚到实际原文。
    let output = egui::ScrollArea::both()
        .id_salt(("compact-delivery-markdown", snapshot))
        .max_height(ui.available_height().max(0.0))
        .min_scrolled_height(0.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let range = controls(ui, text, page);
            fragment(ui, text, range);
        });
    #[cfg(test)]
    ui.ctx().data_mut(|data| {
        data.insert_temp(
            egui::Id::new("delivery-markdown-viewport"),
            output.inner_rect,
        )
    });
    #[cfg(not(test))]
    let _ = output;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf8_preview_segments_concatenate_to_exact_unmodified_full_artifact() {
        for text in [
            String::new(),
            "中文😀长行\r\n<>".repeat(9000),
            "😀".repeat(PAGE_BYTES),
            "x".repeat(PAGE_BYTES),
        ] {
            let mut bytes = Vec::new();
            for page in 0..text.len().div_ceil(PAGE_BYTES).max(1) {
                let range = window(&text, page);
                assert!(text.is_char_boundary(range.start) && text.is_char_boundary(range.end));
                assert!(range.len() <= PAGE_BYTES + 3);
                bytes.extend_from_slice(&text.as_bytes()[range]);
            }
            assert_eq!(bytes, text.as_bytes());
            assert!(window(&text, usize::MAX).end <= text.len());
        }
    }
}
