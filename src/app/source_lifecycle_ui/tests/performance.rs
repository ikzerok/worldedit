//! release CPU 探针：真实路径表单回调和空闲窗口，不代表原生输入/GPU/present 验收。
use super::*;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SOURCE_FILES: usize = 501;
const FORMAL_PATHS: usize = 1001;
const IDLE_SAMPLES: usize = 30;
const DESTINATION: &str = "drafts/deep/new.wl";

/// 与 evidence/measure-source-lifecycle.py 相同规模和正文，不复用上一轮工程。
fn large_app(round: usize) -> (egui::Context, WorldeditApp, PathBuf) {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "source-move-full-ui-{}-{nonce}-{round}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("assets")).unwrap();
    std::fs::create_dir_all(root.join("lore")).unwrap();
    std::fs::write(root.join("assets/resource.txt"), b"unchanged asset bytes\n").unwrap();
    let mut entry = String::new();
    for id in 0..500 {
        entry.push_str(&format!("include \"lore/chapter_{id:03}.wl\"\n"));
        let mut text = format!(
            "event part_{id}\n  [[file:chapter_250.wl|普通字面 lore/chapter_250.wl]]\n  -> END\n"
        );
        if id == 250 {
            text.insert_str(
                0,
                "asset picture file \"../assets/resource.txt\" as \"附件\"\n",
            );
        }
        std::fs::write(root.join(format!("lore/chapter_{id:03}.wl")), text).unwrap();
    }
    entry.push_str("event start\n  开始\n  -> END\n");
    std::fs::write(root.join("world.wl"), entry).unwrap();
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, Some(root.join("world.wl")));
    assert_eq!(app.project.root, root);
    assert_eq!(app.project.documents.len(), SOURCE_FILES);
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    let source = root.join("lore/chapter_250.wl");
    app.active_file = source.clone();
    (ctx, app, source)
}

fn window_frame(ctx: &egui::Context, app: &mut WorldeditApp, frame: &mut u64) -> Duration {
    *frame += 1;
    let start = Instant::now();
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1188.0, 848.0),
            )),
            time: Some(*frame as f64 / 60.0),
            ..Default::default()
        },
        |ctx| app.source_move_window(ctx),
    );
    start.elapsed()
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

fn sample_summary(mut samples: Vec<Duration>) -> serde_json::Value {
    samples.sort_unstable();
    serde_json::json!({
        "samples": samples.len(),
        "p50_ms": milliseconds(samples[samples.len() / 2]),
        "p95_ms": milliseconds(samples[(samples.len() * 95).div_ceil(100) - 1]),
        "max_ms": milliseconds(*samples.last().unwrap()),
    })
}

fn current_form(app: &WorldeditApp) -> bool {
    let form = app.source_move_form.as_ref().expect("路径表单必须保留");
    form.root == app.project.root
        && form
            .plan
            .as_ref()
            .is_some_and(|plan| plan.content_baseline == app.project.content_baseline())
}

/// 五份新工程，各有 501 源码、500 include、500 正式正文链接和 1 附件路径。
/// preview/apply 直接调用窗口使用的同一 UI 方法；完整 apply 计时包含 clone、
/// core 完整重验、remember、阅读位置迁移、manuscript rebase 和 recompile。
/// 空闲样本调用真实 source_move_window，包含其每帧 current/content_baseline 检查；
/// 另列独立 baseline 样本帮助定位，但不从窗口时间扣除，也不声称原生 GUI 响应性。
#[test]
#[ignore = "release-only five-fresh-fixture source move UI responsiveness evidence"]
#[allow(clippy::assertions_on_constants)]
fn release_source_move_501_full_ui_five_fresh_fixtures() {
    assert!(
        !cfg!(debug_assertions),
        "必须使用 release，不接受 debug 性能结果"
    );
    let mut failures = Vec::new();
    for round in 1..=5 {
        let setup = Instant::now();
        let (ctx, mut app, source) = large_app(round);
        let setup_elapsed = setup.elapsed();
        let root = app.project.root.clone();
        let destination = root.join(DESTINATION);
        let original_text = app.project.document(&source).unwrap().to_owned();
        let original_sources = app.project.sources();
        let baseline = app.project.content_baseline();
        let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
        let version = app.version;
        let generation = app.map_revision.content_generation;
        app.reading_return = Some((source.clone(), 2));
        app.personal.history.push(crate::app::personal::Location {
            file: source.clone(),
            ..Default::default()
        });

        let begin = Instant::now();
        app.begin_source_move(source.clone());
        let begin_elapsed = begin.elapsed();
        let mut form = app.source_move_form.take().unwrap();
        form.destination = DESTINATION.into();
        let preview = Instant::now();
        app.preview_source_move(&mut form);
        let preview_elapsed = preview.elapsed();
        assert!(form.error.is_none(), "{:?}", form.error);
        let plan = form.plan.as_ref().unwrap();
        assert_eq!(plan.resources.len(), FORMAL_PATHS);
        assert_eq!(plan.changes.len(), SOURCE_FILES);
        assert_eq!(
            plan.runtime_fingerprint_before,
            plan.runtime_fingerprint_after
        );
        assert_eq!(plan.entry_before, plan.entry_after);
        let changed_occurrences: usize = plan
            .changes
            .iter()
            .map(|change| change.occurrences.len())
            .sum();
        assert_eq!(changed_occurrences, 502);
        assert_eq!(app.project.content_baseline(), baseline);
        assert!(app.history.is_empty());
        assert!(!app.project.is_dirty());
        app.source_move_form = Some(form);
        assert!(current_form(&app));

        let mut frame = 0;
        let cold_window = window_frame(&ctx, &mut app, &mut frame);
        for _ in 0..3 {
            window_frame(&ctx, &mut app, &mut frame);
        }
        let mut idle_samples = Vec::new();
        let mut baseline_samples = Vec::new();
        for _ in 0..IDLE_SAMPLES {
            idle_samples.push(window_frame(&ctx, &mut app, &mut frame));
            let start = Instant::now();
            let current = std::hint::black_box(app.project.content_baseline());
            baseline_samples.push(start.elapsed());
            assert_eq!(current, baseline);
        }
        assert!(current_form(&app));
        assert_eq!(
            app.source_move_form.as_ref().unwrap().destination,
            DESTINATION
        );
        assert_eq!(app.project.content_baseline(), baseline);

        // 表单打开时改变当前工程：窗口拒绝旧基线，直接调用 apply 也必须无损拒绝。
        let edited_text = format!("{original_text}// 预览后保留的当前修改\n");
        app.project.set_text(&source, edited_text.clone()).unwrap();
        let stale_baseline = app.project.content_baseline();
        assert!(!current_form(&app));
        let stale_window = window_frame(&ctx, &mut app, &mut frame);
        let mut form = app.source_move_form.take().unwrap();
        let stale = Instant::now();
        let stale_applied = app.apply_source_move(&mut form);
        let stale_elapsed = stale.elapsed();
        assert!(!stale_applied);
        assert!(form.error.is_some());
        assert_eq!(form.destination, DESTINATION);
        assert_eq!(app.project.content_baseline(), stale_baseline);
        assert_eq!(app.project.document(&source), Ok(edited_text.as_str()));
        assert!(app.history.is_empty());
        assert!(!destination.exists());

        // 撤回测试修改并按真实表单重新预览；不绕过 UI 的当前工程/输入检查。
        app.project
            .set_text(&source, original_text.clone())
            .unwrap();
        assert_eq!(app.project.content_baseline(), baseline);
        let repreview = Instant::now();
        app.preview_source_move(&mut form);
        let repreview_elapsed = repreview.elapsed();
        assert!(form.error.is_none(), "{:?}", form.error);
        assert_eq!(form.plan.as_ref().unwrap().content_baseline, baseline);
        let apply = Instant::now();
        let applied = app.apply_source_move(&mut form);
        let apply_elapsed = apply.elapsed();
        assert!(applied, "{:?}", form.error);
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        assert_eq!(app.history.len(), 1);
        assert_eq!(app.history[0].content_baseline(), baseline);
        assert_eq!(app.version, version + 1, "必须包含完整 recompile");
        assert_eq!(app.map_revision.content_generation, generation + 1);
        assert_eq!(
            app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
            fingerprint
        );
        assert_eq!(app.active_file, destination);
        assert_eq!(app.personal.history[0].file, destination);
        assert_eq!(app.reading_return, Some((destination.clone(), 2)));
        assert_eq!(app.tab, Tab::Edit);
        assert!(app.project.is_dirty());
        assert_eq!(std::fs::read_to_string(&source).unwrap(), original_text);
        assert!(!destination.exists(), "内存应用不能冒称已保存");
        assert_eq!(
            std::fs::read(root.join("assets/resource.txt")).unwrap(),
            b"unchanged asset bytes\n"
        );
        for (path, document) in &app.project.documents {
            if path != &app.project.entry && !document.is_deleted() {
                assert!(document.text.contains("普通字面 lore/chapter_250.wl"));
            }
        }
        let applied_baseline = app.project.content_baseline();
        let undo = Instant::now();
        app.undo(false);
        let undo_elapsed = undo.elapsed();
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        // restore 保留新路径的删除墓碑，基线可变；撤销契约是完整活动源码与语义恢复。
        assert_eq!(app.project.sources(), original_sources);
        assert_eq!(app.project.document(&source), Ok(original_text.as_str()));
        assert!(app.project.document(&destination).is_err());
        assert_eq!(
            app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
            fingerprint
        );
        assert!(app.history.is_empty());
        assert_eq!(app.redo.len(), 1);
        let redo = Instant::now();
        app.undo(true);
        let redo_elapsed = redo.elapsed();
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        assert_eq!(app.project.content_baseline(), applied_baseline);
        assert!(app.project.document(&destination).is_ok());
        assert_eq!(app.history.len(), 1);
        assert!(app.redo.is_empty());

        let idle = sample_summary(idle_samples);
        let baseline_cost = sample_summary(baseline_samples);
        println!(
            "source_move_ui {}",
            serde_json::json!({
                "round": round, "source_files": SOURCE_FILES, "formal_paths": FORMAL_PATHS,
                "changed_files": SOURCE_FILES, "changed_occurrences": changed_occurrences,
                "setup_ms": milliseconds(setup_elapsed), "begin_ms": milliseconds(begin_elapsed),
                "preview_main_ms": milliseconds(preview_elapsed),
                "repreview_main_ms": milliseconds(repreview_elapsed),
                "apply_full_main_ms": milliseconds(apply_elapsed),
                "stale_apply_reject_main_ms": milliseconds(stale_elapsed),
                "cold_window_ms": milliseconds(cold_window), "stale_window_ms": milliseconds(stale_window),
                "idle_window": idle, "content_baseline_only": baseline_cost,
                "undo_ms": milliseconds(undo_elapsed), "redo_ms": milliseconds(redo_elapsed),
                "current_form_preserved": true, "stale_guard_preserved_input": true,
                "undo_redo_success": true, "remember_and_recompile_verified": true,
                "applied_unsaved": true, "runtime_fingerprint_unchanged": true,
                "method": "release CPU; direct UI callbacks; simulated egui idle window; no native GPU/present evidence",
                "gate": {"main_callback_ms": 250, "idle_window_p95_ms": 33},
            })
        );
        for (segment, elapsed) in [
            ("preview", preview_elapsed),
            ("repreview", repreview_elapsed),
            ("apply", apply_elapsed),
            ("stale_apply_reject", stale_elapsed),
            ("undo", undo_elapsed),
            ("redo", redo_elapsed),
        ] {
            if elapsed > Duration::from_millis(250) {
                failures.push(format!(
                    "round={round} {segment}={:.3}ms > 250ms",
                    milliseconds(elapsed)
                ));
            }
        }
        let idle_p95 = idle["p95_ms"].as_f64().unwrap();
        if idle_p95 > 33.0 {
            failures.push(format!("round={round} idle p95={idle_p95:.3}ms > 33ms"));
        }
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }
    // 完成五份测量后统一报告门槛，避免第一份超时掩盖后续新工程样本。
    assert!(
        failures.is_empty(),
        "前台响应性门槛未通过：{}",
        failures.join("; ")
    );
}
