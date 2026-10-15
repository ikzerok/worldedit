//! 只显露本次键盘导航，不能持续追随旧焦点而抵消新的滚轮或 IME 意图。
use crate::app::writing_workspace::preview_navigation as shared;

#[derive(Clone, Copy)]
struct Navigation {
    frame: u64,
    from: Option<egui::Id>,
    requested: Option<egui::Id>,
}
fn key() -> egui::Id {
    egui::Id::new("condition-authoring-keyboard-focus")
}
pub(in crate::app) fn begin(ctx: &egui::Context, enabled: bool) {
    let frame = ctx.cumulative_frame_nr();
    let (tab, newer) = ctx.input(|input| {
        let tab = input.events.iter().any(|event| matches!(event,
            egui::Event::Key { key: egui::Key::Tab, pressed: true, modifiers, .. }
                if *modifiers == egui::Modifiers::NONE || *modifiers == egui::Modifiers::SHIFT));
        let newer = !input.focused || input.events.iter().any(|event| {
            matches!(event, egui::Event::PointerButton { .. } | egui::Event::MouseWheel { .. }
                | egui::Event::Touch { .. } | egui::Event::Text(_) | egui::Event::Paste(_)
                | egui::Event::WindowFocused(false)
                | egui::Event::Ime(egui::ImeEvent::Commit(_)))
                || matches!(event, egui::Event::Ime(egui::ImeEvent::Preedit(text)) if !text.is_empty())
                || matches!(event, egui::Event::Key { key, pressed: true, .. } if *key != egui::Key::Tab)
        });
        (tab, newer)
    });
    // 本表单已用真实跨帧组合态决定 enabled；空闲 Enabled/Disabled 不取消同次 Tab。
    if !enabled || newer {
        ctx.data_mut(|data| data.remove::<Navigation>(key()));
        return;
    }
    if tab {
        let from = ctx.memory(|memory| memory.focused());
        ctx.data_mut(|data| {
            if !data
                .get_temp::<Navigation>(key())
                .is_some_and(|n| n.frame == frame)
            {
                data.insert_temp(
                    key(),
                    Navigation {
                        frame,
                        from,
                        requested: None,
                    },
                );
            }
        });
    }
}
pub(in crate::app) fn finish(ui: &egui::Ui, enabled: bool) {
    if !enabled || !shared::available(ui.ctx()) {
        return;
    }
    let Some(id) = ui.ctx().memory(|memory| memory.focused()) else {
        return;
    };
    let owned = ui.ctx().viewport(|viewport| {
        viewport.this_pass.widgets.get(id).is_some_and(|widget| {
            widget.enabled
                && widget.layer_id == ui.layer_id()
                && ui.min_rect().contains(widget.rect.center())
        })
    });
    if !owned {
        return;
    }
    let Some(response) = ui.ctx().read_response(id) else {
        return;
    };
    if egui::TextEdit::load_state(ui.ctx(), id).is_none() {
        shared::outline(ui, response.rect.expand(crate::theme::focus_width()));
    }
    let Some(mut navigation) = ui.ctx().data(|data| data.get_temp::<Navigation>(key())) else {
        return;
    };
    let frame = ui.ctx().cumulative_frame_nr();
    if navigation.frame <= frame
        && frame - navigation.frame <= 1
        && navigation.from != Some(id)
        && navigation.requested != Some(id)
        && !ui
            .clip_rect()
            .contains_rect(response.rect.expand(crate::theme::focus_width()))
    {
        navigation.requested = Some(id);
        ui.ctx()
            .data_mut(|data| data.insert_temp(key(), navigation));
        ui.scroll_to_rect_animation(
            response.rect.expand(crate::theme::focus_width()),
            None,
            egui::style::ScrollAnimation::none(),
        );
    }
}
