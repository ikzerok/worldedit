//! schema1加法兼容和完整WorkOutput预算；不放松请求字段或定位核验。
use super::*;
use crate::json_budget::check_problem_report_output;
use worldline_core::{
    problems::{ProblemLocation, ProblemsOptions, ProblemsReport},
    project::Project,
};

fn pair() -> (WorkRequest, ProblemsReport) {
    let mut project = Project::new(&std::env::temp_dir().join("worker-source-context-test"));
    let path = project.entry.clone();
    project
        .set_text(&path, "event start\n  -> missing\n".into())
        .unwrap();
    let report = project
        .problems_report(&ProblemsOptions::default())
        .unwrap();
    let request = WorkRequest {
        schema_version: 1,
        job_id: "context-schema1".into(),
        generation: 1,
        baseline: report.content_baseline.clone(),
        entry: PathBuf::from("main.wl"),
        snapshot_state: Some(project.snapshot_state().unwrap()),
        task: WorkTask::ProblemsReport {
            options: report.limits.clone(),
            source_observation: report.source_observation.clone(),
        },
    };
    (request, report)
}

// 冻结0.18的来源形状；旧消费者忽略新增响应字段的承诺只覆盖这条已测路径。
#[derive(Serialize, Deserialize)]
struct LegacyLocation {
    path: Option<String>,
    precision: worldline_core::problems::ProblemPrecision,
    span: Option<worldline_core::Span>,
    byte_range: Option<worldline_core::problems::ProblemRange>,
    char_range: Option<worldline_core::problems::ProblemRange>,
    excerpt: Option<String>,
    excerpt_truncated: bool,
    reason: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct LegacyEntry {
    id: String,
    domain: worldline_core::problems::ProblemDomain,
    severity: worldline_core::Severity,
    code: String,
    message: String,
    note: Option<String>,
    suggestion: Option<String>,
    primary: LegacyLocation,
    related_count: usize,
    text_truncated: bool,
}
#[derive(Serialize, Deserialize)]
struct LegacyReport {
    schema_version: u32,
    report_version: String,
    content_baseline: String,
    source_observation: String,
    language_version: String,
    content_has_errors: bool,
    read_only: bool,
    complete: bool,
    truncated: bool,
    reasons: Vec<String>,
    coverage: Vec<worldline_core::problems::ProblemCoverage>,
    entries: Vec<LegacyEntry>,
    related: std::collections::BTreeMap<String, Vec<LegacyLocation>>,
    limits: ProblemsOptions,
    compile_count: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum LegacyOutput {
    ProblemsReport { report: LegacyReport },
}

#[test]
fn schema1_context_roundtrip_and_frozen_legacy_location_remain_readable() {
    let (request, report) = pair();
    assert!(request.validate().is_ok());
    assert!(report
        .entries
        .iter()
        .any(|entry| entry.primary.context.is_some()));
    let output = WorkOutput::ProblemsReport {
        report: report.clone(),
    };
    let json = serde_json::to_value(&output).unwrap();
    let old_consumer: LegacyOutput = serde_json::from_value(json.clone()).unwrap();
    let restored: WorkOutput =
        serde_json::from_value(serde_json::to_value(old_consumer).unwrap()).unwrap();
    request.accepts(&restored, &[]).unwrap();
    let current: WorkOutput = serde_json::from_value(json.clone()).unwrap();
    request.accepts(&current, &[]).unwrap();
    for entry in &report.entries {
        let new = serde_json::to_value(&entry.primary).unwrap();
        let old: LegacyLocation = serde_json::from_value(new).unwrap();
        assert_eq!(old.excerpt, entry.primary.excerpt);
        let restored: ProblemLocation =
            serde_json::from_value(serde_json::to_value(old).unwrap()).unwrap();
        assert!(restored.context.is_none());
        assert_eq!(restored.span, entry.primary.span);
    }
    let mut legacy = json;
    for entry in legacy["report"]["entries"].as_array_mut().unwrap() {
        entry["primary"].as_object_mut().unwrap().remove("context");
    }
    for related in legacy["report"]["related"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        for location in related.as_array_mut().unwrap() {
            location.as_object_mut().unwrap().remove("context");
        }
    }
    let old: WorkOutput = serde_json::from_value(legacy).unwrap();
    request.accepts(&old, &[]).unwrap(); // 传输读取不等于授权定位；core继续严格要求刷新。
    let WorkOutput::ProblemsReport { report: old } = old else {
        unreachable!()
    };
    assert!(old
        .entries
        .iter()
        .all(|entry| entry.primary.context.is_none()));
    let mut strict = serde_json::to_value(&request).unwrap();
    strict
        .as_object_mut()
        .unwrap()
        .insert("invented_context_request".into(), true.into());
    assert!(serde_json::from_value::<WorkRequest>(strict).is_err());
    let mut unknown = request;
    unknown.schema_version = 2;
    assert!(unknown.validate().is_err());
}

#[test]
fn complete_work_output_budget_includes_context_and_json_wrapper() {
    let (request, mut report) = pair();
    report.reasons = vec![String::new()];
    let bare = serde_json::to_vec(&report).unwrap().len();
    report.reasons[0] = "a".repeat(MAX_JSON_BYTES - bare);
    assert!(serialized_within(&report, MAX_JSON_BYTES));
    assert_eq!(serde_json::to_vec(&report).unwrap().len(), MAX_JSON_BYTES);
    assert!(
        check_problem_report_output(&report).is_err(),
        "裸报告达标不等于外壳达标"
    );
    let output = WorkOutput::ProblemsReport {
        report: report.clone(),
    };
    let full = serde_json::to_vec(&output).unwrap().len();
    assert!(full > MAX_JSON_BYTES);
    assert!(request.accepts(&output, &[]).is_err());
    let wrapper = full - MAX_JSON_BYTES;
    let keep = report.reasons[0].len() - wrapper;
    report.reasons[0].truncate(keep);
    check_problem_report_output(&report).unwrap();
    let exact = WorkOutput::ProblemsReport { report };
    assert_eq!(serde_json::to_vec(&exact).unwrap().len(), MAX_JSON_BYTES);
    request.accepts(&exact, &[]).unwrap();
}

#[test]
fn bound_counter_counts_json_escaping_and_checks_report_requested_limit() {
    assert!(!serialized_within(&"\n", 3));
    assert!(serialized_within(&"\n", 4));
    let (mut request, mut report) = pair();
    let actual = serde_json::to_vec(&report).unwrap().len();
    report.limits.max_report_bytes = actual / 2;
    let WorkTask::ProblemsReport { options, .. } = &mut request.task else {
        unreachable!()
    };
    *options = report.limits.clone();
    assert!(request
        .accepts(&WorkOutput::ProblemsReport { report }, &[])
        .is_err());
}
