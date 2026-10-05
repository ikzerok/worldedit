//! 展示只读 DTO 的语义，明确区分缺采集、零写入、部分结果和省略。
use super::*;
use worldline_runtime::{RouteCoverage, RouteOriginSummary, RouteStatus, VariableWriteEvidence};

fn side() -> RouteSideResult {
    RouteSideResult {
        origin: RouteOriginSummary {
            kind: "start".into(),
            seed: 42,
            checkpoint_fingerprint: None,
            checkpoint_digest: None,
        },
        original_fingerprint: 0,
        status: RouteStatus::Replayed,
        ended: true,
        complete: true,
        executed_steps: 0,
        completed_choices: 0,
        current_node: None,
        detail: None,
        divergence_step: None,
        states: Some(Default::default()),
        vars: Some(Default::default()),
        coverage: RouteCoverage::default(),
        state_actions: Default::default(),
        variable_writes: VariableWriteEvidence::default(),
        omitted: false,
    }
}

#[test]
fn variable_write_display_separates_uncaptured_zero_partial_and_omitted() {
    let mut side = side();
    assert!(evidence_summary(&side).contains("旧结果未提供变量写入证据"));
    assert!(empty_selection(&side, "score").contains("不能认定"));
    side.variable_writes.captured = true;
    assert_eq!(evidence_summary(&side), "本段完整验证，实际变量写入 0 项");
    assert_eq!(
        empty_selection(&side, "score"),
        "此侧本段没有对 score 的全局写入"
    );
    side.complete = false;
    assert!(evidence_summary(&side).contains("本段验证不完整"));
    assert!(empty_selection(&side, "score").contains("后续执行尚未验证"));
    side.variable_writes.total_writes = 500;
    side.variable_writes.omitted = true;
    assert!(evidence_summary(&side).contains("实际 500 项，保留 0 项"));
    assert!(empty_selection(&side, "score").contains("可能因额度省略"));
    side.complete = true;
    assert!(evidence_summary(&side).contains("遗漏不表示未发生"));
    // 另一证据类别的省略不会伪装成本类别的省略。
    side.variable_writes.omitted = false;
    side.omitted = true;
    assert!(!evidence_summary(&side).contains("已省略"));
}

#[test]
fn variable_write_values_preserve_types_empty_string_and_uninitialized() {
    assert_eq!(values::display(None), "未初始化（尚无此全局变量）");
    assert_eq!(
        values::display(Some(&Value::Str(String::new()))),
        "\"\"（空字符串） · 字符串"
    );
    assert_eq!(values::display(Some(&Value::Bool(false))), "false · 布尔");
    assert_eq!(
        values::display(Some(&Value::Str("false".into()))),
        "false · 字符串"
    );
    assert_eq!(values::display(Some(&Value::Num(0.0))), "0 · 数值");
    assert_eq!(
        values::display(Some(&Value::TagSet(vec!["x".into()]))),
        "[x] · 标签集合"
    );
}

#[test]
fn variable_write_preview_is_unicode_safe_explicit_and_full_value_remains_available() {
    let full = "星🌙e\u{301}".repeat(120);
    let (short, truncated) = values::preview(&full);
    assert!(truncated);
    assert!(short.starts_with(&full.chars().take(160).collect::<String>()));
    assert!(short.contains("已截断，共 480 字符"));
    assert_eq!(values::preview("你好🌙"), ("你好🌙".into(), false));
    assert_eq!(full.chars().count(), 480);
}

#[test]
fn old_route_side_without_variable_writes_is_not_reported_as_zero() {
    let mut json = serde_json::to_value(side()).unwrap();
    json.as_object_mut().unwrap().remove("variable_writes");
    let old: RouteSideResult = serde_json::from_value(json).unwrap();
    assert!(!old.variable_writes.captured);
    assert!(evidence_summary(&old).contains("旧结果未提供"));
}
