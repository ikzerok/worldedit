//! 只读、陈旧守卫、确认/取消及窗口重复绘制的离屏回归。
use super::*;
use crate::app::{SavedReplayPath, WorldeditApp};
use worldline_core::project::Project;
use worldline_runtime::{ReplayTrace, Story};

const SOURCE: &str = "let private_balance = 7\nevent start\n  雾港的钟声。\n  choice \"进入港口\"\n    set private_balance = 7\n    -> finish\nevent finish\n  帷幕落下。\n  -> END\n";

fn setup() -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = std::env::temp_dir().join(format!(
        "playthrough-ui-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.project = Project::new(&root);
    app.active_file = app.project.entry.clone();
    app.project
        .documents
        .retain(|path, _| path == &app.active_file);
    app.project
        .set_text(&app.active_file.clone(), SOURCE.into())
        .unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    let result = &app.snapshot.as_ref().unwrap().result;
    let mut story = Story::new_with_seed(&result.program, &result.analysis, 31).unwrap();
    story.continue_story().unwrap();
    story.choose(0).unwrap();
    story.continue_story().unwrap();
    app.replay_debugger.saved_paths.push(SavedReplayPath {
        name: "进港路线".into(),
        trace: story.replay_trace(),
    });
    app.playthrough_report.route = ReportRoute::Saved(0);
    app.playthrough_report.open = true;
    app.start_play();
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .continue_story()
        .unwrap();
    (ctx, app)
}
fn invariant(app: &WorldeditApp) -> (String, bool, String, ReplayTrace, Option<usize>) {
    let story = app.play.as_ref().unwrap().story.as_ref().unwrap();
    (
        app.project.content_baseline(),
        app.project.is_dirty(),
        story.save().unwrap(),
        story.replay_trace(),
        app.replay_debugger.selected_path,
    )
}
fn generate(ctx: &egui::Context, app: &mut WorldeditApp) {
    app.begin_playthrough_report(ctx);
    let start = std::time::Instant::now();
    while app.playthrough_report.job.is_some() {
        assert!(start.elapsed() < std::time::Duration::from_secs(10));
        app.poll_playthrough_report(ctx);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        app.playthrough_report.reviewed.is_some(),
        "{:?}",
        app.playthrough_report.notice
    );
}
fn window(ctx: &egui::Context, app: &mut WorldeditApp) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1040.0, 660.0),
            )),
            ..Default::default()
        },
        |ctx| app.playthrough_report_window(ctx),
    )
}

#[test]
fn preview_copy_and_repeated_frames_do_not_change_live_story_or_project() {
    let (ctx, mut app) = setup();
    let before = invariant(&app);
    generate(&ctx, &mut app);
    let report = &app.playthrough_report.reviewed.as_ref().unwrap().report;
    assert!(report.complete && report.ended);
    assert!(report.markdown.contains("雾港的钟声"));
    assert!(!report.markdown.contains("private_balance"));
    assert!(app.checked_report_markdown().is_err());
    app.copy_playthrough_report(&ctx);
    assert!(app
        .playthrough_report
        .notice
        .as_deref()
        .unwrap()
        .contains("确认"));
    app.playthrough_report.privacy_confirmed = true;
    assert!(app.checked_report_markdown().is_ok());
    app.copy_playthrough_report(&ctx);
    for _ in 0..4 {
        window(&ctx, &mut app);
    }
    assert_eq!(invariant(&app), before);
    assert!(app.playthrough_report.job.is_none());
    let first = app
        .playthrough_report
        .reviewed
        .as_ref()
        .unwrap()
        .report
        .observations
        .clone();
    generate(&ctx, &mut app);
    assert_eq!(
        app.playthrough_report
            .reviewed
            .as_ref()
            .unwrap()
            .report
            .observations,
        first
    );
    assert_eq!(invariant(&app), before);
}

#[test]
fn live_route_capture_is_partial_and_does_not_change_recorded_selection() {
    let (ctx, mut app) = setup();
    app.playthrough_report.route = ReportRoute::Live;
    let before = invariant(&app);
    generate(&ctx, &mut app);
    let report = &app.playthrough_report.reviewed.as_ref().unwrap().report;
    assert!(!report.complete);
    assert!(view::status_text(report).contains("非完整结局"));
    assert_eq!(invariant(&app), before);
}

#[test]
fn comment_only_edits_and_uncompiled_baseline_changes_block_delivery() {
    let (ctx, mut app) = setup();
    generate(&ctx, &mut app);
    app.playthrough_report.privacy_confirmed = true;
    let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
    app.project
        .set_text(&app.active_file.clone(), format!("// comment\n{SOURCE}"))
        .unwrap();
    assert!(app.checked_report_markdown().unwrap_err().contains("过期"));
    app.recompile();
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
    assert!(app.checked_report_markdown().unwrap_err().contains("过期"));
    generate(&ctx, &mut app);
    app.playthrough_report.privacy_confirmed = true;
    assert!(app.checked_report_markdown().is_ok());
    app.playthrough_report.max_steps = 123;
    assert!(app.checked_report_markdown().unwrap_err().contains("预算"));
    app.playthrough_report.max_steps = 100_000;
    app.playthrough_report.route = ReportRoute::Live;
    assert!(app
        .checked_report_markdown()
        .unwrap_err()
        .contains("路径选择"));
}

#[test]
fn unapplied_drafts_require_scope_decision_and_remain_intact() {
    let (ctx, mut app) = setup();
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    buffer.replace_source(format!("{SOURCE}\n// kept draft"));
    app.manuscript.restore_writing_buffers(&[buffer]);
    app.begin_playthrough_report(&ctx);
    assert!(app.playthrough_report.job.is_none());
    assert!(app
        .playthrough_report
        .notice
        .as_deref()
        .unwrap()
        .contains("草稿"));
    let before = invariant(&app);
    app.playthrough_report.confirmed_inputs = app.unapplied_play_inputs();
    app.playthrough_report.scope_confirmed = true;
    generate(&ctx, &mut app);
    assert_eq!(
        app.playthrough_report
            .reviewed
            .as_ref()
            .unwrap()
            .scope
            .excluded_inputs
            .len(),
        1
    );
    assert!(app.manuscript.writing_buffers()[0]
        .source()
        .contains("kept draft"));
    assert_eq!(invariant(&app), before);
}

#[test]
fn close_reset_and_applied_changes_cancel_pending_jobs_without_late_results() {
    let (ctx, mut app) = setup();
    let before = invariant(&app);
    app.begin_playthrough_report(&ctx);
    let token = app
        .playthrough_report
        .job
        .as_ref()
        .unwrap()
        .cancellation
        .clone();
    app.close_playthrough_report();
    assert!(token.is_cancelled());
    app.poll_playthrough_report(&ctx);
    assert!(app.playthrough_report.reviewed.is_none());
    assert_eq!(invariant(&app), before);
    app.begin_playthrough_report(&ctx);
    let token = app
        .playthrough_report
        .job
        .as_ref()
        .unwrap()
        .cancellation
        .clone();
    app.project
        .set_text(&app.active_file.clone(), format!("{SOURCE}\n// changed"))
        .unwrap();
    app.poll_playthrough_report(&ctx);
    assert!(token.is_cancelled());
    assert!(app.playthrough_report.job.is_none());
    app.recompile();
    app.begin_playthrough_report(&ctx);
    let token = app
        .playthrough_report
        .job
        .as_ref()
        .unwrap()
        .cancellation
        .clone();
    app.reset_views();
    assert!(token.is_cancelled());
    assert!(app.playthrough_report.reviewed.is_none());
}

#[test]
fn copy_and_save_recheck_external_disk_changes_without_refresh_or_overwrite() {
    let (ctx, mut app) = setup();
    app.project.save().unwrap();
    app.recompile();
    generate(&ctx, &mut app);
    app.playthrough_report.privacy_confirmed = true;
    let before = invariant(&app);
    std::fs::write(&app.active_file, format!("{SOURCE}\n// external")).unwrap();
    assert!(app.checked_report_markdown().unwrap_err().contains("过期"));
    app.copy_playthrough_report(&ctx);
    assert!(app
        .playthrough_report
        .notice
        .as_deref()
        .unwrap()
        .contains("过期"));
    assert_eq!(invariant(&app), before);
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[path = "keyboard_tests.rs"]
mod keyboard;
