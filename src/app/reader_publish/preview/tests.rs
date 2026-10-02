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
