//! 仅章节检索三个输入框的IME拥有权；不接管正文或其他窗口。
use std::{collections::HashSet, path::Path};

pub(super) fn id(root: &Path, book: &str, field: &str) -> egui::Id {
    egui::Id::new(("manuscript-query-input", root, book, field))
}
#[derive(Clone, Default)]
struct Registry {
    frame: u64,
    ids: HashSet<egui::Id>,
}
fn registry_id() -> egui::Id {
    egui::Id::new("manuscript-query-owned-inputs")
}
pub(super) fn register(response: &egui::Response) {
    if response.has_focus() {
        response.ctx.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    vertical_arrows: true,
                    horizontal_arrows: true,
                    tab: false,
                    escape: false,
                },
            )
        });
    }
    let frame = response.ctx.cumulative_frame_nr();
    response.ctx.data_mut(|data| {
        let registry = data.get_temp_mut_or_default::<Registry>(registry_id());
        if registry.frame != frame {
            registry.frame = frame;
            registry.ids.clear();
        }
        registry.ids.insert(response.id);
    });
}
fn owns_focus(ctx: &egui::Context) -> bool {
    let focused = ctx.memory(|memory| memory.focused());
    let frame = ctx.cumulative_frame_nr();
    ctx.data(|data| {
        data.get_temp::<Registry>(registry_id())
            .is_some_and(|registry| {
                registry.frame.saturating_add(1) >= frame
                    && focused.is_some_and(|id| registry.ids.contains(&id))
            })
    })
}
fn candidate(key: egui::Key) -> bool {
    matches!(
        key,
        egui::Key::Enter
            | egui::Key::Escape
            | egui::Key::Tab
            | egui::Key::ArrowUp
            | egui::Key::ArrowDown
            | egui::Key::ArrowLeft
            | egui::Key::ArrowRight
            | egui::Key::PageUp
            | egui::Key::PageDown
    )
}
#[derive(Default)]
pub(super) struct Composition {
    frame: Option<u64>,
    active: bool,
    blocked: bool,
    gesture: bool,
    owner: Option<egui::Id>,
    raw_buttons: u8,
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
impl Composition {
    pub(super) fn blocked(&self) -> bool {
        self.blocked
    }
    pub(super) fn begin(&mut self, ctx: &egui::Context) -> Scope {
        let frame = ctx.cumulative_frame_nr();
        if self.frame != Some(frame) {
            self.frame = Some(frame);
            if !owns_focus(ctx) {
                self.active = false;
                self.blocked = false;
                self.gesture = false;
                self.owner = None;
            } else {
                self.blocked = self.active;
                ctx.input(|input| {
                    for event in &input.events {
                        if let egui::Event::Ime(event) = event {
                            match event {
                                egui::ImeEvent::Enabled => {}
                                egui::ImeEvent::Preedit(text) => {
                                    self.blocked |= self.active || !text.is_empty();
                                    self.active = !text.is_empty();
                                }
                                egui::ImeEvent::Commit(_) => {
                                    self.blocked = true;
                                    self.active = false;
                                }
                                egui::ImeEvent::Disabled => {
                                    self.blocked |= self.active;
                                    self.active = false;
                                }
                            }
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
            }
        }
        if self.blocked {
            ctx.input_mut(|input| {
                input.events.retain(|event| {
                    !matches!(event,
                egui::Event::Key { key, pressed: true, modifiers, .. }
                if candidate(*key) && !modifiers.command && !modifiers.ctrl && !modifiers.alt)
                })
            });
            if let Some(owner) = self.owner {
                ctx.memory_mut(|memory| memory.request_focus(owner));
            }
        }
        Scope {
            ctx: ctx.clone(),
            pointer: self
                .blocked
                .then(|| ctx.input_mut(|input| std::mem::take(&mut input.pointer))),
            owner: self.active.then_some(self.owner).flatten(),
        }
    }
    pub(super) fn filter_raw(
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
        if cancelled {
            self.active = false;
            self.blocked = false;
            self.gesture = false;
            self.owner = None;
            self.raw_buttons = 0;
        }
        let ours = available && owns_focus(ctx);
        if !ours {
            self.active = false;
            self.blocked = false;
            self.owner = None;
        }
        let composing = raw.focused && ours && (self.active || raw.events.iter().any(|event|
            matches!(event, egui::Event::Ime(egui::ImeEvent::Commit(_)))
                || matches!(event, egui::Event::Ime(egui::ImeEvent::Preedit(text)) if !text.is_empty())));
        let mut blocked_pointer = cancelled;
        raw.events.retain(|event| {
            match event {
                egui::Event::PointerButton {
                    button, pressed, ..
                } => {
                    let bit = 1 << (*button as u8);
                    if composing || self.raw_buttons & bit != 0 {
                        if *pressed {
                            self.raw_buttons |= bit;
                        } else {
                            self.raw_buttons &= !bit;
                        }
                        blocked_pointer = true;
                        return false;
                    }
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if composing
                    && candidate(*key)
                    && !modifiers.command
                    && !modifiers.ctrl
                    && !modifiers.alt =>
                {
                    return false
                }
                _ => {}
            }
            true
        });
        if blocked_pointer {
            ctx.input_mut(|input| input.pointer = egui::PointerState::default());
        }
    }
}
pub(super) fn requests_results(ui: &egui::Ui, response: &egui::Response, blocked: bool) -> bool {
    !blocked
        && (response.has_focus() || response.lost_focus())
        && ui.input_mut(|input| {
            input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)
                || input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
        })
}
