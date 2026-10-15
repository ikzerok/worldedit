//! 复用对象资料窗的临时阅读域；不把 Tab 分派给底层作者表单。
use crate::app::{writing_workspace::preview_navigation as shared, WorldeditApp};
pub(super) fn layer() -> egui::LayerId {
    egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("object-state-inspection"),
    )
}
pub(super) fn reveal(
    ui: &egui::Ui,
    reader: Option<egui::Id>,
    outside: &[egui::Id],
    active: bool,
    navigate: bool,
) {
    if !active {
        return;
    }
    let focused = ui
        .ctx()
        .memory(|memory| memory.focused())
        .filter(|id| {
            !outside.contains(id)
                && ui.ctx().viewport(|viewport| {
                    viewport
                        .this_pass
                        .widgets
                        .get(*id)
                        .is_some_and(|widget| widget.layer_id == ui.layer_id())
                })
        })
        .and_then(|id| ui.ctx().read_response(id));
    if let Some(response) = focused {
        if reader != Some(response.id) {
            let outline = egui::TextEdit::load_state(ui.ctx(), response.id).is_none();
            if navigate {
                // 只执行本次 Tab 的显露；新滚轮/指针/IME 意图立即撤销许可。
                shared::reveal(ui, &response, true, outline);
            } else if outline && response.enabled() {
                shared::outline(ui, response.rect.expand(crate::theme::focus_width()));
            }
        }
    }
}
pub(super) fn finish_closed(app: &mut WorldeditApp, ctx: &egui::Context) {
    shared::clear_transient_host(ctx, layer());
    let state = &mut app.replay_debugger.object_inspection;
    // 明确来源跳转由 author_back 管理；这里只恢复窗口关闭的真实 opener。
    if state.return_focus_pending {
        state.return_focus_pending = false;
        if let Some(id) = state.return_focus {
            let present = ctx.viewport(|viewport| {
                viewport
                    .this_pass
                    .widgets
                    .get(id)
                    .is_some_and(|widget| widget.enabled)
            });
            if present {
                ctx.memory_mut(|memory| memory.request_focus(id));
            }
        }
    }
}

/// 仅此运行菜单的真实按钮域。窗口的 Tab 锁必须交给弹层，关闭时交回入口。
#[derive(Default)]
pub(super) struct RunMenu {
    popup: Option<egui::Id>,
    owners: Vec<egui::Id>,
    frame: u64,
}
impl RunMenu {
    pub(super) fn advance(&self, ctx: &egui::Context, active: bool) {
        if !active
            || self.frame.saturating_add(1) < ctx.cumulative_frame_nr()
            || !self
                .popup
                .is_some_and(|id| egui::Popup::is_id_open(ctx, id))
        {
            return;
        }
        let Some(at) = ctx
            .memory(|memory| memory.focused())
            .and_then(|id| self.owners.iter().position(|owner| *owner == id))
        else {
            return;
        };
        let modifiers = ctx.input(|input| input.modifiers);
        if (modifiers == egui::Modifiers::NONE || modifiers == egui::Modifiers::SHIFT)
            && ctx.input_mut(|input| input.consume_key(modifiers, egui::Key::Tab))
        {
            let step = if modifiers.shift {
                self.owners.len() - 1
            } else {
                1
            };
            ctx.memory_mut(|memory| {
                memory.request_focus(self.owners[(at + step) % self.owners.len()]);
            });
        }
    }
    pub(super) fn finish(
        &mut self,
        launcher: &egui::Response,
        items: Option<&[egui::Response]>,
        active: bool,
    ) {
        let ctx = &launcher.ctx;
        let popup = egui::Popup::default_response_id(launcher);
        let open = egui::Popup::is_id_open(ctx, popup);
        let previous_owner = ctx.memory(|memory| {
            self.owners
                .iter()
                .any(|id| memory.has_focus(*id) || memory.had_focus_last_frame(*id))
        });
        if !open {
            let pointer_intent = ctx.input(|input| {
                input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::PointerButton { pressed: true, .. }
                            | egui::Event::Touch { .. }
                    )
                })
            });
            if active && previous_owner && !pointer_intent {
                launcher.request_focus();
            }
            self.owners.clear();
            self.popup = None;
            return;
        }
        self.popup = Some(popup);
        self.frame = ctx.cumulative_frame_nr();
        let Some(items) = items else {
            return;
        };
        self.owners = items.iter().filter(|r| r.enabled()).map(|r| r.id).collect();
        if !active {
            return;
        }
        if launcher.has_focus() {
            if let Some(first) = self.owners.first() {
                ctx.memory_mut(|memory| memory.request_focus(*first));
            }
        }
        if let Some(response) = items.iter().find(|r| r.enabled() && r.has_focus()) {
            ctx.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    response.id,
                    egui::EventFilter {
                        tab: true,
                        // Escape belongs to Popup; the underlying object window stays open.
                        ..Default::default()
                    },
                )
            });
            let theme = crate::theme::resolved(ctx);
            ctx.layer_painter(response.layer_id)
                .with_clip_rect(ctx.screen_rect())
                .rect_stroke(
                    response.rect.expand(theme.focus_width),
                    2.0,
                    egui::Stroke::new(theme.focus_width, theme.colors.focus),
                    egui::StrokeKind::Inside,
                );
        }
    }
}
