//! 完整 raw-input → App::update；区别于单页离屏测试，不冒称物理 IME。
use super::*;
pub(super) fn native_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
    close: bool,
) -> egui::FullOutput {
    let mut raw = RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1600.0, 1100.0))),
        events,
        ..Default::default()
    };
    if close {
        raw.viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .events
            .push(egui::ViewportEvent::Close);
    }
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest())
    })
}
fn position(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) -> egui::Pos2 {
    for _ in 0..3 {
        native_frame(ctx, app, vec![], false);
    }
    let output = native_frame(ctx, app, vec![], false);
    output
        .shapes
        .iter()
        .find_map(|shape| {
            text_position(&shape.shape, label).filter(|point| shape.clip_rect.contains(*point))
        })
        .unwrap_or_else(|| panic!("没有显示按钮 {label}"))
}
fn gesture(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    point: egui::Pos2,
    event: Option<egui::ImeEvent>,
) {
    for pressed in [true, false] {
        let mut events = vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        if pressed {
            if let Some(event) = &event {
                events.insert(0, Event::Ime(event.clone()));
            }
        }
        native_frame(ctx, app, events, false);
    }
}
pub(super) fn native_click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let point = position(ctx, app, label);
    gesture(ctx, app, point, None);
}
pub(super) fn setup() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app();
    app.tab = Tab::Templates;
    native_click(&ctx, &mut app, "新建空白模板");
    native_click(&ctx, &mut app, "高级 JSON");
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("template-manager-json")));
    native_frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Enabled)],
        false,
    );
    (ctx, app)
}
#[test]
fn template_native_raw_hook_keeps_candidate_owner_when_global_navigation_is_clicked() {
    let (ctx, mut app) = setup();
    let point = position(&ctx, &mut app, "书稿工作台");
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("template-manager-json")));
    gesture(
        &ctx,
        &mut app,
        point,
        Some(egui::ImeEvent::Preedit("候选原文".into())),
    );
    assert_eq!(app.tab, Tab::Templates);
    assert!(app.template_manager.editor.contains("候选原文"));
    assert!(app.template_manager.composition_busy());
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        Some(egui::Id::new("template-manager-json"))
    );
    native_frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Commit("提交原文".into()))],
        false,
    );
    assert!(app.template_manager.editor.contains("提交原文"));
    assert_eq!(app.tab, Tab::Templates);
}
#[test]
fn template_native_first_composition_frame_cannot_trigger_new_world_menu_action() {
    let (ctx, mut app) = setup();
    native_click(&ctx, &mut app, "工程");
    let point = position(&ctx, &mut app, "新建世界");
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("template-manager-json")));
    let root = app.project.root.clone();
    gesture(
        &ctx,
        &mut app,
        point,
        Some(egui::ImeEvent::Preedit("暂存输入".into())),
    );
    assert_eq!(app.project.root, root);
    assert!(app.draft_action.is_none());
    assert!(!app.allow_close);
    assert!(app.template_manager.has_unsubmitted_work());
}
#[test]
fn template_native_close_request_during_first_preedit_preserves_work_and_refuses_close() {
    let (ctx, mut app) = setup();
    let root = app.project.root.clone();
    native_frame(
        &ctx,
        &mut app,
        vec![Event::Ime(egui::ImeEvent::Preedit("关闭前候选".into()))],
        true,
    );
    assert!(!app.allow_close);
    assert_eq!(app.project.root, root);
    assert!(app.template_manager.has_unsubmitted_work());
    assert!(app.template_manager.editor.contains("关闭前候选"));
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        Some(egui::Id::new("template-manager-json"))
    );
}
