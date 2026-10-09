//! 仅模板定义输入拥有 IME；会话通知不等于组合，组合提交帧不触发操作。
use super::ManagerState;
use std::collections::HashSet;

#[derive(Clone, Default)]
struct Registry {
    frame: u64,
    ids: HashSet<egui::Id>,
}
fn key() -> egui::Id {
    egui::Id::new("template-definition-inputs")
}
pub(super) fn register(response: &egui::Response) {
    let frame = response.ctx.cumulative_frame_nr();
    response.ctx.data_mut(|data| {
        let registry = data.get_temp_mut_or_default::<Registry>(key());
        if registry.frame != frame {
            registry.frame = frame;
            registry.ids.clear();
        }
        registry.ids.insert(response.id);
    });
}
fn owns_focus(ctx: &egui::Context) -> bool {
    let Some(id) = ctx.memory(|memory| memory.focused()) else {
        return false;
    };
    // 后台 repaint 会请求写锁；data 读锁内不能再次读取同一个 Context。
    let frame = ctx.cumulative_frame_nr();
    ctx.data(|data| {
        data.get_temp::<Registry>(key()).is_some_and(|registry| {
            registry.frame.saturating_add(1) >= frame && registry.ids.contains(&id)
        })
    })
}
#[derive(Default)]
pub(super) struct Composition {
    active: bool,
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
pub(super) fn begin(state: &mut ManagerState, ctx: &egui::Context) -> Scope {
    let composition = &mut state.composition;
    if !owns_focus(ctx) {
        composition.active = false;
        composition.gesture = false;
        composition.owner = None;
        state.ime_composing = false;
        return Scope {
            ctx: ctx.clone(),
            pointer: None,
            owner: None,
        };
    }
    let mut blocked = composition.active;
    ctx.input(|input| {
        for event in &input.events {
            let egui::Event::Ime(event) = event else {
                continue;
            };
            match event {
                egui::ImeEvent::Enabled => {}
                egui::ImeEvent::Preedit(text) => {
                    blocked |= composition.active || !text.is_empty();
                    composition.active = !text.is_empty();
                }
                egui::ImeEvent::Commit(_) => {
                    blocked = true;
                    composition.active = false;
                }
                egui::ImeEvent::Disabled => {
                    blocked |= composition.active;
                    composition.active = false;
                }
            }
        }
        composition.gesture |= blocked && input.pointer.any_pressed();
        blocked |= composition.gesture;
        if !input.pointer.any_down() {
            composition.gesture = false;
        }
    });
    if blocked && composition.owner.is_none() {
        composition.owner = ctx.memory(|memory| memory.focused());
    }
    if !blocked {
        composition.owner = None;
    }
    state.ime_composing = blocked;
    if blocked {
        ctx.input_mut(|input| {
            for key in [
                egui::Key::Enter,
                egui::Key::Tab,
                egui::Key::Escape,
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
        if let Some(owner) = composition.owner {
            ctx.memory_mut(|memory| memory.request_focus(owner));
        }
    }
    Scope {
        ctx: ctx.clone(),
        pointer: blocked.then(|| ctx.input_mut(|input| std::mem::take(&mut input.pointer))),
        owner: composition.active.then_some(composition.owner).flatten(),
    }
}

pub(super) fn single(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut String,
) -> egui::Response {
    let response = ui.add(
        egui::TextEdit::singleline(value)
            .id(egui::Id::new(id))
            .desired_width(f32::INFINITY),
    );
    register(&response);
    response
}
pub(super) fn multiline(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash,
    value: &mut String,
    rows: usize,
) -> egui::Response {
    let response = ui.add(
        egui::TextEdit::multiline(value)
            .id(egui::Id::new(id))
            .desired_rows(rows)
            .desired_width(f32::INFINITY),
    );
    register(&response);
    response
}

pub(super) fn blocks_actions(state: &ManagerState, ctx: &egui::Context) -> bool {
    owns_focus(ctx)
        && (state.composition_busy()
            || ctx.input(|input| {
                input.events.iter().any(|event| {
        matches!(event, egui::Event::Ime(egui::ImeEvent::Commit(_)))
            || matches!(event, egui::Event::Ime(egui::ImeEvent::Preedit(text)) if !text.is_empty())
    })
            }))
}

pub(super) fn filter_raw_input(
    state: &mut ManagerState,
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
        state.composition.raw_buttons = 0;
        state.composition.gesture = false;
        state.composition.active = false;
        state.composition.owner = None;
        state.ime_composing = false;
        ctx.input_mut(|input| input.pointer = egui::PointerState::default());
    }
    let composing = raw.focused && available && owns_focus(ctx)
        && (state.composition.active || raw.events.iter().any(|event| {
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
                if composing || state.composition.raw_buttons & bit != 0 {
                    if *pressed {
                        state.composition.raw_buttons |= bit;
                    } else {
                        state.composition.raw_buttons &= !bit;
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
        ctx.input_mut(|input| input.pointer = egui::PointerState::default());
    }
}
