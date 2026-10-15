#[path = "advanced_tests.rs"]
mod advanced;
#[path = "identity_tests.rs"]
mod identity;
#[path = "ime_tests.rs"]
mod ime;
#[path = "keyboard_tests.rs"]
mod keyboard;
#[path = "native_focus_tests.rs"]
mod native_focus;
#[path = "roundtrip_tests.rs"]
mod roundtrip;
use super::*;
use crate::app::WorldeditApp;

fn setup() -> (egui::Context, WorldeditApp) {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let ctx = egui::Context::default();
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = std::env::temp_dir().join(format!(
        "condition-ui-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
    ));
    std::fs::create_dir(&root).unwrap();
    app.project = Project::new(&root);
    app.active_file = app.project.entry.clone();
    app.project
        .documents
        .retain(|path, _| path == &app.active_file);
    app.project.set_text(&app.active_file.clone(), "let allowed = true\nlet reputation = 3\nlet title = \"访客\"\nevent start after allowed\n  choice \"进入\" if allowed\n    -> END\n".into()).unwrap();
    app.project.save().unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    (ctx, app)
}
fn buffer(app: &WorldeditApp, source: &str, slot: ConditionSlot) -> Buffer {
    let input = ConditionDraftInput {
        schema_version: 1,
        owner: owner("start", (slot != ConditionSlot::EventAfter).then_some(1)),
        slot,
        generation: 7,
        source: source.into(),
    };
    let projection = app
        .project
        .condition_draft_from_compile(&app.snapshot.as_ref().unwrap().result, &input)
        .unwrap();
    Buffer {
        baseline: app.project.content_baseline(),
        draft_signature: draft_signature(&app.event_editor.as_ref().unwrap().draft),
        tree: projection.tree.clone(),
        input,
        projection,
        preview: None,
        error: None,
    }
}
fn context(app: &WorldeditApp) -> Context<'_> {
    Context {
        project: &app.project,
        compiled: &app.snapshot.as_ref().unwrap().result,
        version: app.version,
        baseline: &app.event_editor.as_ref().unwrap().baseline,
        current_baseline: app.project.content_baseline(),
        draft_signature: draft_signature(&app.event_editor.as_ref().unwrap().draft),
        composing: false,
    }
}
#[test]
fn condition_ui_noop_cancel_preview_and_core_typed_roundtrip_preserve_source() {
    let (_, mut app) = setup();
    app.select_event("start");
    let baseline = app.project.content_baseline();
    let source = "  allowed  ";
    let mut b = buffer(&app, source, ConditionSlot::ChoiceDisplay);
    b.preview(&context(&app), source).unwrap();
    assert_eq!(b.apply(&context(&app), source).unwrap(), source);
    b.tree = Some(ConditionNode::And {
        left: Box::new(b.tree.clone().unwrap()),
        right: Box::new(ConditionNode::Compare {
            variable: "reputation".into(),
            op: ConditionComparison::Ge,
            value: ConditionLiteral::Num(3.0),
            literal_left: false,
        }),
    });
    assert!(b.dirty());
    b.preview(&context(&app), source).unwrap();
    let applied = b.apply(&context(&app), source).unwrap();
    assert!(applied.contains("reputation"));
    let reopened = buffer(&app, &applied, ConditionSlot::ChoiceDisplay);
    assert_eq!(reopened.tree, b.tree);
    assert_eq!(
        app.project.content_baseline(),
        baseline,
        "preview/apply expression buffer must not apply project"
    );
    let mut forms = ConditionForms {
        entries: vec![(owner("start", Some(1)), ConditionSlot::ChoiceDisplay, b)],
        sequence: 1,
        navigation: None,
        ..Default::default()
    };
    forms.clear();
    assert!(!forms.has_pending());
    assert_eq!(app.project.content_baseline(), baseline);
}
#[test]
fn condition_ui_pending_counts_as_real_draft_and_stale_preview_retains_input() {
    let (_, mut app) = setup();
    app.select_event("start");
    let mut b = buffer(&app, "allowed", ConditionSlot::EventAfter);
    b.tree = Some(ConditionNode::Bool { value: false });
    b.preview(&context(&app), "allowed").unwrap();
    let pending = b.tree.clone();
    app.event_editor
        .as_mut()
        .unwrap()
        .condition_forms
        .entries
        .push((owner("start", None), ConditionSlot::EventAfter, b.clone()));
    assert!(app.dirty_draft_names().contains(&"事件正文与分支"));
    assert!(b.apply(&context(&app), "not allowed").is_err());
    assert_eq!(b.tree, pending);
    let source = app
        .project
        .document(&app.active_file)
        .unwrap()
        .replace("= 3", "= 4");
    app.project
        .set_text(&app.active_file.clone(), source)
        .unwrap();
    assert!(b.apply(&context(&app), "allowed").is_err());
    assert_eq!(b.tree, pending);
}
#[test]
fn condition_ui_advanced_invalid_and_over_budget_never_construct_partial_tree() {
    let (_, mut app) = setup();
    app.select_event("start");
    for source in [
        "reputation + 1 > 2".to_owned(),
        "has(".into(),
        "not ".repeat(200) + "true",
    ] {
        let b = buffer(&app, &source, ConditionSlot::EventAfter);
        assert!(b.tree.is_none());
        assert_eq!(b.input.source, source);
        assert!(!matches!(
            b.projection.support,
            ConditionSupport::Supported | ConditionSupport::Empty
        ));
    }
}
#[test]
fn condition_ui_real_controls_preserve_advanced_source_and_ime() {
    let (ctx, mut app) = setup();
    app.select_event("start");
    let mut forms = ConditionForms::default();
    let mut source = "reputation + 1 > 2".to_owned();
    let before = source.clone();
    let baseline = app.project.content_baseline();
    for composing in [false, true, false] {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 600.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let mut context = context(&app);
                    context.composing = composing;
                    show(
                        ui,
                        &context,
                        &mut forms,
                        owner("start", None),
                        ConditionSlot::EventAfter,
                        &mut source,
                    );
                });
            },
        );
    }
    assert_eq!(source, before);
    assert!(!forms.has_pending());
    assert_eq!(app.project.content_baseline(), baseline);
}
