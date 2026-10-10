//! 根据当前正文列和实际字体预算逐级收拢操作，不让工具栏挤掉正文。
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Layout {
    Full,
    Folded,
    Modes,
    Compact,
}

pub(super) fn heading_width(ui: &egui::Ui, title: &str, compact: bool) -> f32 {
    let text = if compact { title } else { "正文" };
    let width = text_width(ui, text, egui::TextStyle::Heading);
    if compact {
        width.min((ui.available_width() * 0.25).clamp(72.0, 180.0))
    } else {
        width
    }
}

pub(super) fn choose(ui: &egui::Ui, heading: f32, mode: Mode) -> Layout {
    if ui.ctx().screen_rect().width() < 600.0 || ui.ctx().screen_rect().height() < 420.0 {
        return Layout::Compact;
    }
    let spacing = ui.spacing();
    let button = |label: &str| {
        (text_width(ui, label, egui::TextStyle::Button) + spacing.button_padding.x * 2.0)
            .max(spacing.interact_size.x)
            + spacing.item_spacing.x
    };
    let modes = heading
        + spacing.item_spacing.x
        + ["写作", "结构", "源码"]
            .into_iter()
            .map(button)
            .sum::<f32>();
    let primary = modes
        + [apply_label(mode), "丢弃此文件草稿"]
            .into_iter()
            .map(button)
            .sum::<f32>();
    let checkbox = text_width(ui, "逐句对白", egui::TextStyle::Button)
        + spacing.icon_width
        + spacing.icon_spacing
        + spacing.button_padding.x * 2.0
        + spacing.item_spacing.x;
    let full = primary + button("插入正式台词") + checkbox + button("角色台本");
    // 一个额外间距吸收像素取整，避免恰好贴边时由 horizontal_wrapped 再开一行。
    let available = ui.available_width() - spacing.item_spacing.x;
    if full <= available {
        Layout::Full
    } else if primary + button("对白工具") <= available {
        Layout::Folded
    } else if modes + button("正文工具") <= available {
        Layout::Modes
    } else {
        Layout::Compact
    }
}

pub(super) fn apply_label(mode: Mode) -> &'static str {
    if mode == Mode::Source {
        "应用源码草稿（可含诊断）"
    } else {
        "应用正文草稿"
    }
}

fn text_width(ui: &egui::Ui, text: &str, style: egui::TextStyle) -> f32 {
    let font = style.resolve(ui.style());
    ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(text.to_owned(), font, theme::TEXT())
            .size()
            .x
    })
}
