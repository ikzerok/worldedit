//! 菜单只记自身展开/滚动，不改正文或其它面板的滚动位置。
use super::*;

#[derive(Clone, Copy)]
struct MenuState {
    frame: u64,
    open: bool,
    popup_id: Option<egui::Id>,
    opened_frame: u64,
    reset_frame: Option<u64>,
}

pub(super) struct RootSession {
    key: egui::Id,
    state: MenuState,
    was_open: bool,
}
impl RootSession {
    pub(super) fn begin(ctx: &egui::Context, key: egui::Id) -> Self {
        let frame = ctx.cumulative_frame_nr();
        let previous = ctx.data(|data| data.get_temp::<MenuState>(key));
        let was_open = previous.is_some_and(|state| {
            state.open
                && state
                    .popup_id
                    .is_some_and(|id| egui::Popup::is_id_open(ctx, id))
                && (state.frame == frame || state.frame.checked_add(1) == Some(frame))
        });
        Self {
            key,
            was_open,
            state: MenuState {
                frame,
                open: was_open,
                popup_id: previous.and_then(|state| state.popup_id),
                opened_frame: previous
                    .filter(|_| was_open)
                    .map_or(frame, |state| state.opened_frame),
                reset_frame: previous.and_then(|state| state.reset_frame),
            },
        }
    }
    pub(super) fn policy(&self, subsequent: egui::PopupCloseBehavior) -> egui::PopupCloseBehavior {
        if self.state.opened_frame == self.state.frame {
            // 上帧 Area response 可仍存在；整个新开帧（含多 pass）不能把打开 click 又判为外点。
            egui::PopupCloseBehavior::IgnoreClicks
        } else {
            subsequent
        }
    }
    fn reset_scroll(&mut self, ctx: &egui::Context, id: egui::Id) {
        if !self.was_open && self.state.reset_frame != Some(self.state.frame) {
            egui::scroll_area::State::default().store(ctx, id);
            self.state.reset_frame = Some(self.state.frame);
        }
    }
    pub(super) fn finish(mut self, ctx: &egui::Context, response: &egui::Response) {
        let id = egui::Popup::default_response_id(response);
        self.state.popup_id = Some(id);
        self.state.open = egui::Popup::is_id_open(ctx, id);
        ctx.data_mut(|data| data.insert_temp(self.key, self.state));
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    ui: &mut egui::Ui,
    project: &Project,
    buffer: &mut WritingBuffer,
    target: &TargetRef,
    view: &mut ViewState,
    input_busy: bool,
    action: &mut Action,
    include_modes: bool,
) {
    let key = egui::Id::new((
        "writing-tools-menu",
        buffer.path(),
        &target.kind,
        &target.id,
        include_modes,
    ));
    let mut session = RootSession::begin(ui.ctx(), key);
    let menu = egui::containers::menu::MenuButton::new("正文工具")
        .config(
            egui::containers::menu::MenuConfig::default()
                .close_behavior(session.policy(egui::PopupCloseBehavior::CloseOnClickOutside)),
        )
        .ui(ui, |ui| {
            let discard_was_pending = view.discard_confirm.is_some();
            ui.set_max_width((ui.ctx().screen_rect().width() - 32.0).min(360.0));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            let scroll_key = key.with("scroll");
            let scroll_id = ui.make_persistent_id(egui::Id::new(scroll_key));
            session.reset_scroll(ui.ctx(), scroll_id);
            let output = egui::ScrollArea::vertical()
                .id_salt(scroll_key)
                .max_height(220.0)
                .show(ui, |ui| {
                    if include_modes {
                        draw_modes(ui, buffer, target, view, input_busy);
                    }
                    draw_actions(ui, buffer, target, view, input_busy, action);
                    dialogue::toolbar_menu(ui, buffer, target, view, action, input_busy);
                    draw_status(ui, project, buffer, target, view, input_busy, action, true);
                });
            debug_assert_eq!(output.id, scroll_id);
            #[cfg(test)]
            ui.ctx().data_mut(|data| {
                data.insert_temp(key.with("observed-scroll"), output.state.offset.y)
            });
            if view.has_mode_request(ui.ctx(), buffer, target)
                || action.apply
                || action.discard
                || action.comment
                || action.world_link
                || action.production
                || (discard_was_pending && view.discard_confirm.is_none())
            {
                ui.close();
            }
        });
    view.protect_toolbar_input(ui, &menu.0, buffer, target);
    // Popup::show 已处理显式关闭、Escape 与外部点击。
    session.finish(ui.ctx(), &menu.0);
}
