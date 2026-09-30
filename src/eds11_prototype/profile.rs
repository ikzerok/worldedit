//! 明确启用的固定原型 CPU 侧 egui frame 技术样本；不是物理输入/GPU/发行性能。
use super::{Prototype, Task};
use std::time::Instant;

fn memory_kib() -> serde_json::Value {
    let Ok(status) = std::fs::read_to_string("/proc/self/status") else {
        return serde_json::Value::Null;
    };
    let field = |name: &str| {
        status
            .lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u64>().ok())
    };
    serde_json::json!({"rss_kib":field("VmRSS:"),"peak_rss_kib":field("VmHWM:")})
}

#[test]
#[ignore = "manual fixed-task egui CPU-side draw and process-memory technical profile"]
fn fixed_tasks_draw_cpu_side_profile() {
    let mut results = Vec::new();
    for task in [Task::World, Task::Writing, Task::Run, Task::Review] {
        let ctx = egui::Context::default();
        crate::fonts::install_cjk_fonts(&ctx);
        let mut app = Prototype {
            task,
            ..Prototype::default()
        };
        if task == Task::Run {
            app.demos[0].run_event().unwrap();
            app.runtime_event = true;
        }
        if task == Task::Review {
            app.demos[0].compare().unwrap();
        }
        let baseline = app.demos[0].project.content_baseline();
        let mut samples = Vec::new();
        for index in 0..41 {
            if task == Task::Writing {
                ctx.memory_mut(|memory| {
                    memory.request_focus(egui::Id::new(("eds11_draft", 0usize)))
                });
            }
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1024.0, 640.0),
                )),
                time: Some(index as f64 / 60.0),
                events: if task == Task::Writing {
                    vec![egui::Event::Text("x".into())]
                } else {
                    vec![]
                },
                ..Default::default()
            };
            let start = Instant::now();
            let _ = ctx.run(input, |ctx| app.draw(ctx));
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        assert_eq!(app.demos[0].project.content_baseline(), baseline);
        assert!(!app.demos[0].project.root.exists());
        let mut warm = samples[1..].to_vec();
        warm.sort_by(f64::total_cmp);
        results.push(serde_json::json!({"task":task.label(),"first_context_frame_ms":samples[0],
            "warm_p50_ms":warm[19],"warm_p95_ms":warm[37],"warm_max_ms":warm[39],
            "raw_frame_ms":samples,"memory_after":memory_kib(),"logical_size":[1024,640],
            "pixels_per_point":ctx.pixels_per_point(),"writing_input":"synthetic egui text events only"}));
    }
    let record = serde_json::json!({"measurement":"headless CPU-side Context::run elapsed wall duration; draw and egui layout only",
        "build":if cfg!(debug_assertions) {"Cargo dev/test"} else {"Cargo release"},
        "not_measured":["OS input-to-photon latency","GPU/present","native compositor", "tessellation","browser zoom","release product SLA"],
        "first_frame_scope":"fresh egui context, not OS-cache-cold launch",
        "percentile_method":"nearest rank over 40 warm frames",
        "preconditioning":"J1 uses initialized sample; J3 Story and J4 proposal comparison execute before timing; their core action durations are not measured","shared_cloud_load":"not controlled as dedicated benchmark hardware", "tasks":results});
    let text = serde_json::to_string_pretty(&record).unwrap();
    if let Some(path) = std::env::var_os("EDS11_PROFILE_OUTPUT") {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        file.write_all(text.as_bytes()).unwrap();
    }
    println!("EDS11_PROFILE_JSON={text}");
}
