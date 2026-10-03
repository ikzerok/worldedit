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
