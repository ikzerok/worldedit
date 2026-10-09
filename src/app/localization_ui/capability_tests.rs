//! The initial capability route must also work inside the real 763×541 / 200% app shell.
use super::short_viewport_tests::Harness;
use super::*;

struct InputSnapshot {
    ids: std::collections::BTreeMap<String, String>,
    other: serde_json::Value,
}

fn input_snapshot(state: &LocalizationUiState) -> InputSnapshot {
    InputSnapshot {
        ids: state.workbench.id_inputs.clone(),
        other: serde_json::json!({
            "whitelist": state.string_ids,
            "json": state.exchange_json, "locales": state.exchange_locales,
            "drafts": state.workbench.drafts.iter().map(|(key, value)|
                (key, &value.source_locale, &value.target_locale, &value.source_baseline, &value.edit)).collect::<Vec<_>>()
        }),
    }
}

fn assert_retained_inputs(state: &LocalizationUiState, before: &InputSnapshot) {
    for (key, value) in &before.ids {
        assert_eq!(
            state.workbench.id_inputs.get(key),
            Some(value),
            "every previously existing input key/value must remain: {key}"
        );
    }
    for (key, value) in &state.workbench.id_inputs {
        if !before.ids.contains_key(key) {
            assert!(value.is_empty(), "a newly rendered source identity may only add an empty placeholder, not authored data: {key}={value:?}");
        }
    }
    assert_eq!(input_snapshot(state).other, before.other);
}

fn assert_window_bounds(h: &Harness) {
    let rect = h
        .ctx
        .memory(|memory| memory.area_rect(egui::Id::new("language-capabilities")))
        .unwrap();
    assert!(
        h.ctx.screen_rect().expand(0.5).contains_rect(rect),
        "complete capability frame exceeds the 200% screen: window={rect:?}, screen={:?}",
        h.ctx.screen_rect()
    );
}

#[test]
fn localization_short_200_capability_success_clears_old_error_only_after_real_enable_and_keeps_inputs(
) {
    let mut h = Harness::new("event start\n  尚无 ID 的原文\n  -> END\n");
    let manifest = h.app.project.root.join(".world/project.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(h.app.project.authoring_document(&manifest).unwrap().bytes())
            .unwrap();
    config["required_features"] = serde_json::json!([]);
    h.app
        .project
        .set_authoring_document(&manifest, serde_json::to_vec(&config).unwrap())
        .unwrap();
    h.app.recompile();
    h.settle();
    assert!(!h.app.project.compile_options().localization_ids);
    h.click("当前源文与译文");
    h.replace("例如 chapter01_welcome", "greeting");
    h.app.localization_ui.string_ids = "keep_other_id".into();
    h.app.localization_ui.exchange_json = "未提交 JSON 原文😀".into();
    h.click("预览此 ID 修改");
    let old_error = h.app.localization_ui.status.clone();
    let entry = h.app.localization_ui.workbench.selected_entry().unwrap();
    let original_key = entry.unit_key.clone();
    let draft = worldline_core::localization::LocalizationIdDraft {
        schema_version: 1,
        source_baseline: h
            .app
            .localization_ui
            .workbench
            .page
            .as_ref()
            .unwrap()
            .source_baseline
            .clone(),
        assignments: vec![worldline_core::localization::LocalizationIdAssignment {
            source: entry.source.clone().unwrap(),
            source_revision: entry.source_revision.clone().unwrap(),
            expected_id: entry.id.clone(),
            id: h.app.localization_ui.workbench.id_inputs[&entry.unit_key].clone(),
        }],
    };
    let core_error = h.app.project.preview_localization_ids(&draft).unwrap_err();
    assert_eq!(core_error.code, "FEATURE_REQUIRED");
    assert_eq!(
        core_error.to_string(),
        "请先显式启用 content.localization.v1"
    );
    assert_eq!(old_error, Some(Err(core_error.to_string())));
    let inputs = input_snapshot(&h.app.localization_ui);
    let baseline = h.app.project.content_baseline();
    h.click("查看语言与资料能力…");
    assert_window_bounds(&h);
    h.click("取消 / 保留当前设置");
    assert!(h.app.capability_ui.is_none());
    assert_eq!(h.app.localization_ui.status, old_error);
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert_retained_inputs(&h.app.localization_ui, &inputs);

    h.click("查看语言与资料能力…");
    h.click("额外资料能力与兼容要求");
    h.click("本地化文本身份 · content.localization.v1");
    h.click("预览全稿兼容影响");
    assert_window_bounds(&h);
    h.click("我已查看兼容影响；旧存档/检查点须匹配指纹，入口轨迹须重新严格验证");
    let source = format!(
        "{}// baseline changed after capability preview\n",
        h.app.project.document(&h.app.project.entry).unwrap()
    );
    h.app
        .project
        .set_text(&h.app.project.entry.clone(), source)
        .unwrap();
    h.app.recompile();
    h.click("确认启用（不自动保存）");
    assert!(
        h.app.capability_ui.is_some(),
        "stale capability plan must fail"
    );
    assert!(!h.app.project.compile_options().localization_ids);
    assert_eq!(h.app.localization_ui.status, old_error);
    assert_retained_inputs(&h.app.localization_ui, &inputs);

    h.click("预览全稿兼容影响");
    assert_window_bounds(&h);
    h.click("我已查看兼容影响；旧存档/检查点须匹配指纹，入口轨迹须重新严格验证");
    h.click("确认启用（不自动保存）");
    assert!(h.app.capability_ui.is_none());
    assert!(h.app.project.compile_options().localization_ids);
    assert!(h.app.localization_ui.status.is_none());
    assert!(h.app.message.as_deref().unwrap().contains("已显式启用"));
    assert_retained_inputs(&h.app.localization_ui, &inputs);
    assert!(h.app.localization_ui.has_unsubmitted_work());
    assert_eq!(h.ctx.screen_rect().size(), egui::vec2(381.5, 270.5));
    assert_eq!(h.ctx.pixels_per_point(), 2.0);
    let current_key = h
        .app
        .localization_ui
        .workbench
        .selected_entry()
        .unwrap()
        .unit_key
        .clone();
    assert_ne!(
        current_key, original_key,
        "new source baseline owns a new catalog identity"
    );
    assert_eq!(
        h.app.localization_ui.workbench.id_inputs[&original_key],
        "greeting"
    );
    assert!(
        h.app.localization_ui.workbench.id_inputs[&current_key].is_empty(),
        "old author input must not be silently migrated"
    );
    h.replace("例如 chapter01_welcome", "greeting");
    assert_eq!(
        h.app.localization_ui.workbench.id_inputs[&original_key],
        "greeting"
    );
    assert_eq!(
        h.app.localization_ui.workbench.id_inputs[&current_key],
        "greeting"
    );
    h.click("预览此 ID 修改");
    let Some(plans::Preview::Id { plan, .. }) = &h.app.localization_ui.workbench.preview else {
        panic!("fresh ID preview after capability enable")
    };
    assert!(plan.can_apply);
}
