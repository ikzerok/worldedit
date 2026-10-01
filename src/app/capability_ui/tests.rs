use super::*;
use egui::{pos2, vec2, Event, PointerButton, RawInput, Rect};

fn app() -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = std::env::temp_dir().join(format!(
        "capability-ui-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.project = worldline_core::project::Project::new(&root);
    app.active_file = app.project.entry.clone();
    app.recompile();
    (ctx, app)
}
fn frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1800.0, 1400.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            app.entity_editor_window(ctx);
            app.capability_window(ctx);
        },
    )
}
fn find(shape: &egui::Shape, text: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(value) if value.galley.job.text == text => {
            Some(value.pos + value.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(values) => values.iter().find_map(|shape| find(shape, text)),
        _ => None,
    }
}
fn click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for _ in 0..3 {
        frame(ctx, app, Vec::new());
    }
    let output = frame(ctx, app, Vec::new());
    let point = output
        .shapes
        .iter()
        .find_map(|shape| find(&shape.shape, label))
        .unwrap_or_else(|| panic!("找不到按钮 {label}"));
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}
fn preview(ctx: &egui::Context, app: &mut WorldeditApp) {
    app.open_capabilities();
    app.capability_ui.as_mut().unwrap().target = LanguageVersion::V1_13;
    click(ctx, app, "预览全稿兼容影响");
}
fn approve(ctx: &egui::Context, app: &mut WorldeditApp) {
    click(
        ctx,
        app,
        "我已查看兼容影响；旧存档/检查点须匹配指纹，入口轨迹须重新严格验证",
    );
    click(ctx, app, "确认启用（不自动保存）");
}

#[test]
fn cancel_keeps_19_bytes_and_preview_is_not_application() {
    let (ctx, mut app) = app();
    let baseline = app.project.content_baseline();
    let fingerprint = worldline_core::analysis::fingerprint_program(&app.project.compile().program);
    preview(&ctx, &mut app);
    assert!(
        app.capability_ui
            .as_ref()
            .unwrap()
            .plan
            .as_ref()
            .unwrap()
            .can_apply
    );
    assert_eq!(app.project.content_baseline(), baseline);
    click(&ctx, &mut app, "取消 / 保留当前设置");
    assert!(app.capability_ui.is_none());
    assert_eq!(app.project.language_version(), "1.9");
    assert_eq!(
        worldline_core::analysis::fingerprint_program(&app.project.compile().program),
        fingerprint
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn enabling_by_ui_creates_three_entity_kinds_without_json_and_undo_restores_language() {
    let (ctx, mut app) = app();
    let sources = app.project.sources();
    let fingerprint = app.project.compile().analysis.fingerprint;
    preview(&ctx, &mut app);
    approve(&ctx, &mut app);
    assert!(app.capability_ui.is_none());
    assert_eq!(app.project.language_version(), "1.13");
    assert!(!app.project.root.exists(), "启用不应创建或保存目录");
    app.undo(false);
    assert_eq!(app.project.language_version(), "1.9");
    assert_eq!(
        app.snapshot
            .as_ref()
            .unwrap()
            .result
            .options
            .language_version,
        LanguageVersion::V1_9
    );
    assert_eq!(app.project.sources(), sources);
    assert_eq!(app.project.compile().analysis.fingerprint, fingerprint);
    assert!(app.project.required_features().is_empty());
    app.undo(true);
    assert_eq!(
        app.snapshot
            .as_ref()
            .unwrap()
            .result
            .options
            .language_version,
        LanguageVersion::V1_13
    );
    for (kind, name) in [
        ("place", "高山站"),
        ("organization", "值守队"),
        ("item", "手抄电报"),
    ] {
        app.edit_entity(None);
        let form = app.entity_editor.as_mut().unwrap();
        form.draft.entity_type = kind.into();
        form.draft.display = name.into();
        let id = form.draft.id.clone();
        click(&ctx, &mut app, "应用资料");
        assert!(app.entity_editor.is_none(), "{:?}", app.io_error);
        assert_eq!(
            app.snapshot
                .as_ref()
                .unwrap()
                .result
                .analysis
                .catalog
                .entities[&id]
                .entity_type,
            kind
        );
    }
    assert!(!app.project.root.exists());
}

#[test]
fn hidden_dirty_input_blocks_upgrade_and_is_preserved() {
    let (ctx, mut app) = app();
    app.new_event(None);
    app.event_editor.as_mut().unwrap().draft.body = "坏稿 {\n".into();
    let baseline = app.project.content_baseline();
    preview(&ctx, &mut app);
    assert!(app.capability_ui.as_ref().unwrap().plan.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.event_editor.as_ref().unwrap().draft.body, "坏稿 {\n");
    click(&ctx, &mut app, "取消 / 保留当前设置");
    assert_eq!(app.project.language_version(), "1.9");
}

#[test]
fn stale_preview_never_reuses_approval() {
    let (ctx, mut app) = app();
    preview(&ctx, &mut app);
    let path = app.project.entry.clone();
    app.project
        .set_text(&path, "event start\n  新正文。\n  -> END\n".into())
        .unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    approve(&ctx, &mut app);
    assert_eq!(app.project.language_version(), "1.9");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.capability_ui.as_ref().unwrap().plan.is_none());
}

#[test]
fn new_entity_and_relation_on_19_open_capability_choice_before_creating_drafts() {
    let (_, mut app) = app();
    let baseline = app.project.content_baseline();
    app.edit_entity(None);
    assert!(app.capability_ui.is_some());
    assert!(app.entity_editor.is_none());
    assert!(app.capability_blockers().is_empty());
    app.capability_ui = None;
    app.edit_relation_type(None);
    assert!(app.capability_ui.is_some());
    assert!(app.relation_type_editor.is_none());
    app.capability_ui = None;
    app.edit_relation(None, None);
    assert!(app.capability_ui.is_some());
    assert!(app.relation_editor.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
}

fn keyboard_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::TopBottomPanel::top("capability-keyboard-background").show(ctx, |ui| {
                let response = ui.button("后台工具栏");
                if app.capability_ui.is_none() {
                    response.request_focus();
                }
            });
            app.capability_window(ctx);
        },
    )
}

fn press_key(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    key: egui::Key,
) -> Vec<egui::output::OutputEvent> {
    let mut events = Vec::new();
    for pressed in [true, false] {
        let output = keyboard_frame(
            ctx,
            app,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        events.extend(output.platform_output.events);
    }
    events
}

fn tab_to(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for _ in 0..24 {
        let events = press_key(ctx, app, egui::Key::Tab);
        if events.iter().any(|event| {
            matches!(event,
            egui::output::OutputEvent::FocusGained(info) if info.label.as_deref() == Some(label))
        }) {
            return;
        }
    }
    panic!("键盘 Tab 未到达 {label}");
}

#[test]
fn opening_focuses_language_and_keyboard_can_select_preview_then_cancel_without_writes() {
    let (ctx, mut app) = app();
    let sources = app.project.sources();
    let baseline = app.project.content_baseline();
    let fingerprint = app.project.compile().analysis.fingerprint;
    keyboard_frame(&ctx, &mut app, Vec::new());
    let background = ctx.memory(|memory| memory.focused()).unwrap();

    app.open_capabilities();
    keyboard_frame(&ctx, &mut app, Vec::new());
    let language = ctx
        .memory(|memory| memory.focused())
        .expect("新窗口须把焦点移入版本选择");
    assert_ne!(language, background);
    assert!(!egui::ComboBox::is_open(&ctx, language));
    press_key(&ctx, &mut app, egui::Key::Enter);
    assert!(
        egui::ComboBox::is_open(&ctx, language),
        "首个 Enter 应打开版本列表"
    );
    let title = language_capabilities()
        .iter()
        .find(|capability| capability.version == LanguageVersion::V1_13)
        .unwrap()
        .title;
    tab_to(&ctx, &mut app, title);
    press_key(&ctx, &mut app, egui::Key::Enter);
    assert_eq!(
        app.capability_ui.as_ref().unwrap().target,
        LanguageVersion::V1_13
    );
    assert!(!egui::ComboBox::is_open(&ctx, language));
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(language));

    tab_to(&ctx, &mut app, "预览全稿兼容影响");
    press_key(&ctx, &mut app, egui::Key::Enter);
    assert!(
        app.capability_ui
            .as_ref()
            .unwrap()
            .plan
            .as_ref()
            .unwrap()
            .can_apply
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(!app.capability_ui.as_ref().unwrap().acknowledged);
    tab_to(&ctx, &mut app, "取消 / 保留当前设置");
    press_key(&ctx, &mut app, egui::Key::Enter);
    assert!(app.capability_ui.is_none());
    assert_eq!(app.project.language_version(), "1.9");
    assert_eq!(app.project.sources(), sources);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.compile().analysis.fingerprint, fingerprint);
    assert!(app.history.is_empty());
    assert!(!app.project.root.exists());

    app.open_capabilities();
    keyboard_frame(&ctx, &mut app, Vec::new());
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(language));
}
