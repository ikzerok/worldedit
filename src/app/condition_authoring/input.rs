//! 只保护当前工程中实际登记的条件 TextEdit；在 egui 清焦点前过滤 IME 候选手势。
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Default)]
struct Inputs {
    root: PathBuf,
    frame: u64,
    ids: HashSet<egui::Id>,
    owner: Option<egui::Id>,
    blocked_buttons: u8,
}
fn key() -> egui::Id {
    egui::Id::new("condition-owned-text-inputs")
}

#[derive(Clone, Default)]
struct HostComposition {
    root: PathBuf,
    frame: Option<u64>,
    active: bool,
    blocked: bool,
}

/// 事件表单的动作保护包括普通字段，但不接管其输入或外层窗口的 TextEdit owner。
/// Enabled/Disabled 是输入法会话生命周期；只有真实预编辑和提交帧需要锁结构动作。
pub(in crate::app) fn event_composing(ctx: &egui::Context, root: &Path, available: bool) -> bool {
    host_composing(ctx, root, available, "condition-event-composition")
}

pub(in crate::app) fn object_composing(ctx: &egui::Context, root: &Path, available: bool) -> bool {
    host_composing(ctx, root, available, "object-inspection-composition")
}

// 各宿主只观察真实组合态；独立 key 防止隐藏的事件表单清掉对象窗的跨帧状态。
fn host_composing(ctx: &egui::Context, root: &Path, available: bool, host: &'static str) -> bool {
    let frame = ctx.cumulative_frame_nr();
    let (focused, events) = ctx.input(|input| {
        (
            input.focused,
            input
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Ime(event) => Some(event.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        )
    });
    ctx.data_mut(|data| {
        let state = data.get_temp_mut_or_default::<HostComposition>(egui::Id::new(host));
        if state.root != root
            || !available
            || !focused
            || state
                .frame
                .is_some_and(|previous| previous.saturating_add(1) < frame)
        {
            *state = HostComposition {
                root: root.to_owned(),
                ..Default::default()
            };
        }
        if !available || !focused {
            return false;
        }
        if state.frame == Some(frame) {
            return state.blocked;
        }
        state.frame = Some(frame);
        state.blocked = state.active;
        for event in events {
            match event {
                egui::ImeEvent::Enabled => {}
                egui::ImeEvent::Preedit(text) => {
                    state.blocked |= state.active || !text.is_empty();
                    state.active = !text.is_empty();
                }
                egui::ImeEvent::Commit(_) => {
                    state.blocked = true;
                    state.active = false;
                }
                egui::ImeEvent::Disabled => {
                    state.blocked |= state.active;
                    state.active = false;
                }
            }
        }
        state.blocked
    })
}

pub(super) fn register(response: &egui::Response, root: &Path) {
    if !response.enabled() {
        return;
    }
    let frame = response.ctx.cumulative_frame_nr();
    response.ctx.data_mut(|data| {
        let state = data.get_temp_mut_or_default::<Inputs>(key());
        if state.root != root {
            *state = Inputs {
                root: root.to_owned(),
                ..Default::default()
            };
        }
        if state.frame != frame {
            state.frame = frame;
            state.ids.clear();
        }
        state.ids.insert(response.id);
    });
}

pub(in crate::app) fn filter_raw(
    ctx: &egui::Context,
    raw: &mut egui::RawInput,
    root: &Path,
    available: bool,
) {
    let Some(mut state) = ctx.data(|data| data.get_temp::<Inputs>(key())) else {
        return;
    };
    let cancelled = !available
        || !raw.focused
        || state.root != root
        || state.frame.saturating_add(1) < ctx.cumulative_frame_nr()
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
        if state.blocked_buttons != 0 {
            ctx.input_mut(|input| input.pointer = egui::PointerState::default());
        }
        state.owner = None;
        state.blocked_buttons = 0;
        ctx.data_mut(|data| data.insert_temp(key(), state));
        return;
    }
    let focused = ctx.memory(|memory| memory.focused());
    let owns = focused.is_some_and(|id| state.ids.contains(&id));
    if !owns {
        state.owner = None;
    }
    let composing = owns && (state.owner == focused && state.owner.is_some()
        || raw.events.iter().any(|event| matches!(event,
            egui::Event::Ime(egui::ImeEvent::Commit(_)))
            || matches!(event, egui::Event::Ime(egui::ImeEvent::Preedit(text)) if !text.is_empty())));
    let mut blocked_pointer = false;
    raw.events.retain(|event| {
        match event {
            egui::Event::PointerButton {
                button, pressed, ..
            } => {
                let bit = 1 << (*button as u8);
                if composing || state.blocked_buttons & bit != 0 {
                    if *pressed {
                        state.blocked_buttons |= bit;
                    } else {
                        state.blocked_buttons &= !bit;
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
                && !modifiers.command
                && !modifiers.ctrl
                && !modifiers.alt
                && matches!(
                    key,
                    egui::Key::Escape
                        | egui::Key::Tab
                        | egui::Key::Enter
                        | egui::Key::ArrowLeft
                        | egui::Key::ArrowRight
                        | egui::Key::ArrowUp
                        | egui::Key::ArrowDown
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
    if blocked_pointer {
        ctx.input_mut(|input| input.pointer = egui::PointerState::default());
    }
    if owns {
        for event in &raw.events {
            match event {
                egui::Event::Ime(egui::ImeEvent::Preedit(text)) => {
                    state.owner = (!text.is_empty()).then_some(focused).flatten()
                }
                egui::Event::Ime(egui::ImeEvent::Commit(_) | egui::ImeEvent::Disabled) => {
                    state.owner = None
                }
                _ => {}
            }
        }
    }
    ctx.data_mut(|data| data.insert_temp(key(), state));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn condition_ui_event_composition_distinguishes_idle_lifecycle_and_real_input() {
        let ctx = egui::Context::default();
        let root = Path::new("/condition-event-input");
        let frame = |root: &Path, event: Option<egui::ImeEvent>| {
            let mut blocked = false;
            let _ = ctx.run(
                egui::RawInput {
                    focused: true,
                    events: event.map(egui::Event::Ime).into_iter().collect(),
                    ..Default::default()
                },
                |ctx| blocked = event_composing(ctx, root, true),
            );
            blocked
        };
        assert!(!frame(root, Some(egui::ImeEvent::Enabled)));
        assert!(!frame(root, Some(egui::ImeEvent::Preedit(String::new()))));
        assert!(!frame(root, Some(egui::ImeEvent::Disabled)));
        assert!(frame(
            root,
            Some(egui::ImeEvent::Preedit("普通字段候选".into()))
        ));
        assert!(frame(root, None));
        assert!(frame(
            root,
            Some(egui::ImeEvent::Commit("普通字段提交".into()))
        ));
        assert!(!frame(root, None));
        assert!(frame(
            root,
            Some(egui::ImeEvent::Preedit("取消候选".into()))
        ));
        assert!(frame(root, Some(egui::ImeEvent::Disabled)));
        assert!(!frame(root, None));
        assert!(frame(
            root,
            Some(egui::ImeEvent::Preedit("旧工程候选".into()))
        ));
        assert!(!frame(Path::new("/another-project"), None));
    }

    #[test]
    fn condition_ui_ime_guard_requires_current_registered_input_and_same_project() {
        let ctx = egui::Context::default();
        let root = Path::new("/condition-input-a");
        let mut ours = String::new();
        let mut other = String::new();
        let mut ids = None;
        for _ in 0..2 {
            let _ = ctx.run(
                egui::RawInput {
                    focused: true,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let own =
                            ui.add(egui::TextEdit::singleline(&mut ours).id_salt("condition"));
                        register(&own, root);
                        let foreign =
                            ui.add(egui::TextEdit::singleline(&mut other).id_salt("unrelated"));
                        ids = Some((own.id, foreign.id));
                    });
                },
            );
        }
        let (own, foreign) = ids.unwrap();
        let escape = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        ctx.memory_mut(|memory| memory.request_focus(own));
        let mut raw = egui::RawInput {
            focused: true,
            events: vec![
                egui::Event::Ime(egui::ImeEvent::Preedit("候选".into())),
                escape.clone(),
            ],
            ..Default::default()
        };
        filter_raw(&ctx, &mut raw, root, true);
        assert_eq!(raw.events.len(), 1);
        assert!(
            matches!(&raw.events[0], egui::Event::Ime(egui::ImeEvent::Preedit(text)) if text == "候选")
        );
        let mut switched = egui::RawInput {
            focused: true,
            events: vec![escape.clone()],
            ..Default::default()
        };
        filter_raw(&ctx, &mut switched, Path::new("/condition-input-b"), true);
        assert_eq!(switched.events, vec![escape.clone()]);
        ctx.memory_mut(|memory| memory.request_focus(foreign));
        let events = vec![
            egui::Event::Ime(egui::ImeEvent::Commit("别处".into())),
            escape,
        ];
        let mut raw = egui::RawInput {
            focused: true,
            events: events.clone(),
            ..Default::default()
        };
        filter_raw(&ctx, &mut raw, root, true);
        assert_eq!(raw.events, events, "another TextEdit keeps all its input");
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(foreign));
    }
}
