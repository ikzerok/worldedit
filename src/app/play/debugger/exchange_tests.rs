//! 编辑器交换动作的边界与失败原子性；故事输入只在测试内构造。
use crate::app::ReplayDebugger;
use worldline_runtime::{
    decode_replay_trace, encode_replay_trace, ReplayStep, ReplayTrace, Story,
    MAX_REPLAY_EXCHANGE_BYTES, MAX_REPLAY_EXCHANGE_STEPS,
};

fn fixture_trace() -> ReplayTrace {
    let compiled = worldline_core::compile_source(
        "exchange.wl",
        "event start\n  中文正文\n  choice \"继续\"\n    -> END\n",
    );
    let mut story = Story::new_with_seed(&compiled.program, &compiled.analysis, 29).unwrap();
    story.continue_story().unwrap();
    story.choose(0).unwrap();
    story.continue_story().unwrap();
    story.replay_trace()
}

fn paths(debugger: &ReplayDebugger) -> Vec<(String, ReplayTrace)> {
    debugger
        .saved_paths
        .iter()
        .map(|path| (path.name.clone(), path.trace.clone()))
        .collect()
}

#[test]
fn exchange_record_export_import_is_canonical_and_selection_clears_delivery() {
    let trace = fixture_trace();
    let mut debugger = ReplayDebugger::default();
    debugger.record_replay_path(trace.clone());
    debugger.export_replay_path();
    let json = debugger.export_json.clone();
    assert_eq!(json, encode_replay_trace(&trace).unwrap());
    assert_eq!(decode_replay_trace(json.as_bytes()).unwrap(), trace);
    debugger.import_json = json.clone();
    debugger.import_replay_path();
    assert_eq!(debugger.selected_path, Some(1));
    assert!(debugger.export_json.is_empty());
    assert_eq!(debugger.saved_paths[1].trace, trace);
    debugger.export_replay_path();
    assert_eq!(debugger.export_json, json);
    debugger.select_replay_path(Some(0));
    assert!(debugger.export_json.is_empty());
    debugger.export_replay_path();
    debugger.select_replay_path(Some(0));
    assert_eq!(debugger.export_json, json);
    debugger.select_replay_path(None);
    assert!(debugger.export_json.is_empty());
}

#[test]
fn exchange_accepts_runtime_recording_between_one_and_four_mib() {
    let source = format!(
        "let payload = \"{}\"\nevent start\n  choice \"继续\"\n    -> start\n",
        "中".repeat(600)
    );
    let compiled = worldline_core::compile_source("large.wl", &source);
    let mut story = Story::new_with_seed(&compiled.program, &compiled.analysis, 29).unwrap();
    story.continue_story().unwrap();
    for _ in 0..600 {
        story.choose(0).unwrap();
        story.continue_story().unwrap();
    }
    let trace = story.replay_trace();
    let mut debugger = ReplayDebugger::default();
    debugger.record_replay_path(trace.clone());
    assert_eq!(debugger.saved_paths.len(), 1, "{:?}", debugger.notice);
    debugger.export_replay_path();
    assert!(debugger.export_json.len() > 1024 * 1024);
    assert!(debugger.export_json.len() < MAX_REPLAY_EXCHANGE_BYTES);
    debugger.import_json = debugger.export_json.clone();
    debugger.import_replay_path();
    assert_eq!(debugger.saved_paths[1].trace, trace);
    debugger.export_replay_path();
    assert_eq!(debugger.export_json, debugger.import_json);
}

#[test]
fn exchange_bytes_include_utf8_and_escaping_and_export_failure_removes_stale_json() {
    let mut boundary = fixture_trace();
    boundary.initial_observation.as_mut().unwrap().state = serde_json::json!({"pad": ""});
    let overhead = encode_replay_trace(&boundary).unwrap().len();
    boundary.initial_observation.as_mut().unwrap().state["pad"] =
        serde_json::Value::String("x".repeat(MAX_REPLAY_EXCHANGE_BYTES - overhead));
    let mut debugger = ReplayDebugger::default();
    debugger.record_replay_path(boundary.clone());
    debugger.export_replay_path();
    assert_eq!(debugger.export_json.len(), MAX_REPLAY_EXCHANGE_BYTES);
    debugger.import_json = debugger.export_json.clone();
    debugger.import_replay_path();
    assert_eq!(debugger.saved_paths[1].trace, boundary);
    debugger.export_replay_path();
    let prior_paths = paths(&debugger);
    let prior_output = debugger.export_json.clone();
    debugger.import_json.push('中');
    assert!(debugger.import_json.chars().count() < MAX_REPLAY_EXCHANGE_BYTES);
    debugger.import_replay_path();
    assert_eq!(paths(&debugger), prior_paths);
    assert_eq!(debugger.export_json, prior_output);
    assert!(debugger.notice.as_deref().unwrap().contains("input_limit"));

    debugger.saved_paths[1]
        .trace
        .initial_observation
        .as_mut()
        .unwrap()
        .state["pad"] = serde_json::Value::String("\"\n中".repeat(MAX_REPLAY_EXCHANGE_BYTES / 6));
    let prior_paths = paths(&debugger);
    debugger.export_replay_path();
    assert!(debugger.export_json.is_empty());
    assert_eq!(paths(&debugger), prior_paths);
    assert_eq!(debugger.selected_path, Some(1));
    assert!(debugger.notice.as_deref().unwrap().contains("output_limit"));
}

#[test]
fn exchange_invalid_imports_reject_duplicates_depth_schema_and_trailing_data_atomically() {
    let mut debugger = ReplayDebugger::default();
    debugger.record_replay_path(fixture_trace());
    debugger.export_replay_path();
    let valid = debugger.export_json.clone();
    let prior = paths(&debugger);
    let mut nested = fixture_trace();
    nested.initial_observation.as_mut().unwrap().state = serde_json::json!({"probe": 0});
    let nested = encode_replay_trace(&nested)
        .unwrap()
        .replace("\"probe\":0", "\"probe\":0,\"probe\":1");
    let invalid = [
        format!("{{\"schema_version\":1,{}", &valid[1..]),
        nested,
        valid.replace("\"schema_version\":1", "\"schema_version\":99"),
        format!("{valid} {{}}"),
        format!("{}0{}", "[".repeat(140), "]".repeat(140)),
        "{\"overflow\":1e999}".into(),
    ];
    for json in invalid {
        debugger.import_json = json;
        debugger.import_replay_path();
        assert_eq!(paths(&debugger), prior);
        assert_eq!(debugger.selected_path, Some(0));
        assert_eq!(debugger.export_json, valid);
        assert!(debugger.notice.as_deref().unwrap().contains("路径导入失败"));
    }
}

#[test]
fn exchange_step_limit_and_session_capacity_are_shared_by_record_and_import() {
    let mut trace = fixture_trace();
    let step = ReplayStep {
        observation: None,
        ..trace.steps[0].clone()
    };
    trace.initial_observation = None;
    trace.steps = vec![step.clone(); MAX_REPLAY_EXCHANGE_STEPS];
    let json = encode_replay_trace(&trace).unwrap();
    let mut debugger = ReplayDebugger {
        import_json: json,
        ..Default::default()
    };
    debugger.import_replay_path();
    assert_eq!(
        debugger.saved_paths[0].trace.steps.len(),
        MAX_REPLAY_EXCHANGE_STEPS
    );
    trace.steps.push(step);
    debugger.record_replay_path(trace.clone());
    assert_eq!(debugger.saved_paths.len(), 1);
    assert!(debugger.notice.as_deref().unwrap().contains("step_limit"));
    debugger.import_json = serde_json::to_string(&trace).unwrap();
    debugger.import_replay_path();
    assert_eq!(debugger.saved_paths.len(), 1);
    assert!(debugger.notice.as_deref().unwrap().contains("step_limit"));

    let small = fixture_trace();
    debugger.saved_paths.clear();
    debugger.selected_path = None;
    for _ in 0..64 {
        debugger.record_replay_path(small.clone());
    }
    debugger.export_replay_path();
    let previous = (
        paths(&debugger),
        debugger.selected_path,
        debugger.export_json.clone(),
        debugger.path_name.clone(),
    );
    debugger.record_replay_path(small.clone());
    debugger.import_json = encode_replay_trace(&small).unwrap();
    debugger.import_replay_path();
    assert_eq!(
        (
            paths(&debugger),
            debugger.selected_path,
            debugger.export_json.clone(),
            debugger.path_name.clone()
        ),
        previous
    );
    assert!(debugger.notice.as_deref().unwrap().contains("64"));
}
