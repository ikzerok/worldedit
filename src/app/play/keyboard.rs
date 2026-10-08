//! 普通试玩的单次键盘确认与成功推进交接；不参与 runtime 决策。
use crate::app::{Tab, WorldeditApp};
use worldline_runtime::ContinuationOutcome;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Target {
    Choice(usize),
    Record,
}
#[derive(Clone, Copy)]
struct PendingFocus {
    source: egui::Id,
    frame: u64,
    target: Option<Target>,
}
#[derive(Default)]
pub(in crate::app) struct PlayKeyboard {
    session: u64,
    last_choice: Option<(Target, egui::Id)>,
    group: u64,
    context: Option<egui::Id>,
    pending: Option<PendingFocus>,
    latch: bool,
    ime: bool,
    navigation: bool,
    confirm: bool,
    fresh: bool,
    prepared: Option<u64>,
    tab_frame: Option<u64>,
}
#[derive(Clone, Copy)]
pub(super) struct Activation {
    source: Option<egui::Id>,
}

impl PlayKeyboard {
    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }

    pub(in crate::app) fn new_session(&mut self) {
        self.cancel();
        self.session = self.session.wrapping_add(1);
        self.group = 0;
        self.last_choice = None;
    }

    pub(super) fn session(&self) -> u64 {
        self.session
    }

    fn prepare(&mut self, ctx: &egui::Context, context: egui::Id, ime: bool, blocked: bool) {
        if self.context != Some(context) || blocked {
            self.cancel();
        }
        self.context = Some(context);
        let frame = ctx.cumulative_frame_nr();
        if self.prepared == Some(frame) {
            return;
        }
        self.prepared = Some(frame);
        self.ime = ime;
        self.navigation = false;
        self.confirm = false;
        self.fresh = false;
        ctx.input(|input| {
            for event in &input.raw.events {
                match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat,
                        modifiers,
                        ..
                    } => {
                        if matches!(key, egui::Key::Enter | egui::Key::Space) {
                            self.confirm = true;
                            self.fresh |= !repeat && modifiers.is_none();
                            self.navigation |= !modifiers.is_none();
                        } else {
                            self.navigation = true;
                            if *key == egui::Key::Tab {
                                self.tab_frame = Some(frame);
                            }
                        }
                    }
                    egui::Event::Ime(_) => self.ime = true,
                    egui::Event::PointerButton { .. }
                    | egui::Event::MouseWheel { .. }
                    | egui::Event::Touch { .. }
                    | egui::Event::WindowFocused(false) => self.navigation = true,
                    _ => {}
                }
            }
            self.navigation |= !input.focused;
            // 释放帧不能包含另一次按下；同帧按下/释放仍只允许原按钮行动。
            if !input.key_down(egui::Key::Enter)
                && !input.key_down(egui::Key::Space)
                && !self.confirm
            {
                self.latch = false;
            }
        });
        if self.ime {
            self.latch |= self.confirm;
        }
        let different_focus = self.pending.is_some_and(|pending| {
            ctx.memory(|memory| memory.focused())
                .is_some_and(|id| id != pending.source)
        });
        if self.ime || self.navigation || different_focus {
            self.cancel();
        }
    }

    pub(super) fn activation(&mut self, response: &egui::Response) -> Option<Activation> {
        if !response.clicked() || self.ime {
            return None;
        }
        if response.clicked_by(egui::PointerButton::Primary) {
            return Some(Activation { source: None });
        }
        if self.confirm {
            if self.latch || !self.fresh || self.navigation || !response.has_focus() {
                return None;
            }
            self.latch = true;
            return Some(Activation {
                source: Some(response.id),
            });
        }
        // 保留辅助技术的显式点击，但不把它冒充键盘焦点交接。
        Some(Activation { source: None })
    }

    pub(super) fn advanced(&mut self, ctx: &egui::Context, activation: Activation) {
        self.group = self.group.wrapping_add(1);
        self.pending = activation.source.map(|source| PendingFocus {
            source,
            frame: ctx.cumulative_frame_nr(),
            target: None,
        });
        ctx.request_repaint();
    }

    pub(super) fn settled(
        &mut self,
        ctx: &egui::Context,
        outcome: ContinuationOutcome,
        first: Option<usize>,
    ) {
        let Some(pending) = &mut self.pending else {
            return;
        };
        let was_waiting = pending.target.is_none();
        match outcome {
            ContinuationOutcome::Choice => pending.target = first.map(Target::Choice),
            ContinuationOutcome::Ended => pending.target = Some(Target::Record),
            _ => self.cancel(),
        }
        if was_waiting && self.pending.is_some_and(|p| p.target.is_some()) {
            ctx.request_repaint();
        }
        if outcome == ContinuationOutcome::Choice && first.is_none() {
            self.cancel();
        }
    }

    pub(super) fn restore_after_inspection(&mut self, ctx: &egui::Context) {
        if let Some((target, source)) = self.last_choice {
            ctx.memory_mut(|memory| memory.request_focus(source));
            self.pending = Some(PendingFocus {
                source,
                frame: ctx.cumulative_frame_nr(),
                target: Some(target),
            });
            self.latch |= ctx.input(|input| {
                input.key_down(egui::Key::Enter) || input.key_down(egui::Key::Space)
            });
            ctx.request_repaint();
        }
    }

    pub(super) fn wants_record_focus(&self) -> bool {
        self.pending
            .is_some_and(|p| p.target == Some(Target::Record))
    }

    pub(super) fn button(
        &mut self,
        ui: &mut egui::Ui,
        role: &str,
        target: Option<Target>,
        enabled: bool,
        text: impl Into<egui::WidgetText>,
    ) -> egui::Response {
        let id = egui::Id::new(("ordinary-play", self.session, self.group, role, target));
        // push_id 仍让按钮自动 ID 依赖父 UI 的位置；独立语义 Ui 避免警告、
        // 条件原因或关键词链接增减后复用旧按钮身份。
        let mut builder = egui::UiBuilder::new()
            .layer_id(ui.layer_id())
            .max_rect(ui.available_rect_before_wrap())
            .layout(*ui.layout())
            .style(ui.style().clone());
        if !ui.is_enabled() {
            builder = builder.disabled();
        }
        if !ui.is_visible() {
            builder = builder.invisible();
        }
        let mut child = egui::Ui::new(ui.ctx().clone(), id, builder);
        child.set_clip_rect(ui.clip_rect());
        let response =
            crate::theme::add_enabled(&mut child, enabled, egui::Button::new(text).wrap());
        ui.advance_cursor_after_rect(child.min_rect());
        if let Some(target @ Target::Choice(_)) = target {
            if response.has_focus() || response.clicked() {
                self.last_choice = Some((target, response.id));
            }
        }
        let frame = ui.ctx().cumulative_frame_nr();
        let restore = self.pending.is_some_and(|pending| {
            target.is_some() && pending.target == target && pending.frame < frame
        });
        if restore && !self.latch && !self.ime && !self.navigation {
            self.cancel();
            if response.enabled() {
                response.request_focus();
                response.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
                ui.ctx().request_repaint();
            }
        } else if self
            .tab_frame
            .is_some_and(|last| frame.saturating_sub(last) <= 1)
            && response.gained_focus()
            && !ui.clip_rect().contains_rect(response.rect)
        {
            response.scroll_to_me_animation(None, egui::style::ScrollAnimation::none());
        }
        response
    }
}

impl WorldeditApp {
    pub(in crate::app) fn prepare_play_keyboard(&mut self, ctx: &egui::Context) {
        let context = egui::Id::new((&self.project.root, self.version));
        let blocked = self.tab != Tab::Play
            || self.comparison.active
            || self.command_palette.open
            || self.play_confirmation.is_some()
            || self.playthrough_report.open
            || self.replay_debugger.inspection.open
            || self.has_open_authoring_form()
            || self
                .play
                .as_ref()
                .is_some_and(|play| play.paused || play.stopped || play.error.is_some());
        self.play_keyboard.prepare(
            ctx,
            context,
            self.ime_composing || self.command_palette.ime || self.command_palette.ime_frame,
            blocked,
        );
    }

    pub(super) fn start_play_activated(&mut self, ctx: &egui::Context, activation: Activation) {
        let session = self.play_keyboard.session();
        self.play_keyboard.cancel();
        self.start_play();
        if self.play_keyboard.session() != session
            && self.play_confirmation.is_none()
            && self.play.as_ref().is_some_and(|play| play.error.is_none())
        {
            self.play_keyboard.advanced(ctx, activation);
        }
    }
}

#[cfg(test)]
mod tests;
