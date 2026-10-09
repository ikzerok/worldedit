use super::*;
use worldline_core::project::Project;
use worldline_core::queries::CatalogQuery;

#[test]
fn catalog_scope_worker_contract_rejects_wrong_baseline_query_budget_and_payload() {
    let project = Project::new(&std::env::temp_dir().join("catalog-scope-worker-contract"));
    let query = CatalogQuery::default();
    let scope = project.catalog_scope_snapshot(&query, 10_000).unwrap();
    let mut request = WorkRequest {
        schema_version: SCHEMA_VERSION,
        job_id: "scope-1".into(),
        generation: 1,
        baseline: project.content_baseline(),
        entry: "world.wl".into(),
        snapshot_state: Some(project.snapshot_state().unwrap()),
        task: WorkTask::CatalogScope {
            query: query.clone(),
            max_candidates: 10_000,
        },
    };
    let output = WorkOutput::CatalogScope { scope };
    assert!(request.validate().is_ok());
    assert!(request.accepts(&output, &[]).is_ok());
    assert!(request.accepts(&output, &[vec![1]]).is_err());
    request.baseline = "changed".into();
    assert!(request.accepts(&output, &[]).is_err());
    request.baseline = project.content_baseline();
    request.task = WorkTask::CatalogScope {
        query,
        max_candidates: 1,
    };
    assert!(request.accepts(&output, &[]).is_err());
    request.snapshot_state = None;
    assert!(request.validate().is_err());
}
#[test]
fn catalog_scope_worker_json_is_typed_and_rejects_unknown_fields() {
    assert!(parse_request_json(r#"{"schema_version":1,"job_id":"scope","generation":1,"baseline":"x","entry":"world.wl","snapshot_state":null,"task":{"kind":"catalog_scope","query":{"schema_version":1},"max_candidates":10000,"unexpected":true}}"#).is_err());
}
