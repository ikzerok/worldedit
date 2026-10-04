use super::*;
use worldline_core::catalog_import::{parse_catalog_csv, CatalogImportRequest};

fn parse_request() -> WorkRequest {
    WorkRequest {
        schema_version: SCHEMA_VERSION,
        job_id: "catalog-csv-test".into(),
        generation: 1,
        baseline: String::new(),
        entry: PathBuf::new(),
        snapshot_state: None,
        task: WorkTask::CatalogCsvParse {
            csv: "kind,id,name\ncharacter,lin,林舟\n".into(),
        },
    }
}

#[test]
fn catalog_csv_worker_contract_rejects_shape_payload_and_task_mismatch() {
    let request = parse_request();
    request.validate().unwrap();
    let table = parse_catalog_csv("kind,id,name\ncharacter,lin,林舟\n").unwrap();
    request
        .accepts(
            &WorkOutput::CatalogCsvParse {
                table: table.clone(),
            },
            &[],
        )
        .unwrap();
    assert!(request
        .accepts_lengths(
            &WorkOutput::CatalogCsvParse {
                table: table.clone()
            },
            &[1]
        )
        .is_err());
    let mut broken = table.clone();
    broken.rows[0].cells.pop();
    assert!(request
        .accepts(&WorkOutput::CatalogCsvParse { table: broken }, &[])
        .is_err());
    let mut broken = table;
    broken.rows.resize(501, broken.rows[0].clone());
    assert!(request
        .accepts(&WorkOutput::CatalogCsvParse { table: broken }, &[])
        .is_err());
    let mut preview = parse_request();
    preview.task = WorkTask::CatalogImportPreview {
        request: CatalogImportRequest {
            schema_version: 1,
            expected_baseline: "before".into(),
            csv: "kind,id\n".into(),
            destination: PathBuf::from("world.wl"),
            columns: vec![],
        },
    };
    assert!(
        preview.validate().is_err(),
        "preview requires the full project snapshot state"
    );
}

#[test]
fn catalog_worker_json_rejects_duplicate_and_unknown_fields_before_dispatch() {
    let request = parse_request();
    let encoded = serde_json::to_string(&request).unwrap();
    parse_request_json(&encoded).unwrap().validate().unwrap();
    assert!(parse_request_json(&encoded.replacen(
        "\"catalog_csv_parse\"",
        "\"catalog_csv_parse\",\"kind\":\"catalog_csv_parse\"",
        1
    ))
    .is_err());
    let mut value = serde_json::to_value(request).unwrap();
    value["task"]["unexpected"] = true.into();
    assert!(parse_request_json(&value.to_string()).is_err());
    value["task"].as_object_mut().unwrap().remove("unexpected");
    value["unexpected"] = true.into();
    assert!(parse_request_json(&value.to_string()).is_err());
}

#[test]
fn catalog_worker_output_keeps_full_u64_fingerprints_and_rejects_duplicate_keys() {
    use worldline_core::catalog_import::CatalogImportPlan;
    let output = WorkOutput::CatalogImportPreview {
        plan: CatalogImportPlan {
            schema_version: 1,
            baseline: "baseline".into(),
            input_digest: "input".into(),
            plan_digest: "plan".into(),
            destination: "world.wl".into(),
            ignored_columns: vec![],
            normalization_count: 0,
            rows: vec![],
            diagnostics: vec![],
            error_count: 0,
            can_apply: true,
            changed_files: vec![],
            runtime_fingerprint_before: u64::MAX,
            runtime_fingerprint_after: Some(u64::MAX - 1),
        },
    };
    let json = serde_json::to_string(&output).unwrap();
    assert!(json.contains("18446744073709551615"));
    let returned = parse_output_json(&json).unwrap();
    assert!(same_json(&output, &returned));
    assert!(parse_output_json(&json.replacen(
        "\"error_count\":0",
        "\"error_count\":0,\"error_count\":0",
        1
    ))
    .is_err());
}
