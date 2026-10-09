use super::workbench_tests::{click, fixture, frame, stage};
use super::*;
use worldline_core::localization::LocalizationStatus;

#[test]
fn localization_saved_catalog_queries_do_not_create_unsubmitted_work() {
    let (mut project, mut state) = fixture(85);
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    stage(&mut state, "已保存的译文");
    plans::preview_edits(&project, &mut state);
    jobs::settle(&project, &mut state, 1);
    assert!(click(
        &egui::Context::default(),
        &mut project,
        &mut state,
        "应用到工程（可撤销）"
    ));
    assert!(!state.has_unsubmitted_work());
    project.save().unwrap();
    let mut reopened = Project::open(&project.entry).unwrap();
    let baseline = reopened.content_baseline();
    let ctx = egui::Context::default();
    let mut state = LocalizationUiState::default();
    frame(
        &ctx,
        &mut reopened,
        &mut state,
        egui::vec2(1200.0, 900.0),
        vec![],
    );
    jobs::settle(&reopened, &mut state, 1);
    assert!(!state.has_unsubmitted_work());
    click(&ctx, &mut reopened, &mut state, "已有语言");
    click(&ctx, &mut reopened, &mut state, "zh-Hant");
    jobs::settle(&reopened, &mut state, 1);
    assert_eq!(state.source_locale, "en");
    assert!(!state.has_unsubmitted_work(), "自动填源语言不是作者输入");
    click(&ctx, &mut reopened, &mut state, "全部状态");
    click(&ctx, &mut reopened, &mut state, "已译 · 1");
    assert_eq!(state.workbench.status, Some(LocalizationStatus::Translated));
    // Filters invalidate after this frame's refresh; the next UI frame queues the new query.
    frame(
        &ctx,
        &mut reopened,
        &mut state,
        egui::vec2(1200.0, 900.0),
        vec![],
    );
    jobs::settle(&reopened, &mut state, 1);
    let page = state.workbench.page.as_ref().unwrap();
    assert_eq!((page.total, page.all_total), (1, 85));
    assert_eq!(
        page.status_counts.get(&LocalizationStatus::Translated),
        Some(&1)
    );
    assert_eq!(
        page.status_counts
            .get(&LocalizationStatus::MissingTranslation),
        Some(&84)
    );
    assert!(!state.has_unsubmitted_work());
    assert!(state.workbench.drafts.is_empty());
    assert!(state.workbench.id_inputs.is_empty());
    click(&ctx, &mut reopened, &mut state, "高级 JSON 交换");
    assert!(!state.has_unsubmitted_work(), "查看高级默认值不是作者编辑");
    click(&ctx, &mut reopened, &mut state, "译文目录");
    click(&ctx, &mut reopened, &mut state, "刷新目录");
    frame(
        &ctx,
        &mut reopened,
        &mut state,
        egui::vec2(1200.0, 900.0),
        vec![],
    );
    jobs::settle(&reopened, &mut state, 1);
    assert!(!state.has_unsubmitted_work());
    assert_eq!(reopened.content_baseline(), baseline);
    assert!(!reopened.is_dirty());
    std::fs::remove_dir_all(&project.root).unwrap();
}

fn author_input(state: &mut LocalizationUiState, kind: &str) {
    match kind {
        "configuration" => {
            state.exchange_locales = Some(("ja".into(), "fr".into()));
            state.config_edited = true;
        }
        "whitelist" => state.string_ids = "pending-id".into(),
        "JSON" => state.exchange_json = "{未完成的 JSON".into(),
        _ => unreachable!(),
    }
}

#[test]
fn localization_query_and_runtime_navigation_preserve_each_advanced_input() {
    for kind in ["configuration", "whitelist", "JSON"] {
        let (_, mut state) = fixture(1);
        author_input(&mut state, kind);
        let selection = state.selection();
        let json = state.exchange_json.clone();
        assert!(state.has_unsubmitted_work(), "{kind}");
        state.advanced = false;
        state.target_locale = "de".into();
        invalidate_selection(&mut state, false);
        assert!(state.has_unsubmitted_work(), "查询隐藏了 {kind}");
        state.open_translation("zh-Hant", Some("line0"));
        assert!(state.has_unsubmitted_work(), "试玩回源隐藏了 {kind}");
        if kind == "configuration" {
            assert_eq!(state.selection().source_locale, selection.source_locale);
            assert_eq!(state.selection().target_locale, selection.target_locale);
        }
        assert_eq!(state.selection().string_ids, selection.string_ids);
        assert_eq!(state.exchange_json, json);
    }
}

#[test]
fn localization_browsing_does_not_reopen_submitted_exchange_inputs() {
    let (_, mut state) = fixture(1);
    author_input(&mut state, "configuration");
    author_input(&mut state, "whitelist");
    author_input(&mut state, "JSON");
    state.config_submitted = true;
    state.exchange_submitted = true;
    assert!(!state.has_unsubmitted_work());
    state.target_locale = "de".into();
    invalidate_selection(&mut state, false);
    assert!(!state.has_unsubmitted_work());
    state.open_translation("zh-Hant", Some("line0"));
    assert!(!state.has_unsubmitted_work());
    assert_eq!(state.exchange_locales(), ("ja", "fr"));
    invalidate_selection(&mut state, true);
    assert!(state.has_unsubmitted_work(), "新的高级配置编辑仍须保护");
    state.config_submitted = true;
    state.invalidate_import();
    assert!(state.has_unsubmitted_work(), "新的 JSON 编辑仍须保护");
}

#[test]
fn localization_cleared_whitelist_does_not_turn_seeded_locales_into_a_draft() {
    let (_, mut state) = fixture(1);
    state.exchange_locales = Some(("en".into(), "zh-Hant".into()));
    state.string_ids = "pending-id".into();
    invalidate_selection(&mut state, true);
    assert!(state.has_unsubmitted_work());
    state.string_ids.clear();
    invalidate_selection(&mut state, true);
    assert!(!state.has_unsubmitted_work());
}

#[test]
fn localization_catalog_apply_does_not_submit_unrelated_advanced_input() {
    for id_assignment in [false, true] {
        for kind in ["configuration", "whitelist", "JSON"] {
            let (mut project, mut state) = fixture(1);
            if id_assignment {
                let entry = project.entry.clone();
                project
                    .set_text(&entry, "event start\n  待分配 ID\n  -> END\n".into())
                    .unwrap();
            }
            catalog::refresh(&project, &mut state, 1);
            jobs::settle(&project, &mut state, 1);
            author_input(&mut state, kind);
            let selection = state.selection();
            let json = state.exchange_json.clone();
            if id_assignment {
                let entry = state.workbench.selected_entry().unwrap().clone();
                state
                    .workbench
                    .id_inputs
                    .insert(entry.unit_key.clone(), "assigned".into());
                plans::preview_id(&project, &mut state, &entry);
            } else {
                stage(&mut state, "目录译文");
                plans::preview_edits(&project, &mut state);
            }
            jobs::settle(&project, &mut state, 1);
            assert!(click(
                &egui::Context::default(),
                &mut project,
                &mut state,
                "应用到工程（可撤销）"
            ));
            assert!(!state.workbench.has_input());
            assert!(
                state.has_unsubmitted_work(),
                "id_assignment={id_assignment}, {kind}"
            );
            assert_eq!(state.selection().source_locale, selection.source_locale);
            assert_eq!(state.selection().target_locale, selection.target_locale);
            assert_eq!(state.selection().string_ids, selection.string_ids);
            assert_eq!(state.exchange_json, json);
        }
    }
}

#[test]
fn localization_each_catalog_input_survives_filter_page_language_and_runtime_return() {
    for id_input in [false, true] {
        let (project, mut state) = fixture(85);
        catalog::refresh(&project, &mut state, 1);
        jobs::settle(&project, &mut state, 1);
        let key = state.workbench.selected_entry().unwrap().unit_key.clone();
        if id_input {
            state
                .workbench
                .id_inputs
                .insert(key.clone(), "retained-id".into());
        } else {
            stage(&mut state, "保留译文");
        }
        state.workbench.offset = 40;
        state.workbench.invalidate();
        catalog::refresh(&project, &mut state, 1);
        jobs::settle(&project, &mut state, 1);
        assert!(state.has_unsubmitted_work());
        state.target_locale = "fr".into();
        state.workbench.search = "无匹配结果".into();
        state.workbench.offset = 0;
        invalidate_selection(&mut state, false);
        catalog::refresh(&project, &mut state, 1);
        jobs::settle(&project, &mut state, 1);
        assert_eq!(state.workbench.page.as_ref().unwrap().total, 0);
        assert!(state.has_unsubmitted_work());
        state.open_translation("zh-Hant", Some("line84"));
        catalog::refresh(&project, &mut state, 1);
        jobs::settle(&project, &mut state, 1);
        assert!(state.has_unsubmitted_work());
        assert_eq!(
            state.workbench.selected_entry().unwrap().id.as_deref(),
            Some("line84")
        );
        if id_input {
            assert_eq!(
                state.workbench.id_inputs.get(&key).map(String::as_str),
                Some("retained-id")
            );
        } else {
            assert_eq!(state.workbench.drafts.len(), 1);
        }
    }
}

#[test]
fn localization_clearing_operation_status_does_not_clear_author_inputs() {
    let (project, mut state) = fixture(1);
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    stage(&mut state, "保留译文");
    author_input(&mut state, "configuration");
    author_input(&mut state, "whitelist");
    author_input(&mut state, "JSON");
    state
        .workbench
        .id_inputs
        .insert("key".into(), "retained-id".into());
    state.status = Some(Ok("已一次应用到工程；可撤销，保存后落盘".into()));
    state.clear_operation_status();
    assert!(state.status.is_none());
    assert!(state.has_unsubmitted_work());
    assert_eq!(state.exchange_locales(), ("ja", "fr"));
    assert_eq!(state.string_ids, "pending-id");
    assert_eq!(state.exchange_json, "{未完成的 JSON");
    assert_eq!(state.workbench.drafts.len(), 1);
    assert_eq!(state.workbench.id_inputs["key"], "retained-id");
}
