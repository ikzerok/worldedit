//! 固定 dev 配置下的报告消费者测量；只包含 core 查询和 offscreen 布局。
use super::tests::{app, frame};
use worldline_core::{problems::ProblemQuery, Severity};

fn percentile(values: &mut [f64], percentile: usize) -> f64 {
    values.sort_by(f64::total_cmp);
    values[(values.len() * percentile).div_ceil(100).saturating_sub(1)]
}
fn rss_kib() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|line| line.starts_with("VmRSS:"))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|value| value.parse().ok())
        })
        .unwrap_or(0)
}

#[test]
#[ignore = "独占cargo票后用 --ignored --test-threads=1 测量并保留日志"]
fn five_thousand_problem_consumer_budget() {
    let (ctx, mut app) = app();
    app.open_problems(&ctx);
    for _ in 0..3 {
        frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    }
    let baseline_rss = rss_kib();
    let mut report = (**app.problems.report.as_ref().unwrap()).clone();
    let entry = report.entries[0].clone();
    report.entries = (0..5_000)
        .map(|i| {
            let mut value = entry.clone();
            value.id = format!("scale-{i}");
            value.message = format!("工程问题 {i:04}：中文与 emoji 🧭 相同basename需要完整来源");
            value.severity = if i % 3 == 0 {
                Severity::Error
            } else {
                Severity::Warning
            };
            value
        })
        .collect();
    let mut query_ms = Vec::new();
    for i in 0..10 {
        let query = ProblemQuery {
            text: i.to_string(),
            severities: if i % 2 == 0 {
                vec![Severity::Error]
            } else {
                vec![]
            },
            ..Default::default()
        };
        let start = std::time::Instant::now();
        let page = report.query(&query, None, 200).unwrap();
        if let Some(cursor) = page.next_cursor {
            report.query(&query, Some(&cursor), 200).unwrap();
        }
        query_ms.push(start.elapsed().as_secs_f64() * 1000.);
    }
    app.problems.install(report, app.version);
    frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    let mut frame_ms = Vec::new();
    let mut peak_rss = rss_kib();
    for _ in 0..60 {
        let start = std::time::Instant::now();
        frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
        frame_ms.push(start.elapsed().as_secs_f64() * 1000.);
        peak_rss = peak_rss.max(rss_kib());
        assert!(app.problems.rendered_rows < 40);
    }
    let query_p50 = percentile(&mut query_ms, 50);
    let query_p95 = percentile(&mut query_ms, 95);
    let frame_p50 = percentile(&mut frame_ms, 50);
    let frame_p95 = percentile(&mut frame_ms, 95);
    let delta = peak_rss.saturating_sub(baseline_rss);
    println!("PROBLEMS_UI_BUDGET {{\"problem_count\":5000,\"query_samples\":10,\"layout_samples\":60,\"query_p50_ms\":{query_p50},\"query_p95_ms\":{query_p95},\"layout_p50_ms\":{frame_p50},\"layout_p95_ms\":{frame_p95},\"baseline_rss_kib\":{baseline_rss},\"peak_rss_kib\":{peak_rss},\"delta_rss_kib\":{delta},\"rendered_rows\":{}}}", app.problems.rendered_rows);
    assert!(query_p95 <= 50., "查询p95超过冻结50ms预算");
    assert!(frame_p95 <= 40., "布局p95超过冻结40ms预算");
    assert!(delta <= 128 * 1024, "增量RSS超过128MiB预算");
}

fn hwm_kib() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|line| line.starts_with("VmHWM:"))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|value| value.parse().ok())
        })
        .unwrap_or(0)
}
struct RssSampler {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    peak: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl RssSampler {
    fn start(baseline: usize) -> Self {
        use std::sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        };
        let stop = Arc::new(AtomicBool::new(false));
        let peak = Arc::new(AtomicUsize::new(baseline));
        let worker_stop = stop.clone();
        let worker_peak = peak.clone();
        let thread = std::thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                worker_peak.fetch_max(rss_kib(), Ordering::AcqRel);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        });
        Self {
            stop,
            peak,
            thread: Some(thread),
        }
    }
    fn peak(&self) -> usize {
        self.peak.load(std::sync::atomic::Ordering::Acquire)
    }
}
impl Drop for RssSampler {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}

#[test]
#[ignore = "独占同一进程实际报告+问题UI的RSS门禁，--ignored --test-threads=1"]
fn actual_report_and_ui_combined_memory_budget() {
    let (ctx, mut app) = app();
    let source = format!(
        "event start\n{}",
        (0..5_000)
            .map(|i| format!("  -> missing_{i:04}\n"))
            .collect::<String>()
    );
    app.project
        .set_text(&app.active_file.clone(), source)
        .unwrap();
    app.recompile();
    app.problems.report = None;
    app.problems.page = None;
    app.open_problems(&ctx);
    for _ in 0..3 {
        frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    }
    let baseline_rss = rss_kib();
    let baseline_hwm = hwm_kib();
    let sampler = RssSampler::start(baseline_rss);
    let started = std::time::Instant::now();
    let ticket = super::schedule::ReportTicket {
        generation: 1,
        version: app.version,
        baseline: app.project.content_baseline(),
        observation: app.project.problems_observation_key().unwrap(),
    };
    let mut job = super::job::ProblemsJob::start(&app.project, ticket, &ctx).unwrap();
    let report = loop {
        if let Some(result) = job.poll() {
            break result.unwrap();
        }
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "实际后台报告超过5秒预算"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    let build_ms = started.elapsed().as_secs_f64() * 1000.;
    let count = report.entries.len();
    assert!(count >= 5_000, "必须由真实core构建不少于5000问题");
    assert_eq!(report.compile_count, 1);
    app.problems.observation = Some(report.source_observation.clone());
    app.problems.install(report, app.version);
    for _ in 0..60 {
        frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    }
    let peak = sampler.peak().max(hwm_kib());
    let delta = peak.saturating_sub(baseline_rss);
    println!("PROBLEMS_COMBINED_RSS {{\"actual_problem_count\":{count},\"build_ms\":{build_ms},\"layout_samples\":60,\"sampling_interval_ms\":1,\"baseline_rss_kib\":{baseline_rss},\"baseline_hwm_kib\":{baseline_hwm},\"peak_rss_or_kernel_hwm_kib\":{peak},\"delta_kib\":{delta},\"compile_count\":1,\"rendered_rows\":{}}}", app.problems.rendered_rows);
    assert!(baseline_rss > 0 && peak > 0, "RSS来源不可用不能当作通过");
    assert!(delta <= 128 * 1024, "同进程实际报告+UI增量超过128MiB");
    assert!(app.problems.rendered_rows < 40);
}
