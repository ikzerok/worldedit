//! 实际 TextEdit + 指针事件；不冒充原生输入法验证。
use super::*;
use egui::{Event, PointerButton};
mod ime_sequences;
mod recovery;

fn typography() -> Typography {
    Typography {
        compact: false,
        size: 17.0,
        spacing: 1.6,
        width: 900.0,
        source_size: 13.0,
    }
}
fn frame(
    ctx: &egui::Context,
    project: &Project,
    buffer: &WritingBuffer,
    target: &TargetRef,
    view: &mut ViewState,
    events: Vec<Event>,
) -> egui::FullOutput {
    frame_with_evidence(ctx, project, buffer, target, view, events).0
}
fn frame_with_evidence(
    ctx: &egui::Context,
    project: &Project,
    buffer: &WritingBuffer,
    target: &TargetRef,
    view: &mut ViewState,
    events: Vec<Event>,
) -> (egui::FullOutput, bool) {
    let mut registered = false;
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1700.0, 2000.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| {
            let _composition = view.begin_input(ctx);
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut same_buffer = buffer.clone();
                    super::super::editors::draw(
                        ui,
                        project,
                        &mut same_buffer,
                        target,
                        view,
                        typography(),
                        &mut Action::default(),
                    );
                    assert_eq!(same_buffer.identity(), buffer.identity());
                });
            });
            // Context::run 返回前会推进 frame_nr，必须在绘制闭包里采集registry。
            registered = view.dialogue.forms.values().any(|form| {
                form.input_ids
                    .iter()
                    .any(|id| super::super::input_registry::drawn_this_frame(ctx, *id))
            });
        },
    );
    (output, registered)
}
fn point(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.text() == label => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| point(shape, label)),
        _ => None,
    }
}
fn press(pos: egui::Pos2, pressed: bool) -> Vec<Event> {
    vec![
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}
#[test]
fn dialogue_replacement_waits_for_all_fields_in_both_row_directions_and_top_insert() {
    for (active, top, paste) in [(1, false, true), (0, false, false), (1, true, true)] {
        let (mut project, _, target) = tests::fixture();
        let path = project.entry.clone();
        let source = project
            .document(&path)
            .unwrap()
            .replace("  -> END", "  say b \"另一句\"\n  -> END");
        project.set_text(&path, source).unwrap();
        project.save().unwrap();
        let buffer = project.open_writing_buffer(&target).unwrap();
        let projection = project.project_dialogue_buffer(&buffer, &target).unwrap();
        let statement = projection.statements[active].clone();
        let ctx = egui::Context::default();
        let mut view = ViewState::default();
        view.dialogue.begin_context(
            &ctx,
            &buffer,
            &projection,
            DialogueOperation::Update {
                statement_id: statement.id.clone(),
                draft: statement.draft.clone(),
            },
            "测试当前输入".into(),
        );
        if top {
            view.dialogue.insert_requested = true;
            view.dialogue.anchor = Some(projection.anchors[0].id.clone());
        }
        for _ in 0..3 {
            frame(&ctx, &project, &buffer, &target, &mut view, vec![]);
        }
        let label = if top {
            "在此写正式台词"
        } else {
            "编辑此句"
        };
        let output = frame(&ctx, &project, &buffer, &target, &mut view, vec![]);
        let pos = output
            .shapes
            .iter()
            .find_map(|shape| point(&shape.shape, label))
            .expect("真实切换控件");
        let mut events = vec![if paste {
            Event::Paste("同帧保留".into())
        } else {
            Event::Text("同帧保留".into())
        }];
        events.extend(press(pos, true));
        events.extend(press(pos, false));
        frame(&ctx, &project, &buffer, &target, &mut view, events);
        let form = &view.dialogue.forms[&Key::new(&buffer, &target)];
        match &form.request.operation {
            DialogueOperation::Update {
                statement_id,
                draft,
            } => {
                assert_eq!(statement_id, &statement.id);
                assert!(draft.parts.iter().any(|part| matches!(part, DialoguePart::Literal { text } if text.contains("同帧保留"))));
            }
            other => panic!("替换丢失旧输入：{other:?}"),
        }
        assert!(form.protected());
        assert!(view.dialogue.pending.is_none());
        assert!(!buffer.is_changed());
        std::fs::remove_dir_all(project.root).unwrap();
    }
}
#[test]
fn dialogue_large_real_chapter_is_bounded_and_keeps_off_page_input_editable() {
    let (mut project, _, target) = tests::fixture();
    let path = project.entry.clone();
    let mut source = "character a as \"人物\"\nevent start\n".to_owned();
    for index in 0..512 {
        source.push_str(&format!("  say a \"真实净台词第{index}句\"\n"));
    }
    source.push_str("  -> END\n");
    project.set_text(&path, source).unwrap();
    project.save().unwrap();
    let buffer = project.open_writing_buffer(&target).unwrap();
    let mut view = tests::open_form(&project, &buffer, &target);
    let ctx = egui::Context::default();
    frame(&ctx, &project, &buffer, &target, &mut view, vec![]);
    let identity = buffer.identity();
    assert_eq!(
        view.dialogue
            .cache
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .statements
            .len(),
        512
    );
    assert_eq!(view.dialogue.statement_indices.len(), 512);
    assert!(view.dialogue.rendered_rows <= render_state::PAGE_ROWS);
    let builds = view.dialogue.cache_builds;
    view.dialogue.row_offset = 320;
    let (output, registered) = frame_with_evidence(
        &ctx,
        &project,
        &buffer,
        &target,
        &mut view,
        vec![Event::Text("跨页保留输入".into())],
    );
    let form = &view.dialogue.forms[&Key::new(&buffer, &target)];
    assert!(form.protected());
    assert!(registered, "页外原字段在同一绘制帧中登记为真实输入接收者");
    assert!(
        output
            .shapes
            .iter()
            .any(|shape| contains(&shape.shape, "跨页保留输入")),
        "当前页外原字段必须仍然可见"
    );
    for _ in 0..5 {
        frame(&ctx, &project, &buffer, &target, &mut view, vec![]);
    }
    assert_eq!(view.dialogue.cache_builds, builds);
    assert_eq!(
        view.projection_cache.builds, 0,
        "正式稳定帧不走全稿旧投影 key"
    );
    assert!(view.dialogue.rendered_rows <= render_state::PAGE_ROWS);
    assert_eq!(buffer.identity(), identity);
    view.invalidate_projection();
    frame(&ctx, &project, &buffer, &target, &mut view, vec![]);
    assert_eq!(view.dialogue.cache_builds, builds + 1);
    assert!(view.has_dialogue_input());
    std::fs::remove_dir_all(project.root).unwrap();
}

#[test]
fn dialogue_attachment_observation_rebuilds_cache_and_exposes_all_rebinds() {
    let (mut project, buffer, target) = tests::fixture();
    let old = project.project_dialogue_buffer(&buffer, &target).unwrap();
    let statement = old.statements[0].clone();
    let old_baseline = project.content_baseline();
    let old_identity = buffer.identity();
    let old_observation = project.catalog_scope_observation_key();
    let ctx = egui::Context::default();
    let mut view = tests::open_form(&project, &buffer, &target);
    frame(&ctx, &project, &buffer, &target, &mut view, vec![]);
    let builds = view.dialogue.cache_builds;
    let asset = project.root.join("attachment.png");
    std::fs::write(&asset, b"ordinary attachment").unwrap();
    assert!(project.refresh().unwrap().is_empty());
    assert_eq!(project.content_baseline(), old_baseline);
    assert_eq!(buffer.identity(), old_identity);
    assert_ne!(project.catalog_scope_observation_key(), old_observation);
    frame(&ctx, &project, &buffer, &target, &mut view, vec![]);
    assert_eq!(view.dialogue.cache_builds, builds + 1);
    let current = view
        .dialogue
        .cache
        .as_ref()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone();
    assert_eq!(old.baseline, current.baseline);
    assert_eq!(old.generation, current.generation);
    assert_ne!(old.snapshot, current.snapshot);
    for operation in [
        DialogueOperation::Update {
            statement_id: statement.id.clone(),
            draft: statement.draft.clone(),
        },
        DialogueOperation::Delete {
            statement_id: statement.id.clone(),
        },
        DialogueOperation::Convert {
            statement_id: statement.id.clone(),
            to: DialogueKind::Text,
            speaker: None,
            allow_direction_loss: false,
        },
        DialogueOperation::Insert {
            anchor_id: old.anchors[0].id.clone(),
            draft: statement.draft.clone(),
        },
    ] {
        view.dialogue.forms.clear();
        view.dialogue
            .begin_context(&ctx, &buffer, &old, operation, "观察过期输入".into());
        let key = Key::new(&buffer, &target);
        assert!(form::needs_rebind(
            &view.dialogue.forms[&key],
            &current,
            &view.dialogue.statement_indices
        ));
        let request = view.dialogue.forms[&key].request.clone();
        let output = frame(&ctx, &project, &buffer, &target, &mut view, vec![]);
        assert!(output
            .shapes
            .iter()
            .any(|shape| point(&shape.shape, "明确选择当前位置").is_some()));
        assert_eq!(view.dialogue.forms[&key].request, request);
    }
    let observed = project.catalog_scope_observation_key();
    std::fs::remove_file(asset).unwrap();
    assert!(project.refresh().unwrap().is_empty());
    assert_ne!(project.catalog_scope_observation_key(), observed);
    assert_eq!(project.content_baseline(), old_baseline);
    frame(&ctx, &project, &buffer, &target, &mut view, vec![]);
    assert_eq!(view.dialogue.cache_builds, builds + 2);
    std::fs::remove_dir_all(project.root).unwrap();
}

fn contains(shape: &egui::Shape, needle: &str) -> bool {
    match shape {
        egui::Shape::Text(text) => text.galley.text().contains(needle),
        egui::Shape::Vec(shapes) => shapes.iter().any(|shape| contains(shape, needle)),
        _ => false,
    }
}
