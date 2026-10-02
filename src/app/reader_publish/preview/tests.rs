use super::*;

fn fixture(extra: serde_json::Value) -> (archive::Files, ReaderExportPreview) {
    let url = "maps/r0000000000000001.html";
    let preview = ReaderExportPreview {
        schema_version: 3,
        plan_digest: "test".into(),
        content_baseline: "test".into(),
        exclusions: vec![],
        included: vec![worldline_core::reader_export::ReaderExportIncluded {
            target: Some(TargetRef::new("map", "atlas")),
            manuscript_id: None,
            chapter_id: None,
            title: "地图".into(),
            output_path: url.into(),
        }],
        content: vec![worldline_core::reader_export::ReaderContentPreview {
            title: "地图".into(),
            output_path: url.into(),
            text: "公开地图\n灯塔 可公开标记\n".into(),
            empty_content: false,
        }],
    };
    let entries = serde_json::json!([
        {"title":"地图", "url":url,"text":preview.content[0].text,"kind":"map"}, extra
    ]);
    (
        BTreeMap::from([(
            std::path::PathBuf::from("search-index.json"),
            serde_json::to_vec(&entries).unwrap(),
        )]),
        preview,
    )
}

#[test]
fn public_map_anchor_entries_are_allowed_without_zipping_arrays_by_position() {
    let (files, preview) = fixture(
        serde_json::json!({"title":"灯塔","url":"maps/r0000000000000001.html#p-1234","text":"灯塔 可公开标记","kind":"map_placement"}),
    );
    assert!(validate_public_index(&files, &preview).is_ok());
}

#[test]
fn extra_unreviewed_pages_duplicate_urls_and_unreviewed_anchor_text_are_rejected() {
    for extra in [
        serde_json::json!({"title":"私密","url":"objects/private.html","text":"私密","kind":"entity"}),
        serde_json::json!({"title":"地图","url":"maps/r0000000000000001.html","text":"公开地图\n灯塔 可公开标记\n","kind":"map"}),
        serde_json::json!({"title":"私密","url":"maps/r0000000000000001.html#p-1234","text":"未授权新文字","kind":"map_placement"}),
        serde_json::json!({"title":"灯塔","url":"maps/r0000000000000001.html#../private","text":"灯塔 可公开标记","kind":"map_placement"}),
    ] {
        let (files, preview) = fixture(extra);
        assert!(validate_public_index(&files, &preview).is_err());
    }
}

#[test]
fn public_url_paths_reject_backslashes_on_every_platform() {
    for path in [
        "objects\\index.html",
        "objects/../index.html",
        "objects/%2e%2e/index.html",
        "C:/index.html",
        "//host/index.html",
    ] {
        assert!(validate_page_path(path).is_err(), "{path}");
    }
    assert!(validate_page_path("objects/index.html").is_ok());
}

#[test]
fn legacy_v1_without_content_projection_still_validates_all_index_urls() {
    let (_, mut preview) = fixture(serde_json::json!({}));
    preview.schema_version = 1;
    preview.content.clear();
    for url in [
        "objects\\o0001.html",
        "C:/index.html",
        "../index.html",
        "objects/index.html#../escape",
        "//host/index.html",
    ] {
        let files = BTreeMap::from([(
            std::path::PathBuf::from("search-index.json"),
            serde_json::to_vec(&serde_json::json!([{"title":"x", "url":url, "text":"x"}])).unwrap(),
        )]);
        assert!(
            validate_public_index(&files, &preview).is_err(),
            "v1错误接受了 {url}"
        );
    }
    let files = BTreeMap::from([(
        std::path::PathBuf::from("search-index.json"),
        serde_json::to_vec(
            &serde_json::json!([{"title":"x", "url":"objects/o0001.html", "text":"x"}]),
        )
        .unwrap(),
    )]);
    assert!(validate_public_index(&files, &preview).is_ok());
}
