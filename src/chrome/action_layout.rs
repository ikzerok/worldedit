//! 工具条共用真实字体、控件内边距与命中尺寸预算；不按字符数猜宽度。
use crate::theme;

pub(crate) fn text_width(ui: &egui::Ui, text: &str, style: egui::TextStyle) -> f32 {
    let font = style.resolve(ui.style());
    ui.fonts(|fonts| {
        fonts
            .layout_no_wrap(text.to_owned(), font, theme::TEXT())
            .size()
            .x
    })
}

pub(crate) fn button_width(ui: &egui::Ui, label: &str) -> f32 {
    (text_width(ui, label, egui::TextStyle::Button) + ui.spacing().button_padding.x * 2.0)
        .max(ui.spacing().interact_size.x)
}

pub(crate) fn checkbox_width(ui: &egui::Ui, label: &str) -> f32 {
    text_width(ui, label, egui::TextStyle::Button)
        + ui.spacing().icon_width
        + ui.spacing().icon_spacing
        + ui.spacing().button_padding.x * 2.0
}

pub(crate) fn row_width(widths: &[f32], gap: f32) -> f32 {
    widths.iter().sum::<f32>() + widths.len().saturating_sub(1) as f32 * gap
}

/// 弹出动作组的键盘焦点必须显露；测试另记真实矩形，正式构建不留测试状态。
pub(crate) fn record(ui: &egui::Ui, role: &str, response: &egui::Response) {
    if keyboard_navigation_active(ui.ctx())
        && response.has_focus()
        && !ui
            .clip_rect()
            .contains_rect(response.rect.expand(theme::focus_width()))
    {
        response.scroll_to_me(None);
        ui.ctx().request_repaint();
    }
    #[cfg(test)]
    ui.ctx().data_mut(|data| {
        let records = data.get_temp_mut_or_default::<Vec<ActionRect>>(trace_id());
        records.push(ActionRect {
            role: role.to_owned(),
            rect: response.rect,
            clip: ui.clip_rect(),
            id: response.id,
            enabled: response.enabled(),
        });
    });
    #[cfg(not(test))]
    let _ = (ui, role, response);
}

/// Tab 的同次/下一帧显露许可；新的指针、文字或 IME 意图立即撤销。
pub(crate) fn keyboard_navigation_active(ctx: &egui::Context) -> bool {
    let key = egui::Id::new("action-layout-keyboard-navigation");
    let frame = ctx.cumulative_frame_nr();
    let (tab, newer) = ctx.input(|input| {
        let tab = input.events.iter().any(|event| matches!(event,
            egui::Event::Key { key: egui::Key::Tab, pressed: true, modifiers, .. }
                if *modifiers == egui::Modifiers::NONE || *modifiers == egui::Modifiers::SHIFT));
        let newer = input.events.iter().any(|event| {
            matches!(event, egui::Event::PointerButton { .. } | egui::Event::MouseWheel { .. }
                | egui::Event::Touch { .. } | egui::Event::Ime(_) | egui::Event::Text(_)
                | egui::Event::Paste(_) | egui::Event::WindowFocused(false))
                || matches!(event, egui::Event::Key { key, pressed: true, .. } if *key != egui::Key::Tab)
        });
        (tab, newer)
    });
    ctx.data_mut(|data| {
        if newer {
            data.remove::<u64>(key);
            return false;
        }
        if tab {
            data.insert_temp(key, frame);
        }
        data.get_temp::<u64>(key)
            .is_some_and(|started| started <= frame && frame - started <= 1)
    })
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct ActionRect {
    pub role: String,
    pub rect: egui::Rect,
    pub clip: egui::Rect,
    pub id: egui::Id,
    pub enabled: bool,
}

#[cfg(test)]
fn trace_id() -> egui::Id {
    egui::Id::new("worldedit-action-bar-response-trace")
}

#[cfg(test)]
pub(crate) fn clear_trace(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove::<Vec<ActionRect>>(trace_id()));
}

#[cfg(test)]
pub(crate) fn trace(ctx: &egui::Context) -> Vec<ActionRect> {
    ctx.data(|data| data.get_temp::<Vec<ActionRect>>(trace_id()))
        .unwrap_or_default()
}
