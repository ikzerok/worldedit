//! 仅键盘焦点转移请求滚入；反向 Tab 可在紧邻下一帧才交付焦点。
#[derive(Clone, Copy)]
pub(super) struct FocusReveal {
    allowed: bool,
}

impl FocusReveal {
    pub(super) fn for_frame(ctx: &egui::Context, restoring: bool) -> Self {
        let stamp = egui::Id::new("route-comparison-keyboard-focus-frame");
        let frame = ctx.cumulative_frame_nr();
        let (tab, pointer) = ctx.input(|input| {
            (
                input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Key {
                            key: egui::Key::Tab,
                            pressed: true,
                            ..
                        }
                    )
                }),
                input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::PointerButton { .. } | egui::Event::MouseWheel { .. }
                    )
                }),
            )
        });
        if restoring || pointer {
            ctx.data_mut(|data| data.remove::<u64>(stamp));
        } else if tab {
            ctx.data_mut(|data| data.insert_temp(stamp, frame));
        }
        let recent = ctx
            .data(|data| data.get_temp::<u64>(stamp))
            .is_some_and(|last| last <= frame && frame - last <= 1);
        Self {
            allowed: recent && !restoring && !pointer,
        }
    }

    pub(super) fn reveal(self, ui: &egui::Ui, response: &egui::Response) {
        if self.allowed && response.gained_focus() && !ui.clip_rect().contains_rect(response.rect) {
            // None 只移动到足够可见，不把已经可见的控件强制放在中央。
            response.scroll_to_me(None);
        }
    }
}

/// 在明确返回意图发生时记录交接帧，不依赖帧末按键是否仍按住。
pub(super) fn queue_restore(ctx: &egui::Context, id: Option<egui::Id>) {
    let stamp = egui::Id::new("route-comparison-return-focus-frame");
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|data| {
        data.remove::<(egui::Id, u64)>(stamp);
        if let Some(id) = id {
            data.insert_temp(stamp, (id, frame));
        }
    });
}

/// 跨过返回帧的 end_pass 空间导航后，只完成一次明确的焦点交接。
pub(super) fn restore(ctx: &egui::Context, pending: &mut Option<egui::Id>) {
    let stamp = egui::Id::new("route-comparison-return-focus-frame");
    let Some(id) = *pending else {
        ctx.data_mut(|data| data.remove::<(egui::Id, u64)>(stamp));
        return;
    };
    let requested = ctx.data(|data| data.get_temp::<(egui::Id, u64)>(stamp));
    if requested == Some((id, ctx.cumulative_frame_nr())) {
        ctx.request_repaint();
        return;
    }
    ctx.data_mut(|data| data.remove::<(egui::Id, u64)>(stamp));
    ctx.memory_mut(|memory| memory.request_focus(id));
    *pending = None;
}
