//! IME 会话启停不是组合；真实组合期间保留输入控件与被阻止的鼠标手势。
use super::ViewState;

#[derive(Default)]
pub(super) struct Composition {
    frame: Option<u64>,
    active: bool,
    blocked: bool,
    composing_frame: bool,
    committed_frame: bool,
    gesture: bool,
    owner: Option<egui::Id>,
}

impl Composition {
    fn observe(&mut self, ctx: &egui::Context) {
        let frame = ctx.cumulative_frame_nr();
        if self.frame == Some(frame) {
            return;
        }
        self.frame = Some(frame);
        self.committed_frame = false;
        // 上层窗口持有焦点时，其 IME 只能由该窗口处理；不抢回下层正文。
        if !super::input_registry::owns_focus(ctx) {
            self.active = false;
            self.blocked = false;
            self.composing_frame = false;
            self.gesture = false;
            self.owner = None;
            return;
        }
        if !self.active && !self.gesture {
            self.owner = None;
        }
        self.blocked = self.active;
        ctx.input(|input| {
            for event in &input.events {
                let egui::Event::Ime(event) = event else {
                    continue;
                };
                match event {
                    egui::ImeEvent::Enabled => {}
                    egui::ImeEvent::Preedit(text) => {
                        self.blocked |= self.active || !text.is_empty();
                        self.active = !text.is_empty();
                    }
                    egui::ImeEvent::Commit(_) => {
                        self.committed_frame = true;
                        self.blocked = true;
                        self.active = false;
                    }
                    egui::ImeEvent::Disabled => {
                        self.blocked |= self.active;
                        self.active = false;
                    }
                }
            }
            self.composing_frame = self.blocked;
            // 被组合保护拒绝的按下，不应在组合提交后的松开帧重新变成点击。
            self.gesture |= self.blocked && input.pointer.any_pressed();
            self.blocked |= self.gesture;
            if !input.pointer.any_down() {
                self.gesture = false;
            }
        });
        if self.blocked {
            if self.owner.is_none() {
                self.owner = ctx.memory(|memory| memory.focused());
            }
        } else {
            self.owner = None;
        }
    }

    fn blocks(&self, ctx: &egui::Context) -> bool {
        self.active
            || self.gesture
            || (self.frame == Some(ctx.cumulative_frame_nr()) && self.blocked)
            || ctx.input(|input| {
                input.events.iter().any(|event| {
                    matches!(event, egui::Event::Ime(egui::ImeEvent::Commit(_)))
                        || matches!(event, egui::Event::Ime(egui::ImeEvent::Preedit(text)) if !text.is_empty())
                })
            })
    }
}

/// egui 在 TextEdit 的 response 阶段即可因远处按下失焦；绘制后恢复已经太晚。
/// 只暂存派生指针状态，不覆盖事件列表，因此 IME/粘贴和已消费快捷键不会丢失或复活。
pub(in crate::app) struct CompositionScope {
    ctx: egui::Context,
    pointer: Option<egui::PointerState>,
    lock_owner: Option<egui::Id>,
}

impl Drop for CompositionScope {
    fn drop(&mut self) {
        // TextEdit 会设置自己的 filter；在它绘制后才锁住下一帧的候选导航键。
        if let Some(owner) = self.lock_owner {
            self.ctx.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    owner,
                    egui::EventFilter {
                        tab: true,
                        escape: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                    },
                );
            });
        }
        if let Some(pointer) = self.pointer.take() {
            self.ctx.input_mut(|input| input.pointer = pointer);
        }
    }
}

impl ViewState {
    /// Only the existing owner may finish this frame's IME exchange after a
    /// workspace guard changes. This never grants general editing permission.
    pub(in crate::app) fn composition_receiver(&self, ctx: &egui::Context) -> Option<egui::Id> {
        (self.composition.frame == Some(ctx.cumulative_frame_nr())
            && self.composition.composing_frame)
            .then_some(self.composition.owner)
            .flatten()
    }

    pub(in crate::app) fn begin_input(&mut self, ctx: &egui::Context) -> CompositionScope {
        let frame = ctx.cumulative_frame_nr();
        if self.frame_input_owner.as_ref().map(|(frame, _)| *frame) != Some(frame) {
            // 编排元数据也会登记 TextEdit；先固定上帧真正的正文 owner，免被本帧重登记覆盖。
            self.frame_input_owner = Some((
                frame,
                ctx.memory(|memory| memory.focused())
                    .filter(|_| super::input_registry::owns_focus(ctx)),
            ));
        }
        self.composition.observe(ctx);
        self.ime_active = self.composition.blocks(ctx);
        if self.composition.composing_frame {
            crate::app::object_picker::consume_candidate_ime_keys(ctx);
            ctx.input_mut(|input| {
                input.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
                input.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
            });
        }
        let pointer = if self.composition.blocked {
            if let Some(owner) = self.composition.owner {
                ctx.memory_mut(|memory| memory.request_focus(owner));
            }
            Some(ctx.input_mut(|input| std::mem::take(&mut input.pointer)))
        } else {
            None
        };
        CompositionScope {
            ctx: ctx.clone(),
            pointer,
            lock_owner: self
                .composition
                .active
                .then_some(self.composition.owner)
                .flatten(),
        }
    }

    pub(super) fn committed(&self) -> bool {
        self.composition.committed_frame
    }

    pub(super) fn composing(&self) -> bool {
        self.composition.active
    }

    pub(in crate::app) fn filter_raw_input(
        &mut self,
        ctx: &egui::Context,
        raw: &mut egui::RawInput,
        available: bool,
    ) {
        let cancelled = !raw.focused
            || raw.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::WindowFocused(false)
                        | egui::Event::Touch {
                            phase: egui::TouchPhase::Cancel,
                            ..
                        }
                )
            });
        if cancelled && (self.raw_blocked_buttons != 0 || self.composition.gesture) {
            self.raw_blocked_buttons = 0;
            self.composition.gesture = false;
            ctx.input_mut(|input| input.pointer = egui::PointerState::default());
        }
        let composing = raw.focused
            && available
            && super::input_registry::owns_focus(ctx)
            && (self.composition.active
                || raw.events.iter().any(|event| {
                    matches!(event, egui::Event::Ime(egui::ImeEvent::Commit(_)))
                        || matches!(event, egui::Event::Ime(egui::ImeEvent::Preedit(text)) if !text.is_empty())
                }));
        let mut pointer_blocked = false;
        raw.events.retain(|event| {
            match event {
                egui::Event::PointerButton {
                    button, pressed, ..
                } => {
                    let bit = 1 << (*button as u8);
                    if composing || self.raw_blocked_buttons & bit != 0 {
                        if *pressed {
                            self.raw_blocked_buttons |= bit;
                        } else {
                            self.raw_blocked_buttons &= !bit;
                        }
                        pointer_blocked = true;
                        return false;
                    }
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if composing
                    && !modifiers.command
                    && !modifiers.ctrl
                    && !modifiers.alt
                    && matches!(
                        key,
                        egui::Key::Enter
                            | egui::Key::Tab
                            | egui::Key::Escape
                            | egui::Key::ArrowUp
                            | egui::Key::ArrowDown
                            | egui::Key::ArrowLeft
                            | egui::Key::ArrowRight
                            | egui::Key::PageUp
                            | egui::Key::PageDown
                    ) =>
                {
                    return false
                }
                _ => {}
            }
            true
        });
        if pointer_blocked {
            // 在 begin_pass 前撤销被拒手势的派生状态；既不触发全局栏，也不遗留按下键。
            ctx.input_mut(|input| input.pointer = egui::PointerState::default());
        }
    }

    pub(in crate::app) fn input_blocked(&self, ctx: &egui::Context) -> bool {
        self.composition.blocks(ctx)
    }
}
