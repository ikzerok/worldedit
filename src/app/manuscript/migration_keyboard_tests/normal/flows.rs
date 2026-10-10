use super::*;

#[test]
fn normal_preview_keyboard_existing_operations_and_same_buffer_prose_path() {
    for case in [
        Case::UpdateText,
        Case::UpdateSay,
        Case::InsertSay,
        Case::TextToSay,
        Case::SayToText,
        Case::DeleteText,
        Case::DeleteSay,
        Case::ProseThenUpdate,
    ] {
        let mut h = setup(case);
        let plan = normal_plan(&h);
        match (&plan.request.operation, case) {
            (DialogueOperation::Update { draft, .. }, Case::UpdateText | Case::ProseThenUpdate) => {
                assert_eq!(draft.kind, DialogueKind::Text)
            }
            (DialogueOperation::Update { draft, .. }, Case::UpdateSay) => {
                assert_eq!(draft.kind, DialogueKind::Say)
            }
            (DialogueOperation::Insert { draft, .. }, Case::InsertSay) => {
                assert_eq!(draft.kind, DialogueKind::Say)
            }
            (DialogueOperation::Convert { to, .. }, Case::TextToSay) => {
                assert_eq!(*to, DialogueKind::Say)
            }
            (DialogueOperation::Convert { to, .. }, Case::SayToText) => {
                assert_eq!(*to, DialogueKind::Text)
            }
            (DialogueOperation::Delete { .. }, Case::DeleteText | Case::DeleteSay) => {
                assert!(plan.after.is_empty());
                assert!(plan.before.contains(OLD));
                if matches!(case, Case::DeleteSay) {
                    assert!(plan
                        .metadata_losses
                        .iter()
                        .any(|loss| loss.contains("delete private note")));
                }
            }
            _ => panic!("wrong real entrypoint for {case:?}"),
        }
        let state = h.state();
        let fields = h.fields();
        h.tab_to(NORMAL_READ);
        let out = h.settle();
        assert!(h.reading_focus_visible(&out));
        super::reading::read_exact(&mut h, &[plan.before.clone(), plan.after.clone()]);
        assert_eq!(h.state(), state);
        assert_eq!(h.fields(), fields);
        assert!(plan.can_apply);
        stage_apply_history(&mut h, &plan);
        if matches!(case, Case::DeleteText | Case::DeleteSay) {
            let source = h.app.project.document(&h.app.active_file).unwrap();
            assert!(!source.contains(OLD));
            for comment in ["// keep before", "// keep inline", "// keep after"] {
                assert!(
                    source.contains(comment),
                    "{case:?} lost {comment}: {source}"
                );
            }
            let target = TargetRef::new("event", "arrival");
            let buffer = h.app.project.open_writing_buffer(&target).unwrap();
            assert!(h
                .app
                .project
                .project_dialogue_buffer(&buffer, &target)
                .unwrap()
                .statements
                .is_empty());
        }
    }
}

#[test]
fn normal_preview_keyboard_direction_loss_cancel_and_explicit_confirmation() {
    let mut h =
        Harness::normal("  say traveler \"original plain\" direction \"private direction\"\n");
    h.toolbar("逐句对白");
    h.click("语句操作");
    h.click("转为普通旁白…");
    h.click("预览语句变更");
    let blocked = normal_plan(&h);
    assert!(!blocked.can_apply);
    assert!(blocked
        .metadata_losses
        .iter()
        .any(|s| s.contains("private direction")));
    let state = h.state();
    let fields = h.fields();
    h.tab_to(NORMAL_READ);
    super::reading::read_exact(&mut h, &[blocked.before.clone(), blocked.after.clone()]);
    h.tab_to("取消此句输入");
    h.key(Key::Enter, Modifiers::NONE);
    h.tab_to("继续保留此句");
    h.key(Key::Enter, Modifiers::NONE);
    assert_eq!(h.fields(), fields);
    assert_eq!(h.state(), state);
    assert!(!normal_plan(&h).can_apply);
    h.tab_to(NORMAL_READ);
    h.key(Key::Escape, Modifiers::NONE);
    assert!(!h
        .app
        .manuscript
        .writing_view
        .dialogue_plan_is_current(&blocked.source_path, &blocked));
    assert_eq!(h.fields(), fields);
    assert_eq!(h.state(), state);
    h.key(Key::Enter, Modifiers::NONE); // visible returned Preview rebuilds the blocked plan
    assert!(!normal_plan(&h).can_apply);
    h.tab_to(LOSS);
    h.key(Key::Enter, Modifiers::NONE);
    let confirmed_fields = h.fields();
    let request: worldline_core::manuscript::DialogueEditRequest =
        serde_json::from_str(confirmed_fields.values().next().unwrap()).unwrap();
    assert!(matches!(
        request.operation,
        DialogueOperation::Convert {
            allow_direction_loss: true,
            ..
        }
    ));
    tab_to_unplanned_preview(&mut h);
    h.key(Key::Enter, Modifiers::NONE);
    let plan = normal_plan(&h);
    assert!(plan.can_apply);
    assert!(!plan.after.contains("private direction"));
    assert_eq!(h.state(), state);
    stage_apply_history(&mut h, &plan);
}

// LOSS changed the request and invalidated its plan. This one transition uses
// the existing primary-button inner focus frame, not the active preview scope.
fn tab_to_unplanned_preview(h: &mut Harness) {
    let fields = h.fields();
    let request: worldline_core::manuscript::DialogueEditRequest =
        serde_json::from_str(fields.values().next().unwrap()).unwrap();
    let buffer = h.app.manuscript.writing_buffers().pop().unwrap();
    let derived = h
        .app
        .project
        .preview_dialogue_edit(&buffer, &request)
        .unwrap();
    assert!(!h
        .app
        .manuscript
        .writing_view
        .dialogue_plan_is_current(buffer.path(), &derived));
    for _ in 0..160 {
        let out = h.key(Key::Tab, Modifiers::NONE);
        let Some(response) = h.focused_response.as_ref() else {
            continue;
        };
        assert_eq!(h.ctx.memory(|m| m.focused()), Some(response.id));
        let Some(rect) = visible_label(&out, "预览语句变更") else {
            continue;
        };
        if !control_owns_label(&h.ctx, response, rect) {
            continue;
        }
        assert!(response.has_focus() && response.interact_rect.contains_rect(response.rect));
        let theme = crate::theme::resolved(&h.ctx);
        let wanted = response.rect.shrink(2.0);
        assert!(
            out.shapes.iter().any(|s| primary_frame(
                &s.shape,
                s.clip_rect,
                wanted,
                theme.colors.on_accent,
                theme.focus_width
            )),
            "the actual unplanned Preview must have its complete primary inner focus frame"
        );
        return;
    }
    panic!("real Tab did not reach the unplanned Preview action");
}
fn primary_frame(
    shape: &egui::Shape,
    clip: Rect,
    wanted: Rect,
    color: egui::Color32,
    width: f32,
) -> bool {
    match shape {
        egui::Shape::Rect(r) => {
            r.rect == wanted
                && clip.contains_rect(r.rect)
                && r.stroke.color == color
                && r.stroke.width == width
        }
        egui::Shape::Vec(shapes) => shapes
            .iter()
            .any(|s| primary_frame(s, clip, wanted, color, width)),
        _ => false,
    }
}
