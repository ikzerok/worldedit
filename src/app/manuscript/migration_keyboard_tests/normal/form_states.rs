//! Full-App state transitions, without synthetic focus, scroll or successful plans.
use super::*;
use worldline_core::manuscript::DialogueEditRequest;
mod guards;
mod lifecycle;
const FIRST: &str = "first line\nsecond line\nthird line";
const MIDDLE: &str = " middle literal ";
const LAST: &str = " final literal";
const NOTE: &str = "private direction\nkeep complete note";
const PREVIEW: &str = "预览语句变更";

fn long_form() -> Harness {
    let mut h = Harness::normal("  say traveler \"first line\\nsecond line\\nthird line{1} middle literal [[character:traveler|shown link]] final literal\" direction \"private direction\\nkeep complete note\"\n");
    assert!(
        !immediate(&h),
        "default prose has no typed form scroll policy"
    );
    h.toolbar("逐句对白");
    assert!(
        !immediate(&h),
        "a typed list without a Form keeps its scroll policy"
    );
    h.click("编辑此句");
    assert!(
        immediate(&h),
        "clean initial Form is independent of plan/protected state"
    );
    let buffer = h.app.manuscript.writing_buffers().pop().unwrap();
    let projection = h
        .app
        .project
        .project_dialogue_buffer(&buffer, &TargetRef::new("event", "arrival"))
        .unwrap();
    assert_eq!(projection.statements.len(), 1);
    assert_eq!(projection.statements[0].draft.parts.len(), 5);
    assert!(
        !h.app.manuscript.has_dialogue_input(),
        "clean Update starts without protected F"
    );
    h
}
fn immediate(h: &Harness) -> bool {
    let buffer = h.app.manuscript.writing_buffers().pop().unwrap();
    h.app
        .manuscript
        .writing_view
        .uses_immediate_dialogue_scroll(
            &h.app.project,
            &buffer,
            &TargetRef::new("event", "arrival"),
        )
}
fn fields(h: &Harness) -> BTreeMap<String, String> {
    h.app
        .manuscript
        .writing_view
        .retained_runtime_drafts(&h.app.project.root)
}
fn request(h: &Harness) -> DialogueEditRequest {
    let f = fields(h);
    assert_eq!(f.len(), 1);
    serde_json::from_str(f.values().next().unwrap()).unwrap()
}
fn current(h: &Harness, plan: &DialogueEditPlan) -> bool {
    let path = h.app.project.root.join(&plan.source_path);
    let buffer = h
        .app
        .manuscript
        .writing_buffers()
        .into_iter()
        .find(|buffer| buffer.path() == path)
        .expect("the exact plan file has its real WritingBuffer");
    h.app
        .manuscript
        .writing_view
        .dialogue_plan_is_current(buffer.path(), plan)
}

fn no_reader(out: &egui::FullOutput) {
    assert!(!labels(out).contains(READ));
    assert!(!labels(out).contains(NORMAL_READ));
}
fn key(
    h: &mut Harness,
    key: Key,
    modifiers: Modifiers,
) -> (egui::FullOutput, Vec<egui::WidgetInfo>) {
    let mut info = vec![];
    let mut commands = vec![];
    for pressed in [true, false] {
        let out = h.frame(vec![Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers,
        }]);
        for event in out.platform_output.events {
            if let egui::output::OutputEvent::FocusGained(w) = event {
                info.push(w);
            }
        }
        commands.extend(out.platform_output.commands);
    }
    let mut out = h.frame(vec![]);
    for event in &out.platform_output.events {
        if let egui::output::OutputEvent::FocusGained(w) = event {
            info.push(w.clone());
        }
    }
    commands.append(&mut out.platform_output.commands);
    out.platform_output.commands = commands;
    (out, info)
}
fn finish_reveal(h: &mut Harness, mut out: egui::FullOutput) -> egui::FullOutput {
    let r = h.focused_response.clone().expect("actual final-pass owner");
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(r.id));
    if !r.interact_rect.contains_rect(r.rect) {
        assert_eq!(
            out.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
            std::time::Duration::ZERO,
            "offscreen control must request a real redraw"
        );
        let state = h.state();
        let f = fields(h);
        out = h.frame(vec![]);
        assert_eq!(h.ctx.memory(|m| m.focused()), Some(r.id));
        assert_eq!(h.focused_response.as_ref().unwrap().id, r.id);
        assert_eq!(h.state(), state);
        assert_eq!(fields(h), f);
    }
    out
}
fn standard_text_frame(h: &Harness, out: &egui::FullOutput, r: &egui::Response) -> bool {
    let stroke = h.ctx.style().visuals.selection.stroke;
    let target = r
        .rect
        .expand(h.ctx.style().visuals.widgets.active.expansion);
    fn visit(s: &egui::Shape, clip: Rect, target: Rect, stroke: egui::Stroke) -> bool {
        match s {
            egui::Shape::Rect(p) => {
                p.stroke == stroke
                    && clip.contains_rect(p.rect)
                    && (p.rect.min - target.min).length() <= 0.5
                    && (p.rect.max - target.max).length() <= 0.5
            }
            egui::Shape::Vec(v) => v.iter().any(|s| visit(s, clip, target, stroke)),
            _ => false,
        }
    }
    out.shapes
        .iter()
        .any(|s| visit(&s.shape, s.clip_rect, target, stroke))
}
fn primary_frame(h: &Harness, out: &egui::FullOutput, r: &egui::Response) -> bool {
    let theme = crate::theme::resolved(&h.ctx);
    fn visit(s: &egui::Shape, clip: Rect, target: Rect, stroke: egui::Stroke) -> bool {
        match s {
            egui::Shape::Rect(p) => {
                p.rect == target && p.stroke == stroke && clip.contains_rect(p.rect)
            }
            egui::Shape::Vec(v) => v.iter().any(|s| visit(s, clip, target, stroke)),
            _ => false,
        }
    }
    out.shapes.iter().any(|s| {
        visit(
            &s.shape,
            s.clip_rect,
            r.rect.shrink(2.0),
            egui::Stroke::new(theme.focus_width, theme.colors.on_accent),
        )
    })
}
fn seek(h: &mut Harness, value: &str, text: bool, reverse: bool) -> egui::Id {
    let state = h.state();
    let f = fields(h);
    let mut last = String::new();
    for _ in 0..180 {
        let (out, info) = key(
            h,
            Key::Tab,
            if reverse {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
        );
        let Some(r) = h.focused_response.clone() else {
            continue;
        };
        let named = info.iter().any(|w| {
            if text {
                w.typ == egui::WidgetType::TextEdit
                    && w.current_text_value.as_deref() == Some(value)
            } else {
                w.label.as_deref() == Some(value)
            }
        });
        let drawn = texts(&out).iter().any(|(s, rect, _)| {
            s == value
                && if text {
                    r.rect.expand(1.0).contains_rect(*rect)
                        && egui::TextEdit::load_state(&h.ctx, r.id).is_some()
                } else {
                    control_owns_label(&h.ctx, &r, *rect)
                }
        });
        last = format!(
            "target={value:?} id={:?} rect={:?} interact={:?} info={info:?} offset={} paint={:?}",
            r.id,
            r.rect,
            r.interact_rect,
            h.offset(),
            texts(&out)
        );
        if !named && !drawn {
            continue;
        }
        let out = finish_reveal(h, out);
        let r = h.focused_response.clone().unwrap();
        assert_eq!(h.ctx.memory(|m| m.focused()), Some(r.id));
        assert!(
            r.has_focus() && r.interact_rect.contains_rect(r.rect),
            "full local response: {last}"
        );
        assert!(
            texts(&out).iter().any(|(s, rect, clip)| s == value
                && clip.contains_rect(*rect)
                && if text {
                    r.rect.expand(1.0).contains_rect(*rect)
                } else {
                    clip.contains_rect(r.rect) && control_owns_label(&h.ctx, &r, *rect)
                }),
            "same-pass exact visible field/control: {last}"
        );
        assert!(
            has_control_focus_paint(&h.ctx, &out, r.rect)
                || (text && standard_text_frame(h, &out, &r))
                || (!text && value == PREVIEW && primary_frame(h, &out, &r)),
            "distinguishable exact owner focus: {last}"
        );
        assert_eq!(h.state(), state);
        assert_eq!(fields(h), f);
        return r.id;
    }
    panic!("real Tab failed: {last}")
}
fn field(h: &mut Harness, value: &str) -> egui::Id {
    seek(h, value, true, false)
}
fn control(h: &mut Harness, label: &str) -> egui::Id {
    seek(h, label, false, false)
}
fn enter(h: &mut Harness, label: &str) {
    control(h, label);
    key(h, Key::Enter, Modifiers::NONE);
}
fn replace_current(h: &mut Harness, value: &str) {
    assert!(egui::TextEdit::load_state(&h.ctx, h.ctx.memory(|m| m.focused()).unwrap()).is_some());
    key(h, Key::A, Modifiers::COMMAND);
    h.frame(vec![Event::Text(value.into())]);
}
fn copied_request(h: &mut Harness) -> DialogueEditRequest {
    control(h, "复制保留的 typed 输入");
    let (out, _) = key(h, Key::Enter, Modifiers::NONE);
    let copies: Vec<_> = out
        .platform_output
        .commands
        .iter()
        .filter_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t),
            _ => None,
        })
        .collect();
    assert_eq!(copies.len(), 1);
    serde_json::from_str(copies[0]).unwrap()
}

fn input_evidence(h: &Harness, out: &egui::FullOutput) -> String {
    let response = h.focused_response.as_ref();
    let owner = h.ctx.memory(|memory| memory.focused());
    let cursor = owner
        .and_then(|id| egui::TextEdit::load_state(&h.ctx, id))
        .and_then(|state| state.cursor.char_range());
    let paint: Vec<_> = texts(out)
        .into_iter()
        .filter(|(text, rect, _)| {
            text == "1 + 2" || response.is_some_and(|r| r.rect.intersects(*rect))
        })
        .collect();
    format!("request={:?}; final_owner={owner:?}; samepass={:?}; cursor={cursor:?}; offset={}; repaint={:?}; local_paint={paint:?}",
        request(h), response.map(|r|(r.id,r.rect,r.interact_rect,r.has_focus(),r.layer_id)),h.offset(),
        out.viewport_output.get(&egui::ViewportId::ROOT).map(|o|o.repaint_delay))
}
