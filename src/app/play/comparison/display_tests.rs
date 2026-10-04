//! 只验证正式值和 catalog 的显示，不重新解释运行语义。
use super::*;
use worldline_runtime::Value;

fn display(value: Value) -> (String, Option<String>) {
    super::value(Some(&serde_json::to_value(value).unwrap()), false, None)
}
fn catalog() -> Catalog {
    let mut catalog = Catalog::default();
    for (id, display) in [
        ("returned", "归还"),
        ("returned_copy", "归还"),
        ("sold", "售出"),
    ] {
        catalog.tags.insert(
            id.into(),
            worldline_core::catalog::TagInfo {
                id: id.into(),
                display: display.into(),
                description: String::new(),
                properties: Default::default(),
                file: "world.wl".into(),
                line: 1,
                declared: true,
            },
        );
    }
    catalog
}

#[test]
fn numeric_boolean_and_string_values_use_runtime_display_and_kind() {
    assert_eq!(
        display(Value::Num(10.0)),
        ("10".into(), Some("数值".into()))
    );
    assert_eq!(
        display(Value::Num(10.5)),
        ("10.5".into(), Some("数值".into()))
    );
    assert_eq!(
        display(Value::Bool(false)),
        ("false".into(), Some("布尔".into()))
    );
    assert_eq!(
        display(Value::Str("false".into())),
        ("false".into(), Some("字符串".into()))
    );
    assert_eq!(
        display(Value::Str("透镜仍在 {coins} 枚硬币旁".into())).0,
        "透镜仍在 {coins} 枚硬币旁"
    );
    assert_eq!(display(Value::Tag("returned".into())).0, "returned");
    assert_eq!(
        display(Value::TagSet(vec!["returned".into()])).0,
        "[returned]"
    );
    assert_eq!(
        super::value(None, false, None),
        ("∅ 此侧没有此值".into(), None)
    );
}

#[test]
fn state_and_action_tags_use_catalog_display_without_merging_same_name_identities() {
    let catalog = catalog();
    let tags = vec!["returned".into(), "returned_copy".into()];
    assert_eq!(tags_text(&tags, Some(&catalog)), "归还 · 归还");
    assert_eq!(tag_ids(&tags), "returned, returned_copy");
    assert_eq!(
        super::value(Some(&serde_json::json!(tags)), true, Some(&catalog)),
        (
            "归还 · 归还".into(),
            Some("标签 ID：returned, returned_copy".into())
        )
    );
    assert_eq!(tags_text(&["unknown".into()], Some(&catalog)), "unknown");
    assert_eq!(tags_text(&[], Some(&catalog)), "空标签集");
}

#[test]
fn stale_tag_display_falls_back_to_recorded_ids_and_unknown_value_is_not_guessed() {
    let tags = vec!["returned".into(), "sold".into()];
    assert_eq!(tags_text(&tags, None), "returned · sold");
    assert_eq!(
        super::value(Some(&serde_json::json!(tags)), true, None),
        ("returned · sold".into(), None)
    );
    let (text, hint) = super::value(Some(&serde_json::json!({"NewKind": 10})), false, None);
    assert_eq!(text, "无法按当前变量格式展示");
    assert!(hint.unwrap().contains("NewKind"));
}
