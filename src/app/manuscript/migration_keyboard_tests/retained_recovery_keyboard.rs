//! Full-App external refresh and readonly protected-F recovery. No injected focus/scroll.
use super::*;
use std::collections::BTreeSet;
use worldline_core::manuscript::{DialogueEditRequest, DialogueOperation, DialoguePart};
mod flows;
use super::reading_mesh as mesh;
mod reading;
const COPY: &str = "复制这份对白输入";
const CANCEL: &str = "取消此句输入";
const KEEP: &str = "继续保留此句";
const RECOVERY_READ: &str = "原输入恢复阅读区 · ↑↓滚动";
const RELOAD: &str = "载入已应用原文，保留对白字段";
const EXTERNAL: &str = "// recovery keyboard external observation\n";
type FocusEvents = Vec<(egui::Id, egui::WidgetInfo)>;

fn fields(h: &Harness) -> BTreeMap<String, String> {
    h.app
        .manuscript
        .writing_view
        .retained_runtime_drafts(&h.app.project.root)
}
fn request(h: &Harness) -> DialogueEditRequest {
    let retained = fields(h);
    assert_eq!(retained.len(), 1, "one protected real Update request");
    serde_json::from_str(retained.values().next().unwrap()).unwrap()
}
fn buffers(h: &Harness) -> Value {
    let mut values: Vec<_> = h
        .app
        .manuscript
        .writing_buffers()
        .into_iter()
        .map(|b| {
            (
                b.path().to_owned(),
                b.identity().to_owned(),
                b.source().to_owned(),
                b.baseline().to_owned(),
                b.generation(),
                b.is_changed(),
            )
        })
        .collect();
    values.sort();
    serde_json::to_value(values).unwrap()
}
fn disk(h: &Harness) -> Value {
    let files: BTreeMap<_, _> = worldline_core::file_access::workspace_files(&h.app.project.root)
        .unwrap()
        .into_iter()
        .map(|path| {
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777
            };
            #[cfg(not(unix))]
            let mode = 0;
            let bytes = std::fs::read(&path).unwrap();
            (path, (bytes, mode))
        })
        .collect();
    serde_json::to_value(files).unwrap()
}
fn state(h: &Harness) -> Value {
    // Refresh has already invalidated history. Preserve every stack, not just their sum.
    assert!(h.app.history.is_empty() && h.app.redo.is_empty());
    assert!(h.app.search_state.undo.is_empty() && h.app.search_state.redo.is_empty());
    json!({"project":h.state(),"buffers":buffers(h),"fields":fields(h),"disk":disk(h),
        "node":format!("{:?}",h.app.history_state.current)})
}
fn fixture(long: bool, dirty_body: bool) -> Harness {
    let literal = if long {
        (0..30)
            .map(|n| format!("LITERAL_{n:02} 原输入逐字阅读中文😀\n"))
            .collect::<String>()
    } else {
        "original first line\nsecond line\nthird line".into()
    };
    let direction = if long {
        (0..28)
            .map(|n| format!("DIRECTION_{n:02} 作者私密原备注中文😀\n"))
            .collect::<String>()
    } else {
        "private original direction\nkeep complete note".into()
    };
    let content =
        format!("{literal}{{1}} middle literal [[character:traveler|shown link]] final literal");
    let source = format!(
        "  say traveler {} direction {}\n",
        serde_json::to_string(&content).unwrap(),
        serde_json::to_string(&direction).unwrap()
    );
    let mut h = Harness::normal(&source);
    // Business fixture only; polling itself below is the real full-App timer route.
    h.app.project.save().unwrap();
    let path = h.app.active_file.clone();
    h.app.load_project(path);
    h.settle();
    assert!(h.app.saved_location);
    if dirty_body {
        h.click("源码");
        let owner = h.ctx.memory(|m| m.focused()).unwrap();
        assert!(egui::TextEdit::load_state(&h.ctx, owner).is_some());
        h.key(Key::End, Modifiers::COMMAND);
        h.frame(vec![Event::Text("\n// retained dirty body\n".into())]);
        assert!(h.app.manuscript.writing_buffers()[0].is_changed());
        h.click("写作");
    }
    h.toolbar("逐句对白");
    if long {
        reveal_long_edit(&mut h, &literal);
    }
    h.click("编辑此句");
    let owner = h.ctx.memory(|m| m.focused()).unwrap();
    assert!(egui::TextEdit::load_state(&h.ctx, owner).is_some());
    h.key(Key::End, Modifiers::COMMAND);
    h.frame(vec![Event::Text(" ext08".into())]);
    let f = request(&h);
    let DialogueOperation::Update { draft, .. } = &f.operation else {
        panic!("actual Update")
    };
    assert_eq!(draft.parts.len(), 5);
    assert!(
        matches!(&draft.parts[0], DialoguePart::Literal{text} if text == &format!("{literal} ext08"))
    );
    h.key(Key::Enter, Modifiers::COMMAND);
    let b = h.app.manuscript.writing_buffers().pop().unwrap();
    let f = request(&h);
    let plan = h.app.project.preview_dialogue_edit(&b, &f).unwrap();
    assert!(plan.can_apply && !plan.no_change);
    assert!(h
        .app
        .manuscript
        .writing_view
        .dialogue_plan_is_current(b.path(), &plan));
    h.tab_to(NORMAL_READ);
    for _ in 0..12 {
        h.key(Key::ArrowDown, Modifiers::NONE);
    }
    assert!(
        h.offset() > 0.0,
        "real reader establishes the old outer offset"
    );
    h
}
// Setup only: keep the original long P and reach its existing edit button by a
// bounded real wheel at actually painted body text. No keyboard-access credit.
fn reveal_long_edit(h: &mut Harness, literal: &str) {
    let original = h.state();
    let original_fields = fields(h);
    let initial_offset = h.offset();
    let mut trace = vec![];
    let mut out = h.frame(vec![]);
    for _ in 0..24 {
        if visible_label(&out, "编辑此句").is_some() {
            assert_eq!(h.state(), original);
            assert_eq!(fields(h), original_fields);
            assert!(h.offset() > initial_offset);
            eprintln!("RECOVERY_SETUP long existing edit visible; wheel={trace:?}");
            return;
        }
        let (text, rect, clip) = texts(&out)
            .into_iter()
            .find(|(text, rect, clip)| {
                (text == literal || text == " middle literal " || text == " final literal")
                    && rect.intersect(*clip).is_positive()
            })
            .expect("actual visible portion of this exact long Say, not a toolbar margin");
        let visible = rect.intersect(clip);
        let point = visible.center();
        assert!(rect.contains(point) && clip.contains(point));
        assert!(!egui::Popup::is_any_open(&h.ctx));
        let before = h.offset();
        h.frame(vec![
            Event::PointerMoved(point),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -320.0),
                modifiers: Modifiers::NONE,
            },
        ]);
        out = h.frame(vec![]);
        trace.push((text.len(), rect, clip, point, before, h.offset()));
        assert_eq!(h.state(), original);
        assert_eq!(fields(h), original_fields);
    }
    panic!("bounded setup did not reveal real edit button: {trace:?}");
}
fn refresh(h: &mut Harness, remove: bool) {
    let old_f = fields(h);
    let old_b = buffers(h);
    let baseline = h.app.project.content_baseline();
    let path = h.app.active_file.clone();
    if remove {
        std::fs::remove_file(&path).unwrap();
    } else {
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str(EXTERNAL);
        std::fs::write(&path, text).unwrap();
    }
    let externally_changed_disk = disk(h);
    // One real observation interval, never injected last_refresh/project.refresh.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let out = h.frame(vec![]);
    assert_ne!(h.app.project.content_baseline(), baseline);
    assert_eq!(
        disk(h),
        externally_changed_disk,
        "poll never writes the external workspace"
    );
    assert_eq!(fields(h), old_f);
    assert_eq!(buffers(h), old_b);
    let b = h.app.manuscript.writing_buffers().pop().unwrap();
    assert!(h
        .app
        .project
        .project_dialogue_buffer(&b, &request(h).target)
        .is_err());
    assert_eq!(
        out.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
        std::time::Duration::ZERO
    );
    h.frame(vec![]);
    assert_eq!(fields(h), old_f);
    assert_eq!(buffers(h), old_b);
}
fn key(h: &mut Harness, key: Key, modifiers: Modifiers) -> (egui::FullOutput, FocusEvents) {
    let mut gained = vec![];
    let mut commands = vec![];
    for pressed in [true, false] {
        let out = h.frame(vec![Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers,
        }]);
        capture(h, &out, &mut gained);
        commands.extend(out.platform_output.commands);
    }
    let mut out = h.frame(vec![]);
    capture(h, &out, &mut gained);
    commands.append(&mut out.platform_output.commands);
    out.platform_output.commands = commands;
    (out, gained)
}
fn capture(h: &Harness, out: &egui::FullOutput, gained: &mut FocusEvents) {
    if let Some(r) = &h.focused_response {
        for event in &out.platform_output.events {
            if let egui::output::OutputEvent::FocusGained(info) = event {
                gained.push((r.id, info.clone()));
            }
        }
    }
}
fn reveal(h: &mut Harness, mut out: egui::FullOutput) -> egui::FullOutput {
    let response = h.focused_response.clone().expect("real current-pass focus");
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(response.id));
    if !response.interact_rect.contains_rect(response.rect) {
        assert_eq!(
            out.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
            std::time::Duration::ZERO,
            "offscreen focus must request its one normal host repaint"
        );
        let before = state(h);
        out = h.frame(vec![]);
        assert_eq!(h.ctx.memory(|m| m.focused()), Some(response.id));
        assert_eq!(h.focused_response.as_ref().unwrap().id, response.id);
        assert_eq!(state(h), before);
    }
    out
}
fn focused_control(h: &Harness, out: &egui::FullOutput, label: &str) -> egui::Id {
    let r = h.focused_response.as_ref().expect("actual focused control");
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(r.id));
    assert!(
        r.enabled() && r.has_focus() && r.interact_rect.contains_rect(r.rect),
        "{label}: owner={:?} rect={:?} interact={:?} offset={} paint={:?}",
        r.id,
        r.rect,
        r.interact_rect,
        h.offset(),
        texts(out)
    );
    assert!(
        texts(out).iter().any(|(s, rect, clip)| s == label
            && clip.contains_rect(*rect)
            && clip.contains_rect(r.rect)
            && control_owns_label(&h.ctx, r, *rect)),
        "exact local label/response must both be fully visible: {label}; owner={:?} actual={:?} rect={:?} interact={:?} sense={:?} spacing={:?}/{:?} offset={} repaint={:?} focus_paint={} exact_labels={:?}",
        r.id, h.ctx.memory(|m|m.focused()), r.rect, r.interact_rect, r.sense,
        h.ctx.style().spacing.button_padding, h.ctx.style().spacing.interact_size,
        h.offset(), out.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
        has_control_focus_paint(&h.ctx,out,r.rect),
        texts(out).into_iter().filter(|(s,_,_)|s==label).map(|(s,rect,clip)|{
            let spacing=&h.ctx.style().spacing;
            let height_limit=(rect.height()+2.0*spacing.button_padding.y).max(spacing.interact_size.y)+2.0;
            (s,rect,clip,clip.contains_rect(rect),clip.contains_rect(r.rect),
                r.rect.expand(1.0).contains_rect(rect),height_limit,
                r.rect.height()<=height_limit,control_owns_label(&h.ctx,r,rect))
        }).collect::<Vec<_>>()
    );
    assert!(
        has_control_focus_paint(&h.ctx, out, r.rect),
        "complete distinguishable local focus: {label} {:?}",
        r.rect
    );
    r.id
}
fn seek(h: &mut Harness, label: &str) -> egui::FullOutput {
    let before = state(h);
    let mut last = String::new();
    for step in 0..180 {
        let (out, gained) = key(h, Key::Tab, Modifiers::NONE);
        let Some(r) = h.focused_response.as_ref() else {
            continue;
        };
        let named = gained
            .iter()
            .any(|(id, info)| *id == r.id && info.label.as_deref() == Some(label));
        // ComboBox reports its external label, not selected_text. Only this
        // existing rebind control has the explicit empty-label adaptation.
        let rebind = label == "明确选择当前位置"
            && gained.iter().any(|(id, info)| {
                *id == r.id
                    && info.typ == egui::WidgetType::ComboBox
                    && info.label.as_deref() == Some("")
            })
            && texts(&out).iter().any(|(text, rect, clip)| {
                text == label
                    && clip.contains_rect(*rect)
                    && clip.contains_rect(r.rect)
                    && control_owns_label(&h.ctx, r, *rect)
            });
        last = format!(
            "step={step} id={:?} layer={:?} rect={:?} interact={:?} gained={gained:?}",
            r.id, r.layer_id, r.rect, r.interact_rect
        );
        if !named && !rebind {
            continue;
        }
        // A clipped label may overlap an unrelated toolbar button. Only the
        // real focus event identifies this target; geometry then verifies it.
        let target = r.id;
        assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(target));
        let out = reveal(h, out);
        assert_eq!(focused_control(h, &out, label), target);
        assert_eq!(state(h), before);
        return out;
    }
    panic!("real Tab did not reach {label}: {last}")
}
fn enter(h: &mut Harness, label: &str) -> egui::FullOutput {
    seek(h, label);
    key(h, Key::Enter, Modifiers::NONE).0
}
