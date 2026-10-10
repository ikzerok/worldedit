use super::super::delivery::Action;
use super::*;

#[test]
fn production_flow_three_formats_require_real_confirmation_and_copy_preview_exactly() {
    let _serial = serial();
    let mut flow = Flow::new(3);
    flow.stage(
        "Line 00",
        r#"=1+1\n<script>[click](https://example.test)</script>,\"quoted\""#,
    );
    let disk = flow.disk();
    let baseline = flow.app.project.content_baseline();
    flow.generate();
    let snapshot = flow.snapshot();
    let serialized_page = serde_json::to_string(&snapshot.page(0, 100).unwrap()).unwrap();
    assert!(!serialized_page.contains(PRIVATE));
    for (format, label, extension) in [
        (ProductionFormat::Json, "精确 JSON", "json"),
        (ProductionFormat::Markdown, "阅读 Markdown", "md"),
        (ProductionFormat::Csv, "表格 CSV", "csv"),
    ] {
        if !flow.app.manuscript.production.export_open {
            flow.click("准备私密交付");
        }
        flow.click(label);
        flow.preview();
        let bytes = flow.artifact();
        let expected = snapshot
            .export(&ProductionExportOptions {
                schema_version: 1,
                format,
                include_direction: false,
            })
            .unwrap();
        assert_eq!(bytes, expected.bytes());
        let rendered = String::from_utf8(bytes.clone()).unwrap();
        for forbidden in [
            PRIVATE,
            "OTHER_ROLE_SECRET",
            "UNSELECTED_SECRET",
            "say a",
            "excerpt",
            flow.app.project.root.to_str().unwrap(),
        ] {
            assert!(
                !rendered
                    .replace('\\', "")
                    .contains(&forbidden.replace('\\', "")),
                "{format:?} leaked {forbidden}"
            );
        }
        match format {
            ProductionFormat::Json => {
                let document: Value = serde_json::from_slice(&bytes).unwrap();
                let rows = document["rows"].as_array().unwrap();
                assert!(rows.iter().all(|row| row.get("direction").is_none()));
                assert!(rows.iter().any(|row| row["source_parts"][0]["text"]
                    .as_str()
                    .is_some_and(|text| text.starts_with("=1+1\n<script>"))));
            }
            ProductionFormat::Markdown => {
                assert!(rendered.contains("&lt;script&gt;"));
                assert!(!rendered.contains("[click](https://example.test)"));
            }
            ProductionFormat::Csv => {
                let cells = csv_cells(&bytes);
                assert!(cells.iter().all(|cell| cell.starts_with('\'')));
                assert!(cells.iter().any(|cell| cell.starts_with("'=1+1\n<script>")));
                assert!(rendered.contains("\"\"quoted\"\""));
                assert!(rendered.ends_with("\r\n"));
            }
        }
        assert!(flow.app.checked_production_artifact(&flow.ctx).is_err());
        assert!(copied(&flow.click("复制相同完整材料")).is_none());
        let destination = flow.directory.join(format!("script.{extension}"));
        flow.app.manuscript.production.destination = destination.to_string_lossy().into_owned();
        flow.app.finish_production_export(&flow.ctx, Action::Save);
        assert!(!destination.exists());
        flow.confirm();
        let output = flow.click("复制相同完整材料");
        assert_eq!(copied(&output).unwrap().as_bytes(), bytes);
        flow.click("写入新文件");
        assert_eq!(fs::read(&destination).unwrap(), bytes);
        flow.click("写入新文件");
        assert_eq!(fs::read(&destination).unwrap(), bytes);
        assert!(!flow.notice().starts_with("已写入"));

        flow.click("明确纳入作者私密演出备注（源语言）");
        assert!(!flow.app.manuscript.production.confirmed);
        assert!(flow.app.checked_production_artifact(&flow.ctx).is_err());
        assert!(copied(&flow.click("复制相同完整材料")).is_none());
        flow.preview();
        let explicit = String::from_utf8(flow.artifact()).unwrap();
        assert!(explicit.replace('\\', "").contains(PRIVATE));
        assert!(!explicit.contains("OTHER_ROLE_SECRET"));
        assert_eq!(flow.snapshot().key(), snapshot.key());
        flow.confirm();
        let bytes = flow.artifact();
        assert_eq!(
            copied(&flow.click("复制相同完整材料")).unwrap().as_bytes(),
            bytes
        );
        flow.click("明确纳入作者私密演出备注（源语言）");
    }
    assert_eq!(flow.app.project.content_baseline(), baseline);
    assert!(!flow.app.project.is_dirty());
    assert_eq!(flow.disk(), disk);
    assert!(!fs::read_dir(&flow.directory).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".worldline-")));
}

#[test]
fn production_flow_delivery_rejects_internal_existing_wrong_extension_and_stale_paths() {
    let _serial = serial();
    let mut flow = Flow::new(3);
    flow.generate();
    flow.preview();
    flow.confirm();
    let bytes = flow.artifact();
    let existing = flow.directory.join("existing.json");
    fs::write(&existing, b"preserve me").unwrap();
    let internal = flow.app.project.root.join("private.json");
    let wrong = flow.directory.join("wrong.csv");
    let missing_parent = flow.directory.join("missing/production.json");
    for destination in [&existing, &internal, &wrong, &missing_parent] {
        flow.app.manuscript.production.destination = destination.to_string_lossy().into_owned();
        flow.app.finish_production_export(&flow.ctx, Action::Save);
        assert!(!flow.notice().starts_with("已写入"), "{}", flow.notice());
    }
    assert_eq!(fs::read(&existing).unwrap(), b"preserve me");
    for absent in [&internal, &wrong, &missing_parent] {
        assert!(!absent.exists());
    }
    flow.app.manuscript.production.destination = "relative.json".into();
    flow.app.finish_production_export(&flow.ctx, Action::Save);
    assert!(!flow.notice().starts_with("已写入"));
    let stale = flow.directory.join("stale.json");
    flow.app.manuscript.production.destination = stale.to_string_lossy().into_owned();
    flow.app.manuscript.production.speaker = Some(TargetRef::new("character", "b"));
    flow.app.finish_production_export(&flow.ctx, Action::Save);
    assert!(!stale.exists());
    assert_eq!(flow.artifact(), bytes);
    flow.generate();
    assert!(flow.app.manuscript.production.artifact.is_none());
    assert!(!flow.app.manuscript.production.confirmed);
    flow.preview();
    flow.confirm();
    flow.click("写入新文件");
    assert_eq!(fs::read(&stale).unwrap(), flow.artifact());
}

fn csv_cells(bytes: &[u8]) -> Vec<String> {
    let mut result = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        assert_eq!(bytes[index], b'"');
        index += 1;
        let mut cell = Vec::new();
        loop {
            assert!(index < bytes.len(), "unterminated CSV cell");
            let byte = bytes[index];
            index += 1;
            if byte == b'"' {
                if bytes.get(index) == Some(&b'"') {
                    cell.push(b'"');
                    index += 1;
                } else {
                    break;
                }
            } else {
                cell.push(byte);
            }
        }
        result.push(String::from_utf8(cell).unwrap());
        match bytes.get(index) {
            Some(b',') => index += 1,
            Some(b'\r') => {
                assert_eq!(bytes.get(index + 1), Some(&b'\n'));
                index += 2;
            }
            _ => panic!("CSV records require CRLF and cells require comma separation"),
        }
    }
    result
}
