//! 冲突手工候选的跨帧IME保护：保留接收控件、阻止组合键与延迟鼠标手势。
#[derive(Default)]
pub(super) struct Composition {
    frame: Option<u64>,
    active: bool,
    blocked: bool,
    gesture: bool,
    owner: Option<egui::Id>,
}

impl Composition {
    pub(super) fn observe(&mut self, ctx: &egui::Context) {
        let frame = ctx.cumulative_frame_nr();
        if self.frame == Some(frame) {
            return;
        }
        self.frame = Some(frame);
        self.blocked = self.active;
        ctx.input(|input| {
            for event in &input.events {
                let egui::Event::Ime(event) = event else {
                    continue;
                };
                // 会话Enabled不代表跨帧组合；但本帧不得把IME伴随键解释为动作。
                self.blocked = true;
                match event {
                    egui::ImeEvent::Enabled => {}
                    egui::ImeEvent::Preedit(text) => self.active = !text.is_empty(),
                    egui::ImeEvent::Commit(_) | egui::ImeEvent::Disabled => self.active = false,
                }
            }
            self.gesture |= self.blocked && input.pointer.any_pressed();
            self.blocked |= self.gesture;
            if !input.pointer.any_down() {
                self.gesture = false;
            }
        });
        if self.blocked && self.owner.is_none() {
            self.owner = ctx.memory(|memory| memory.focused());
        }
        if !self.blocked {
            self.owner = None;
        }
        if self.blocked {
            ctx.input_mut(|input| {
                for key in [
                    egui::Key::Escape,
                    egui::Key::Enter,
                    egui::Key::Tab,
                    egui::Key::ArrowLeft,
                    egui::Key::ArrowRight,
                    egui::Key::ArrowUp,
                    egui::Key::ArrowDown,
                    egui::Key::PageUp,
                    egui::Key::PageDown,
                ] {
                    input.consume_key(egui::Modifiers::NONE, key);
                    input.consume_key(egui::Modifiers::SHIFT, key);
                }
            });
        }
    }

    pub(super) fn blocks_actions(&self) -> bool {
        self.active || self.blocked || self.gesture
    }

    pub(super) fn scope(&self, ctx: &egui::Context) -> Scope {
        if self.blocks_actions() {
            if let Some(owner) = self.owner {
                ctx.memory_mut(|memory| memory.request_focus(owner));
            }
        }
        Scope {
            ctx: ctx.clone(),
            pointer: self
                .blocks_actions()
                .then(|| ctx.input_mut(|input| std::mem::take(&mut input.pointer))),
            owner: self.active.then_some(self.owner).flatten(),
        }
    }
}

pub(super) struct Scope {
    ctx: egui::Context,
    pointer: Option<egui::PointerState>,
    owner: Option<egui::Id>,
}
impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(owner) = self.owner {
            self.ctx.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    owner,
                    egui::EventFilter {
                        tab: true,
                        escape: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                    },
                )
            });
        }
        if let Some(pointer) = self.pointer.take() {
            self.ctx.input_mut(|input| input.pointer = pointer);
        }
    }
}
