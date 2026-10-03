//! 受控receiver模拟尚未结束的native编译，证明取消不等于已释放后台槽。
use super::tests::app;
use super::*;

fn tick(ctx: &egui::Context, app: &mut WorldeditApp, now: f64) {
    let _ = ctx.run(
        egui::RawInput {
            time: Some(now),
            ..Default::default()
        },
        |ctx| app.poll_problems(ctx),
    );
}
fn hold_job(
    app: &mut WorldeditApp,
) -> (
    schedule::ReportTicket,
    std::sync::mpsc::Sender<Result<ProblemsReport, String>>,
) {
    app.problems.schedule.retry(0.);
    let ticket = app
        .problems
        .schedule
        .begin(
            app.project.content_baseline(),
            app.project.problems_observation_key().unwrap(),
        )
        .unwrap();
    let (job, sender) = job::ProblemsJob::controlled(ticket.clone());
    app.problems.job = Some(job);
    app.problems.invalidated = true;
    (ticket, sender)
}

#[test]
fn repeated_invalidations_keep_exactly_one_unfinished_slot_and_only_latest_pending_version() {
    let (ctx, mut app) = app();
    let (ticket, sender) = hold_job(&mut app);
    for step in 1..=100 {
        app.version += 1;
        tick(&ctx, &mut app, step as f64);
        let retained = app.problems.job.as_ref().unwrap();
        assert_eq!(retained.ticket, ticket, "旧编译未结束前不能并行替换slot");
        assert!(retained.is_cancelled());
    }
    let latest = app.version;
    tick(&ctx, &mut app, 101.);
    assert_eq!(app.problems.job.as_ref().unwrap().ticket, ticket);
    drop(sender); // 原生任务真正退出才关闭receiver。
    tick(&ctx, &mut app, 102.);
    let next = &app.problems.job.as_ref().unwrap().ticket;
    assert_eq!(next.version, latest);
    assert_ne!(next.generation, ticket.generation);
}

#[test]
fn explicit_cancel_retains_slot_until_terminal_and_does_not_restart() {
    let (ctx, mut app) = app();
    let (ticket, sender) = hold_job(&mut app);
    app.cancel_problems();
    tick(&ctx, &mut app, 2.);
    assert_eq!(app.problems.job.as_ref().unwrap().ticket, ticket);
    assert!(app.problems.cancelling());
    let mut old_report = (**app.problems.report.as_ref().unwrap()).clone();
    old_report.language_version = "不能接管的旧结果".into();
    sender.send(Ok(old_report)).unwrap();
    tick(&ctx, &mut app, 3.);
    assert!(app.problems.job.is_none());
    assert!(app.problems.error.as_ref().unwrap().contains("已取消"));
    assert_ne!(
        app.problems.report.as_ref().unwrap().language_version,
        "不能接管的旧结果"
    );
    tick(&ctx, &mut app, 10.);
    assert!(app.problems.job.is_none(), "旧终态不能复活已取消的任务");
}

#[test]
fn explicit_retry_waits_for_old_terminal_then_starts_one_latest_task() {
    let (ctx, mut app) = app();
    let (ticket, sender) = hold_job(&mut app);
    app.cancel_problems();
    app.retry_problems(&ctx);
    for now in [1., 2., 3.] {
        tick(&ctx, &mut app, now);
        assert_eq!(app.problems.job.as_ref().unwrap().ticket, ticket);
    }
    drop(sender);
    tick(&ctx, &mut app, 4.);
    let next = app.problems.job.as_ref().unwrap();
    assert_ne!(next.ticket.generation, ticket.generation);
    assert_eq!(next.ticket.version, app.version);
    assert!(!next.is_cancelled());
}

#[test]
fn workspace_reset_clears_results_but_retires_the_existing_native_slot() {
    let (ctx, mut app) = app();
    let (ticket, sender) = hold_job(&mut app);
    app.reset_views();
    assert!(app.problems.report.is_none());
    assert!(app.problems.selected.is_none());
    assert_eq!(app.problems.job.as_ref().unwrap().ticket, ticket);
    assert!(app.problems.cancelling());
    tick(&ctx, &mut app, 1.);
    assert_eq!(app.problems.job.as_ref().unwrap().ticket, ticket);
    app.cancel_problems();
    drop(sender);
    tick(&ctx, &mut app, 2.);
    assert!(app.problems.job.is_none());
    assert!(app.problems.report.is_none());
}
