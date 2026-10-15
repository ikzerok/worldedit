//! 内部注释必须从正式源码范围进入高级字段；UI 不剥注释或修补 DSL。
use super::*;
use crate::chrome::action_layout::trace;
use egui::{Event, Key, Modifiers};
const AFTER: &str = "(allowed /*前置中文🙂*/ and true)";
const DISPLAY: &str = "(allowed /*显示中文🙂*/ and true)";
const ENABLE: &str = "(reputation /*可选中文🙂*/ >= 3)";

fn ready() -> (egui::Context, WorldeditApp, String) {
    let (ctx, mut app) = setup();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let source = format!("let allowed = true\r\nlet reputation = 3\r\nevent start after {AFTER} // 原事件注释\r\n  choice once \"保留🙂\" if {DISPLAY} enable {ENABLE} disabled \"需要声望\" #wl-localization:gate // 原分支注释\r\n    不变正文🙂\r\n    -> END\r\n");
    std::fs::create_dir_all(app.project.root.join(".world")).unwrap();
    std::fs::write(
        app.project.root.join(".world/project.json"),
        r#"{"schema_version":1,"language_version":"1.12","required_features":["content.localization.v1"]}"#,
    )
    .unwrap();
    std::fs::write(&app.active_file, &source).unwrap();
    app.project = Project::open(&app.project.root).unwrap();
    app.saved_location = true;
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.snapshot.as_ref().unwrap().result.diagnostics
    );
    app.select_event("start");
    app.tab = crate::app::Tab::Timeline;
    app.personal.pending_restore = false;
    (ctx, app, source)
}
fn raw(events: Vec<Event>) -> egui::RawInput {
    let modifiers = events
        .iter()
        .find_map(|event| match event {
            Event::Key { modifiers, .. } => Some(*modifiers),
            _ => None,
        })
        .unwrap_or_default();
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1188.0, 980.0),
        )),
        events,
        modifiers,
        focused: true,
        ..Default::default()
    }
}
fn app_frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) {
    let mut input = raw(events);
    eframe::App::raw_input_hook(app, ctx, &mut input);
    let _ = ctx.run(input, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest())
    });
}
fn expressions(app: &WorldeditApp) -> [(ConditionSlot, String); 3] {
    let draft = &app.event_editor.as_ref().unwrap().draft;
    let choice = &draft.choices()[0];
    [
        (ConditionSlot::ChoiceDisplay, choice.condition.clone()),
        (ConditionSlot::ChoiceEnable, choice.enable_condition.clone()),
        (ConditionSlot::EventAfter, draft.after.clone()),
    ]
}

#[test]
fn condition_ui_three_commented_slots_open_advanced_without_partial_controls_and_return_losslessly()
{
    let (ctx, mut app, source) = ready();
    let expected = [DISPLAY, ENABLE, AFTER];
    for ((_, actual), expected) in expressions(&app).into_iter().zip(expected) {
        assert_eq!(
            actual, expected,
            "formal Draft extraction must retain internal comments"
        );
    }
    let draft = app.event_editor.as_ref().unwrap().draft.clone();
    for _ in 0..3 {
        app_frame(&ctx, &mut app, vec![]);
    }
    let forms = &app.event_editor.as_ref().unwrap().condition_forms;
    assert_eq!(forms.entries.len(), 3);
    for (slot, source) in expressions(&app) {
        let (_, _, buffer) = forms
            .entries
            .iter()
            .find(|(_, candidate, _)| *candidate == slot)
            .unwrap();
        assert_eq!(buffer.input.source, source);
        assert_eq!(buffer.projection.source, source);
        assert_eq!(buffer.projection.support, ConditionSupport::Advanced);
        assert!(buffer.tree.is_none());
    }
    assert!(!forms.has_pending());
    assert!(
        !trace(&ctx).iter().any(|r| r.role.starts_with("condition/")
            && (r.role.starts_with("condition/candidate/")
                || r.role.ends_with("-picker")
                || r.role.ends_with("/preview"))),
        "advanced expressions must not expose a partial typed editor"
    );
    app.open_object_inspection(&ctx, TargetRef::new("event", "start"));
    app_frame(&ctx, &mut app, vec![]);
    app_frame(
        &ctx,
        &mut app,
        vec![Event::Key {
            key: Key::Escape,
            physical_key: Some(Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert!(!app.replay_debugger.object_inspection.open);
    assert!(app.event_editor.as_ref().unwrap().draft == draft);
    app_frame(
        &ctx,
        &mut app,
        vec![Event::Key {
            key: Key::Escape,
            physical_key: Some(Key::Escape),
            pressed: false,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
    );
    let button = trace(&ctx)
        .into_iter()
        .find(|r| r.role == "condition/event-apply")
        .unwrap();
    assert!(button.enabled);
    assert!(button.clip.contains_rect(button.rect));
    for pressed in [true, false] {
        app_frame(
            &ctx,
            &mut app,
            vec![
                Event::PointerMoved(button.rect.center()),
                Event::PointerButton {
                    pos: button.rect.center(),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(app.project.document(&app.active_file).unwrap(), source);
    assert!(app.save());
    assert_eq!(std::fs::read_to_string(&app.active_file).unwrap(), source);
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

fn field_frame(
    ctx: &egui::Context,
    app: &WorldeditApp,
    forms: &mut ConditionForms,
    slot: ConditionSlot,
    source: &mut String,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(raw(events), |ctx| {
        crate::chrome::action_layout::clear_trace(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            show(
                ui,
                &context(app),
                forms,
                owner("start", (slot != ConditionSlot::EventAfter).then_some(1)),
                slot,
                source,
            );
        });
    })
}
fn header(output: &egui::FullOutput) -> egui::Pos2 {
    fn find(shape: &egui::Shape, clip: egui::Rect) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == "高级源码（完整保留）" => {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                clip.contains_rect(rect).then_some(rect.center())
            }
            egui::Shape::Vec(children) => children.iter().find_map(|shape| find(shape, clip)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|shape| find(&shape.shape, shape.clip_rect))
        .unwrap()
}

#[test]
fn condition_ui_explicit_advanced_text_edit_preserves_comment_and_only_rewrites_selected_slot() {
    for (slot, old, new) in [
        (
            ConditionSlot::ChoiceDisplay,
            DISPLAY,
            "(allowed /*显示中文🙂*/ and false)",
        ),
        (
            ConditionSlot::ChoiceEnable,
            ENABLE,
            "(reputation /*可选中文🙂*/ >= 4)",
        ),
        (
            ConditionSlot::EventAfter,
            AFTER,
            "(allowed /*前置中文🙂*/ and false)",
        ),
    ] {
        let (ctx, mut app, original) = ready();
        let mut source = expressions(&app)
            .into_iter()
            .find(|(s, _)| *s == slot)
            .unwrap()
            .1;
        assert_eq!(source, old);
        let mut forms = ConditionForms::default();
        let output = field_frame(&ctx, &app, &mut forms, slot, &mut source, vec![]);
        let pos = header(&output);
        for pressed in [true, false] {
            field_frame(
                &ctx,
                &app,
                &mut forms,
                slot,
                &mut source,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    },
                ],
            );
        }
        field_frame(&ctx, &app, &mut forms, slot, &mut source, vec![]);
        let ids: Vec<_> = ctx.viewport(|v| {
            v.this_pass
                .widgets
                .layers()
                .flat_map(|(_, widgets)| widgets.iter())
                .map(|w| w.id)
                .collect()
        });
        let id = ids
            .into_iter()
            .find(|id| egui::TextEdit::load_state(&ctx, *id).is_some())
            .expect("actual advanced TextEdit");
        ctx.memory_mut(|memory| memory.request_focus(id));
        for pressed in [true, false] {
            field_frame(
                &ctx,
                &app,
                &mut forms,
                slot,
                &mut source,
                vec![Event::Key {
                    key: Key::A,
                    physical_key: Some(Key::A),
                    pressed,
                    repeat: false,
                    modifiers: Modifiers::COMMAND,
                }],
            );
        }
        field_frame(
            &ctx,
            &app,
            &mut forms,
            slot,
            &mut source,
            vec![Event::Text(new.into())],
        );
        field_frame(&ctx, &app, &mut forms, slot, &mut source, vec![]);
        assert_eq!(
            source, new,
            "source change must come from actual advanced TextEdit input"
        );
        assert_eq!(
            forms.entries[0].2.projection.support,
            ConditionSupport::Advanced
        );
        assert!(forms.entries[0].2.tree.is_none());
        let editor = app.event_editor.as_mut().unwrap();
        if slot == ConditionSlot::EventAfter {
            editor.draft.after = source;
        } else {
            let mut choice = editor.draft.choices().remove(0);
            if slot == ConditionSlot::ChoiceDisplay {
                choice.condition = source;
            } else {
                choice.enable_condition = source;
            }
            editor
                .draft
                .write_choice(Some(choice.line), &choice)
                .unwrap();
        }
        let editor = app.event_editor.as_ref().unwrap().clone();
        assert!(
            app.commit("高级条件已应用", |project| project
                .write_event_at_baseline(
                    &editor.path,
                    editor.original.as_deref(),
                    &editor.draft,
                    &editor.baseline
                )),
            "{:?}",
            app.io_error
        );
        assert!(app.save());
        let reopened = Project::open(&app.project.root).unwrap();
        assert_eq!(
            reopened.document(&app.active_file).unwrap(),
            original.replacen(old, new, 1)
        );
        std::fs::remove_dir_all(&app.project.root).unwrap();
    }
}
