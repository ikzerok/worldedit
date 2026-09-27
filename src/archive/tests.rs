use super::*;
use std::io::{Cursor, Write};
use std::path::Path;

#[test]
fn archive_roundtrip_preserves_nested_sources_assets_and_entry() {
    let files = Files::from([
        (
            "world.wl".into(),
            "include \"events/雾港.wl\"\n".as_bytes().to_vec(),
        ),
        (
            "events/雾港.wl".into(),
            "event start\n  你好。\n  -> END\n".as_bytes().to_vec(),
        ),
        ("assets/image.png".into(), vec![0, 128, 255, 1]),
    ]);
    let restored = decode(&encode(&files).unwrap()).unwrap();
    assert_eq!(restored, files);
    assert_eq!(entry(&restored).unwrap(), Path::new("world.wl"));
    for path in [
        "../bad.wl",
        "/bad.wl",
        "C:/bad.wl",
        "events/../../bad.wl",
        "..\\bad.wl",
    ] {
        assert!(relative_path(path).is_err(), "{path}");
    }
}

#[test]
fn browser_snapshot_roundtrip_keeps_checkpoint_session_and_reads_legacy_base64() {
    let stored =
        encode_browser_snapshot("UEsDBA==", "browser-session-a", Some("d2xjcA==")).unwrap();
    assert_eq!(
        decode_browser_snapshot(&stored).unwrap(),
        DecodedBrowserSnapshot {
            archive_base64: "UEsDBA==".into(),
            checkpoint_session_id: Some("browser-session-a".into()),
            checkpoint_snapshot_base64: Some("d2xjcA==".into()),
        }
    );
    let legacy_envelope = decode_browser_snapshot(
        r#"{"version":1,"checkpoint_session_id":"browser-session-a","archive_base64":"UEsDBA=="}"#,
    )
    .unwrap();
    assert_eq!(
        legacy_envelope.checkpoint_session_id.as_deref(),
        Some("browser-session-a")
    );
    assert_eq!(legacy_envelope.checkpoint_snapshot_base64, None);
    assert_eq!(
        decode_browser_snapshot("UEsDBA==").unwrap(),
        DecodedBrowserSnapshot {
            archive_base64: "UEsDBA==".into(),
            checkpoint_session_id: None,
            checkpoint_snapshot_base64: None,
        }
    );
    assert!(decode_browser_snapshot(
        r#"{"version":3,"checkpoint_session_id":"browser-session-a","archive_base64":"UEsDBA=="}"#
    )
    .is_err());
    assert!(decode_browser_snapshot(
        r#"{"version":1,"checkpoint_session_id":"../other","archive_base64":"UEsDBA=="}"#
    )
    .is_err());
    assert!(encode_browser_snapshot(
        "UEsDBA==",
        "browser-session-a",
        Some(&"x".repeat(MAX_BROWSER_CHECKPOINT_SNAPSHOT_BASE64 + 1)),
    )
    .is_err());
}

#[test]
fn browser_recovery_bundle_roundtrips_workspace_and_checkpoint_payload() {
    let project_files = Files::from([("world.wl".into(), b"event start\n".to_vec())]);
    let project_archive = encode(&prepare_import(project_files).unwrap()).unwrap();
    let checkpoints = b"bounded checkpoint bytes";
    let recovery =
        encode_browser_recovery_bundle(&project_archive, checkpoints, "browser-session-a").unwrap();
    let decoded = decode_browser_recovery_bundle(&recovery).unwrap().unwrap();
    assert_eq!(decoded.project_files, decode(&project_archive).unwrap());
    assert_eq!(decoded.checkpoint_session_id, "browser-session-a");
    assert_eq!(decoded.checkpoint_snapshot, checkpoints);
    assert!(decode_browser_recovery_bundle(&project_archive)
        .unwrap()
        .is_none());
}

#[test]
fn archive_roundtrip_preserves_authoring_manifest_and_opaque_json_bytes() {
    let files = Files::from([
        ("world.wl".into(), b"event start\n  -> END\n".to_vec()),
        (
            MANIFEST.into(),
            br#"{"entry":"world.wl","future":true}"#.to_vec(),
        ),
        (
            ".world/project.json".into(),
            br#"{"schema_version":1,"unknown":{"kept":true},"entry":"world.wl"}"#.to_vec(),
        ),
        (".world/maps/raw.json".into(), vec![b'{', 0xff, b'}']),
        ("assets/map.png".into(), vec![0, 1, 2, 255]),
    ]);
    let restored = decode(&encode(&files).unwrap()).unwrap();
    assert_eq!(restored, files);
    assert_eq!(entry(&restored).unwrap(), Path::new("world.wl"));
}

#[test]
fn entry_rejects_conflicting_legacy_and_project_manifests() {
    let files = Files::from([
        ("world.wl".into(), b"event start\n  -> END\n".to_vec()),
        (MANIFEST.into(), br#"{"entry":"world.wl"}"#.to_vec()),
        (
            ".world/project.json".into(),
            br#"{"schema_version":1,"entry":"other.wl"}"#.to_vec(),
        ),
        ("other.wl".into(), b"event other\n  -> END\n".to_vec()),
    ]);
    assert!(entry(&files).is_err());
}

#[test]
fn entry_uses_project_manifest_when_legacy_record_is_absent() {
    let files = Files::from([
        (
            "stories/intro.wl".into(),
            b"event start\n  -> END\n".to_vec(),
        ),
        (
            ".world/project.json".into(),
            br#"{"schema_version":1,"entry":"stories/intro.wl"}"#.to_vec(),
        ),
    ]);
    assert_eq!(entry(&files).unwrap(), Path::new("stories/intro.wl"));
}

#[test]
fn entry_does_not_use_last_duplicate_project_manifest_key() {
    let files = Files::from([
        ("world.wl".into(), b"event start\n  -> END\n".to_vec()),
        ("other.wl".into(), b"event other\n  -> END\n".to_vec()),
        (
            ".world/project.json".into(),
            br#"{"entry":"other.wl","entry":"world.wl"}"#.to_vec(),
        ),
    ]);
    assert_eq!(entry(&files).unwrap(), Path::new("world.wl"));
}

#[test]
fn legacy_manifest_is_preserved_and_missing_entry_is_added() {
    let legacy = br#"{"entry":"world.wl","future":{"kept":true}}"#.to_vec();
    let mut files = Files::from([
        ("world.wl".into(), b"event start".to_vec()),
        (MANIFEST.into(), legacy.clone()),
        (
            ".world/project.json".into(),
            br#"{"schema_version":1,"entry":"world.wl"}"#.to_vec(),
        ),
    ]);
    ensure_legacy_manifest(&mut files, "world.wl").unwrap();
    assert_eq!(files[Path::new(MANIFEST)], legacy);
    assert_eq!(entry(&files).unwrap(), Path::new("world.wl"));

    let conflict_legacy = br#"{"entry":"other.wl","future":{"kept":true}}"#.to_vec();
    let mut conflict = Files::from([
        ("world.wl".into(), b"event start".to_vec()),
        ("other.wl".into(), b"event other".to_vec()),
        (MANIFEST.into(), conflict_legacy.clone()),
        (
            ".world/project.json".into(),
            br#"{"schema_version":1,"entry":"world.wl"}"#.to_vec(),
        ),
    ]);
    assert!(ensure_legacy_manifest(&mut conflict, "world.wl").is_err());
    assert_eq!(conflict[Path::new(MANIFEST)], conflict_legacy);
    assert!(entry(&conflict).is_err());

    let mut legacy_only = Files::from([
        ("world.wl".into(), b"event start".to_vec()),
        ("other.wl".into(), b"event other".to_vec()),
        (
            MANIFEST.into(),
            br#"{"entry":"other.wl","future":true}"#.to_vec(),
        ),
    ]);
    assert!(ensure_legacy_manifest(&mut legacy_only, "world.wl").is_err());
    assert_eq!(
        legacy_only[Path::new(MANIFEST)],
        br#"{"entry":"other.wl","future":true}"#
    );

    let mut missing = Files::from([("world.wl".into(), b"event start".to_vec())]);
    ensure_legacy_manifest(&mut missing, "world.wl").unwrap();
    assert_eq!(entry(&missing).unwrap(), Path::new("world.wl"));
}

#[test]
fn archive_rejects_transaction_directory() {
    let files = Files::from([(
        ".world/.transactions/tx/journal.json".into(),
        b"{}".to_vec(),
    )]);
    assert!(encode(&files).is_err());
    let mixed_case = Files::from([(
        ".WORLD/.TRANSACTIONS/tx/journal.json".into(),
        b"{}".to_vec(),
    )]);
    assert!(encode(&mixed_case).is_err());
    let windows_style = Files::from([(
        r#".world\.transactions\tx\journal.json"#.into(),
        b"{}".to_vec(),
    )]);
    assert!(encode(&windows_style).is_err());

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        ".world/.transactions/tx/journal.json",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(b"{}").unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    assert!(decode(&bytes).is_err());

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file(
        ".WORLD/.TRANSACTIONS/tx/journal.json",
        zip::write::SimpleFileOptions::default(),
    )
    .unwrap();
    zip.write_all(b"{}").unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    assert!(decode(&bytes).is_err());
}

#[test]
fn archive_rejects_windows_case_collisions_on_encode_and_decode() {
    let files = Files::from([
        ("Maps/Overview.wl".into(), b"one".to_vec()),
        ("maps/overview.wl".into(), b"two".to_vec()),
    ]);
    assert!(encode(&files).is_err());

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, contents) in [("Maps/Overview.wl", b"one"), (r"maps\overview.wl", b"two")] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(contents).unwrap();
    }
    let bytes = zip.finish().unwrap().into_inner();
    assert!(decode(&bytes).is_err());
}

#[test]
fn archive_rejects_directory_and_file_collisions_after_normalization() {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.add_directory("Maps/", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.start_file("maps", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"file").unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    assert!(decode(&bytes).is_err());

    let files = Files::from([
        ("Assets/a.txt".into(), b"one".to_vec()),
        ("assets/b.txt".into(), b"two".to_vec()),
    ]);
    assert!(encode(&files).is_err());
    let ordinary = Files::from([
        ("assets/a.txt".into(), b"one".to_vec()),
        ("assets/b.txt".into(), b"two".to_vec()),
    ]);
    assert!(encode(&ordinary).is_ok());
    let different_parent = Files::from([
        ("assets/A.txt".into(), b"one".to_vec()),
        ("notes/a.txt".into(), b"two".to_vec()),
    ]);
    assert!(encode(&different_parent).is_ok());

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.add_directory("Maps/", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.add_directory("maps\\", zip::write::SimpleFileOptions::default())
        .unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    assert!(decode(&bytes).is_err());

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, contents) in [("Assets/a.txt", b"one"), ("assets/b.txt", b"two")] {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(contents).unwrap();
    }
    let bytes = zip.finish().unwrap().into_inner();
    assert!(decode(&bytes).is_err());
}

#[test]
fn validate_files_rejects_paths_that_directory_import_must_reject() {
    let files = Files::from([
        ("Assets/a.txt".into(), b"one".to_vec()),
        ("assets/b.txt".into(), b"two".to_vec()),
    ]);
    assert!(validate_files(&files).is_err());

    let transactions = Files::from([(
        ".WORLD/.TRANSACTIONS/tx/journal.json".into(),
        b"{}".to_vec(),
    )]);
    assert!(validate_files(&transactions).is_err());
}

#[test]
fn raw_import_boundary_can_fail_after_legacy_manifest_is_added() {
    let mut files = Files::new();
    files.insert("world.wl".into(), b"event start".to_vec());
    for index in 0..(4096 - 1) {
        files.insert(format!("assets/{index}.bin").into(), vec![index as u8]);
    }
    assert_eq!(files.len(), 4096);
    assert!(validate_files(&files).is_ok());
    assert!(prepare_import(files).is_err());
}

#[test]
fn raw_size_boundary_can_fail_after_legacy_manifest_is_added() {
    let mut files = Files::new();
    files.insert("world.wl".into(), b"w".to_vec());
    files.insert(
        "opaque.bin".into(),
        vec![0_u8; MAX_BYTES as usize - files[Path::new("world.wl")].len()],
    );
    assert_eq!(
        files.values().map(|bytes| bytes.len() as u64).sum::<u64>(),
        MAX_BYTES
    );
    assert!(validate_files(&files).is_ok());
    assert!(prepare_import(files).is_err());
}

#[test]
fn prepare_import_returns_the_same_complete_files_that_save_will_encode() {
    let raw = Files::from([("world.wl".into(), b"event start".to_vec())]);
    assert!(!raw.contains_key(Path::new(MANIFEST)));
    let prepared = prepare_import(raw).unwrap();
    assert!(prepared.contains_key(Path::new(MANIFEST)));
    assert!(validate_files(&prepared).is_ok());
    let bytes = encode(&prepared).unwrap();
    assert_eq!(decode(&bytes).unwrap(), prepared);
}

#[test]
fn archive_rejects_incompressible_output_over_package_limit() {
    let mut bytes = vec![0_u8; MAX_BYTES as usize - 1024];
    let mut state = 0x9e37_79b9_u32;
    for byte in &mut bytes {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        *byte = state as u8;
    }
    let files = Files::from([
        ("world.wl".into(), b"event start".to_vec()),
        ("opaque.bin".into(), bytes),
    ]);
    assert!(validate_files(&files).is_ok());
    let error = prepare_import(files).unwrap_err();
    assert!(error.contains("压缩后的工程包超过 64 MiB"));
}

#[test]
fn project_package_reopens_unsaved_new_and_deleted_json_documents() {
    let root = std::env::temp_dir().join(format!(
        "worldedit-package-roundtrip-{}",
        std::process::id()
    ));
    let reopen = root.with_file_name(format!("worldedit-package-reopen-{}", std::process::id()));
    std::fs::create_dir_all(root.join("assets")).unwrap();
    std::fs::write(root.join("notes.json"), b"ordinary json").unwrap();
    std::fs::write(root.join("assets/reference.bin"), [0, 9, 255]).unwrap();
    let mut project = worldline_core::project::Project::new(&root);
    let manifest = root.join(".world/project.json");
    let map = root.join(".world/maps/new.json");
    project
        .create_authoring_document(
            &manifest,
            br#"{"schema_version":1,"entry":"world.wl","maps":{"new":".world/maps/new.json"}}"#
                .to_vec(),
        )
        .unwrap();
    let map_bytes = br#"{"opaque":17,"unknown":{"kept":true}}"#.to_vec();
    project
        .create_authoring_document(&map, map_bytes.clone())
        .unwrap();

    let mut package: Files = worldline_core::workspace_snapshot::snapshot_files(&project)
        .unwrap()
        .into_iter()
        .collect();
    package.insert(MANIFEST.into(), br#"{"entry":"world.wl"}"#.to_vec());
    let restored = decode(&encode(&package).unwrap()).unwrap();
    assert_eq!(
        restored[Path::new(".world/project.json")],
        package[Path::new(".world/project.json")]
    );
    assert_eq!(restored[Path::new(".world/maps/new.json")], map_bytes);
    assert_eq!(restored[Path::new("notes.json")], b"ordinary json");
    assert_eq!(restored[Path::new("assets/reference.bin")], [0, 9, 255]);

    for (relative, bytes) in &restored {
        let path = reopen.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    let reopened = worldline_core::project::Project::open(&reopen).unwrap();
    assert_eq!(
        reopened
            .authoring_document(&reopen.join(".world/maps/new.json"))
            .unwrap()
            .bytes(),
        map_bytes
    );
    assert_eq!(
        std::fs::read(reopen.join("notes.json")).unwrap(),
        b"ordinary json"
    );
    assert_eq!(
        std::fs::read(reopen.join("assets/reference.bin")).unwrap(),
        [0, 9, 255]
    );

    project.delete_authoring_document(&map).unwrap();
    let deleted: Files = worldline_core::workspace_snapshot::snapshot_files(&project)
        .unwrap()
        .into_iter()
        .collect();
    assert!(!deleted.contains_key(Path::new(".world/maps/new.json")));
}

#[test]
fn shared_export_roundtrips_into_the_desktop_compiler() {
    let root = std::env::temp_dir().join(format!("worldedit-web-test-{}", std::process::id()));
    let mut project = worldline_core::project::Project::new(&root);
    let before = project.compile().analysis.fingerprint;
    let files = decode(&encode(&project.export_files().unwrap()).unwrap()).unwrap();
    assert!(files.contains_key(Path::new("world.wl")));
    let sources = files
        .iter()
        .filter(|(p, _)| p.extension().is_some_and(|e| e == "wl"))
        .map(|(p, bytes)| (root.join(p), String::from_utf8(bytes.clone()).unwrap()))
        .collect();
    let result = worldline_core::compile_sources(&root.join(entry(&files).unwrap()), &sources);
    assert!(!result.has_errors());
    assert_eq!(before, result.analysis.fingerprint);
    project.mark_saved();
    assert!(!project.is_dirty());
    project
        .set_text(&project.entry.clone(), "broken draft".into())
        .unwrap();
    assert!(project.is_dirty());
}
