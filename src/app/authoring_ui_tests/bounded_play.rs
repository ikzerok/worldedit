//! 普通试玩的有界推进回归；离屏 egui 帧不替代原生验收。
use super::*;
use worldline_runtime::ContinuationOutcome;

fn bounded_app(source: &str, steps: u64) -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = app();
    app.project
        .set_text(&app.active_file.clone(), source.into())
        .unwrap();
    app.recompile();
    app.replay_debugger.live_max_steps = steps;
    app.replay_debugger.live_time_budget_ms = 2_000;
    app.tab = Tab::Play;
    app.start_play();
    (ctx, app)
}

#[test]
fn ordinary_pair_loop_pauses_without_claiming_end_or_auto_resuming() {
    let (ctx, mut app) = bounded_app("event start\n  -> next\nevent next\n  -> start\n", 8);
    let _ = frame(&ctx, &mut app, Vec::new(), 20);
    let play = app.play.as_ref().unwrap();
    assert!(play.paused);
    assert!(!play.ended);
    assert_eq!(
        play.interruption,
        Some(ContinuationOutcome::StepBudgetExceeded)
    );
    let state = play.story.as_ref().unwrap().save().unwrap();
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 20);
    }
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .save()
            .unwrap(),
        state
    );
    let output = frame(&ctx, &mut app, Vec::new(), 20);
    let mut text = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut text);
    }
    assert!(text.contains("达到步数上限"), "{text}");
    assert!(!text.contains("世界线收束"), "{text}");
}

#[test]
fn explicit_continue_preserves_output_once_and_can_raise_budget() {
    let (ctx, mut app) = bounded_app("event start\n  第一行\n  第二行\n  -> END\n", 1);
    let baseline = app.project.content_baseline();
    let _ = frame(&ctx, &mut app, Vec::new(), 20);
    assert_eq!(app.play.as_ref().unwrap().transcript, "第一行");
    app.replay_debugger.live_max_steps = 100;
    click(&ctx, &mut app, 20, "▶ 继续");
    let play = app.play.as_ref().unwrap();
    assert!(play.ended);
    assert!(!play.paused);
    assert_eq!(play.transcript, "第一行\n第二行");
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn stop_is_distinct_from_story_end_and_restart_uses_current_source() {
    let (ctx, mut app) = bounded_app("event start\n  旧稿\n  -> start\n", 2);
    let _ = frame(&ctx, &mut app, Vec::new(), 20);
    click(&ctx, &mut app, 20, "■ 停止");
    assert!(app.play.as_ref().unwrap().stopped);
    assert!(!app.play.as_ref().unwrap().ended);
    let old_version = app.play.as_ref().unwrap().version;
    app.project
        .set_text(
            &app.active_file.clone(),
            "event start\n  新稿\n  -> END\n".into(),
        )
        .unwrap();
    app.recompile();
    assert_eq!(app.play.as_ref().unwrap().version, old_version);
    app.replay_debugger.live_max_steps = 100;
    click(&ctx, &mut app, 20, "↻ 重新开始(应用最新改动)");
    assert_eq!(app.play.as_ref().unwrap().transcript, "新稿");
    assert!(app.play.as_ref().unwrap().ended);
    assert!(!app.play.as_ref().unwrap().stopped);
    assert_eq!(app.play.as_ref().unwrap().version, app.version);
}

#[test]
fn temporal_history_only_gets_entry_hint_when_play_is_explicitly_started() {
    let (ctx, mut app) = bounded_app("period past\nevent record during past\n  历史正文\n", 100);
    assert!(app.snapshot.as_ref().unwrap().result.diagnostics.is_empty());
    assert!(app
        .play
        .as_ref()
        .unwrap()
        .entry_diagnostics
        .iter()
        .any(|d| d.code == "A202"));
    let _ = frame(&ctx, &mut app, Vec::new(), 20);
    assert!(app.play.as_ref().unwrap().ended);
    assert_eq!(app.play.as_ref().unwrap().transcript, "历史正文");
}

#[test]
fn normal_end_status_stays_visible_outside_the_transcript_scroll_area() {
    let (ctx, mut app) = bounded_app("event start\n  完成\n  -> END\n", 100);
    let _ = frame(&ctx, &mut app, Vec::new(), 20);
    let output = frame(&ctx, &mut app, Vec::new(), 20);
    assert!(output.shapes.iter().any(|clipped| {
        text_position_contains(&clipped.shape, "故事已正常结束")
            .is_some_and(|point| clipped.clip_rect.contains(point))
    }));
}
