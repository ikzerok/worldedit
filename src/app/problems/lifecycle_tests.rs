//! 生命周期证据：模拟 egui 事件，只证明程序路由，不能替代真实输入法。
use super::tests::{app, frame, key};
use super::*;
use crate::app::Tab;
use egui::Key;

#[test]
fn narrow_escape_returns_to_list_then_closes_with_prior_focus() {
    let (ctx, mut app) = app();
    let original = egui::Id::new("before-problems");
    ctx.memory_mut(|memory| memory.request_focus(original));
    app.open_problems(&ctx);
    frame(&ctx, &mut app, egui::vec2(1040., 660.), vec![]);
    app.problems.narrow_detail = true;
    frame(
        &ctx,
        &mut app,
        egui::vec2(1040., 660.),
        vec![key(Key::Escape)],
    );
    assert!(app.personal.settings.diagnostics);
    assert!(!app.problems.narrow_detail);
    frame(
        &ctx,
        &mut app,
        egui::vec2(1040., 660.),
        vec![key(Key::Escape)],
    );
    assert!(!app.personal.settings.diagnostics);
    // egui可以在下一帧注销一个未绘制的widget；只断言返回路由保存了正确焦点。
    assert_eq!(app.problems.return_focus, Some(original));
}

#[test]
fn ime_events_and_filter_text_focus_do_not_take_list_arrow_or_enter() {
    let (ctx, mut app) = app();
    app.open_problems(&ctx);
    frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    frame(
        &ctx,
        &mut app,
        egui::vec2(1280., 800.),
        vec![
            egui::Event::Ime(egui::ImeEvent::Preedit("未完成".into())),
            key(Key::ArrowDown),
            key(Key::Enter),
        ],
    );
    assert!(app.problems.selected.is_none());
    assert_eq!(app.tab, Tab::Manuscript);
    frame(
        &ctx,
        &mut app,
        egui::vec2(1280., 800.),
        vec![
            egui::Event::Ime(egui::ImeEvent::Commit("完整".into())),
            key(Key::ArrowDown),
        ],
    );
    assert!(app.problems.selected.is_none());
    ctx.memory_mut(|memory| memory.request_focus(egui::Id::new("ordinary-input")));
    app.problems_shortcuts(&ctx);
    assert!(app.problems.selected.is_none());
}

#[test]
fn same_buffer_version_with_new_observation_invalidates_old_ticket() {
    let mut schedule = schedule::ReportSchedule::default();
    schedule.observe(9, 0.);
    let previous = schedule
        .begin("same-buffer".into(), "asset-present".into())
        .unwrap();
    assert!(schedule.accepts(&previous, 9, "same-buffer", "asset-present"));
    assert!(!schedule.accepts(&previous, 9, "same-buffer", "asset-deleted"));
    schedule.invalidate(9, 1.);
    assert!(!schedule.accepts(&previous, 9, "same-buffer", "asset-present"));
    assert!(!schedule.ready(1.24));
    assert!(schedule.ready(1.25));
    let replacement = schedule
        .begin("same-buffer".into(), "asset-deleted".into())
        .unwrap();
    assert_ne!(previous.generation, replacement.generation);
}

#[test]
fn unavailable_observation_never_labels_cached_report_as_current() {
    let (_, mut app) = app();
    assert!(!app.problems.stale(app.version));
    app.problems.observation_failed = true;
    assert!(app.problems.stale(app.version));
    app.problems.observation_failed = false;
    app.problems.observation = Some("different-observation".into());
    assert!(app.problems.stale(app.version));
}

#[test]
fn reset_workspace_cancels_result_and_does_not_inherit_query_or_selection() {
    let (ctx, mut app) = app();
    app.open_problems(&ctx);
    app.problems.query.text = "keeper".into();
    app.problems
        .select(app.problems.report.as_ref().unwrap().entries[0].id.clone());
    app.reset_views();
    assert!(app.problems.report.is_none());
    assert!(app.problems.selected.is_none());
    assert_eq!(app.problems.query, Default::default());
}
