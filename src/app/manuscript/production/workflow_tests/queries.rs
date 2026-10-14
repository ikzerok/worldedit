use super::*;

#[test]
fn production_flow_current_draft_scope_closure_and_paging_share_one_snapshot() {
    let _serial = serial();
    let mut flow = Flow::new(45);
    let disk = flow.disk();
    let baseline = flow.app.project.content_baseline();
    flow.stage("Line 00", "DRAFT_CURRENT_00");
    flow.click("生成当前稿台本");
    flow.finish_job();
    assert!(flow.app.production_is_current(), "{}", flow.notice());
    let snapshot = flow.snapshot();
    assert_eq!(snapshot.summary().matching_rows, 46);
    assert!(flow
        .rows()
        .iter()
        .any(|row| text(&row.source_parts).contains("DRAFT_CURRENT_00")));
    assert!(flow
        .rows()
        .iter()
        .all(|row| row.speaker.as_ref().unwrap().target.id == "a"));
    assert_eq!(
        flow.app
            .manuscript
            .production
            .page
            .as_ref()
            .unwrap()
            .rows
            .len(),
        40
    );
    flow.click("下一页台词");
    flow.frame(vec![]);
    let page = flow.app.manuscript.production.page.as_ref().unwrap();
    assert_eq!((page.offset, page.total, page.rows.len()), (40, 46, 6));
    assert_eq!(snapshot.key(), flow.snapshot().key());
    flow.preview();
    let artifact: Value = serde_json::from_slice(&flow.artifact()).unwrap();
    assert_eq!(artifact["rows"].as_array().unwrap().len(), 46);
    assert_eq!(artifact["scope_kind"], "current_target");

    flow.app.manuscript.navigation.session.text = "Selected".into();
    flow.app.manuscript.navigation.session.offset = 999;
    flow.app.manuscript.navigation.session.collapsed = vec!["one".into()];
    flow.click("书稿筛选章节");
    flow.click("从当前完整书稿筛选中明确勾选章节");
    flow.click("Selected One · one");
    flow.click("Selected Two · two");
    flow.generate();
    let snapshot = flow.snapshot();
    let summary = snapshot.summary();
    assert_eq!(summary.selected_chapter_occurrences, 2);
    assert_eq!(summary.root_targets, 1);
    assert_eq!(summary.definition_count, 4);
    assert_eq!(summary.added_fragment_definitions, 3);
    assert_eq!(summary.call_sites, 5);
    assert_eq!(summary.matching_rows, 46);
    assert!(summary.includes_fragment_closure && summary.complete);
    let leaf = flow
        .rows()
        .into_iter()
        .find(|row| row.stable_line_id.as_deref() == Some("leaf"))
        .unwrap();
    assert_eq!(leaf.external_call_uses.len(), 2);
    assert!(leaf.control_ancestry.is_empty());
    assert!(snapshot.call_sites().iter().any(|call| call
        .control_ancestry
        .iter()
        .any(|control| !control.evaluated)));

    flow.click("纳入共享片段定义");
    flow.generate();
    assert!(!flow.snapshot().summary().includes_fragment_closure);
    assert_eq!(flow.snapshot().summary().matching_rows, 45);
    flow.click("清空章节勾选");
    flow.generate();
    assert_eq!(flow.snapshot().summary().matching_rows, 0);
    assert_eq!(
        flow.app.manuscript.production.page.as_ref().unwrap().total,
        0
    );
    flow.click("全工程活动定义");
    flow.generate();
    assert_eq!(flow.snapshot().summary().matching_rows, 46);
    assert!(flow.snapshot().summary().includes_fragment_closure);
    assert_eq!(flow.app.project.content_baseline(), baseline);
    assert!(!flow.app.project.is_dirty());
    assert_eq!(flow.disk(), disk);
}

#[test]
fn production_flow_locale_status_search_filter_before_page_and_allow_empty_translation() {
    let _serial = serial();
    let mut flow = Flow::new(45);
    flow.app.manuscript.production.locale = "en".into();
    flow.generate();
    assert_eq!(flow.snapshot().summary().matching_rows, 46);
    assert!(flow
        .rows()
        .iter()
        .all(|row| row.status == ProductionStatus::Translated));
    let empty = flow
        .rows()
        .into_iter()
        .find(|row| row.stable_line_id.as_deref() == Some("main_1"))
        .unwrap();
    assert!(empty.selected_parts.is_empty());
    flow.preview();
    flow.confirm();
    // The unselected role and ordinary narration have no translations.
    // Selected-only strict delivery must still succeed.
    let bytes = flow.artifact();
    let output = flow.click("复制相同完整材料");
    assert_eq!(copied(&output).unwrap().as_bytes(), bytes);

    flow.app.manuscript.production.status = Some(ProductionStatus::Translated);
    flow.app.manuscript.production.search = "main_4".into();
    flow.app.manuscript.production.offset = 40;
    assert!(!flow.app.production_is_current());
    flow.generate();
    let page = flow.app.manuscript.production.page.as_ref().unwrap();
    assert_eq!((page.offset, page.total, page.rows.len()), (0, 6, 6));
    assert!(page
        .rows
        .iter()
        .all(|row| row.stable_line_id.as_ref().unwrap().starts_with("main_4")));
    flow.app.manuscript.production.status = Some(ProductionStatus::Missing);
    flow.generate();
    assert_eq!(flow.snapshot().summary().matching_rows, 0);
    flow.preview();
    let empty: Value = serde_json::from_slice(&flow.artifact()).unwrap();
    assert_eq!(empty["target_locale"], "en");
    assert_eq!(empty["speaker"]["id"], "a");
    assert!(empty["rows"].as_array().unwrap().is_empty());
}

#[test]
fn production_flow_locale_strict_failure_and_explicit_fallback_keep_exact_statuses() {
    let _serial = serial();
    let mut flow = Flow::new(5);
    let mut locale = flow.locale();
    locale["entries"].as_object_mut().unwrap().remove("main_2");
    locale["entries"]["main_3"]["source_revision"] = json!("old-revision");
    locale["entries"]["leaf"]["translation_parts"] =
        json!([{"type":"text","text":"lost placeholder"}]);
    flow.set_locale(&locale);
    flow.app.manuscript.production.locale = "en".into();
    flow.generate();
    for (id, expected) in [
        ("main_2", ProductionStatus::Missing),
        ("main_3", ProductionStatus::Stale),
        ("leaf", ProductionStatus::Invalid),
    ] {
        assert_eq!(
            flow.rows()
                .iter()
                .find(|row| row.stable_line_id.as_deref() == Some(id))
                .unwrap()
                .status,
            expected
        );
    }
    flow.preview();
    assert!(flow.app.manuscript.production.artifact.is_none());
    assert!(
        flow.notice().contains("LOCALE_INCOMPLETE"),
        "{}",
        flow.notice()
    );
    assert!(!flow.app.manuscript.production.confirmed);
    flow.click("交付范围与语言选项");
    flow.click("明确允许缺译、过期或无效行回退源文");
    flow.generate();
    assert_eq!(flow.snapshot().summary().source_fallback_rows, 3);
    for row in flow.rows().iter().filter(|row| row.used_source_fallback) {
        assert_eq!(row.selected_parts, row.source_parts);
        assert_ne!(row.status, ProductionStatus::Translated);
    }
    flow.preview();
    let artifact: Value = serde_json::from_slice(&flow.artifact()).unwrap();
    assert_eq!(artifact["locale_policy"], "source_fallback");
    assert_eq!(artifact["summary"]["source_fallback_rows"], 3);
    flow.confirm();
}

#[test]
fn production_flow_unknown_locale_and_global_duplicate_cannot_hide_behind_zero_filter() {
    let _serial = serial();
    let mut flow = Flow::new(3);
    flow.generate();
    flow.app.manuscript.production.search = "no matching text".into();
    flow.app.manuscript.production.locale = "not-registered".into();
    flow.start();
    flow.finish_job();
    assert!(!flow.app.production_is_current());
    assert!(
        flow.notice().contains("UNKNOWN_LOCALE"),
        "{}",
        flow.notice()
    );
    assert!(flow.app.manuscript.production.captured.is_none());
    flow.app.manuscript.production.locale.clear();
    let entry = flow.app.project.entry.clone();
    let source = flow.app.project.document(&entry).unwrap().replace(
        "say b \"UNSELECTED_SECRET\"",
        "say b \"UNSELECTED_SECRET\" #wl-localization:main_0",
    );
    flow.app.project.set_text(&entry, source).unwrap();
    flow.app.recompile();
    flow.start();
    flow.finish_job();
    assert!(!flow.app.production_is_current());
    assert!(flow.app.manuscript.production.captured.is_none());
    flow.app
        .finish_production_export(&flow.ctx, super::super::delivery::Action::Preview);
    assert!(flow.app.manuscript.production.artifact.is_none());
}

#[test]
fn production_flow_new_draft_role_is_selected_through_real_id_text_edit() {
    let _serial = serial();
    let mut flow = Flow::new(3);
    let disk = flow.disk();
    let baseline = flow.app.project.content_baseline();
    flow.stage(
        "character a as",
        "character fresh as \"草稿人物\"\ncharacter a as",
    );
    flow.stage("say a \"Line 00\"", "say fresh \"Line 00\"");
    assert!(!flow
        .app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .symbols
        .characters
        .contains_key("fresh"));
    // Only the ID field renders the exact text "a"; the candidate picker renders
    // "character:a". Use actual pointer focus and TextEdit keyboard events.
    flow.click("a");
    assert!(flow.ctx.memory(|memory| memory.focused()).is_some());
    flow.frame(vec![egui::Event::Key {
        key: egui::Key::A,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }]);
    flow.frame(vec![egui::Event::Text("fresh".into())]);
    assert_eq!(
        flow.app.manuscript.production.speaker,
        Some(TargetRef::new("character", "fresh"))
    );
    flow.click("生成当前稿台本");
    flow.finish_job();
    assert!(flow.app.production_is_current(), "{}", flow.notice());
    let rows = flow.rows();
    assert_eq!(rows.len(), 1);
    let speaker = rows[0].speaker.as_ref().unwrap();
    assert_eq!(speaker.target, TargetRef::new("character", "fresh"));
    assert_eq!(speaker.display, "草稿人物");
    assert!(text(&rows[0].source_parts).contains("Line 00"));
    flow.preview();
    flow.confirm();
    let artifact: Value = serde_json::from_slice(&flow.artifact()).unwrap();
    assert_eq!(artifact["speaker"]["id"], "fresh");
    assert_eq!(artifact["rows"].as_array().unwrap().len(), 1);
    assert_eq!(flow.app.project.content_baseline(), baseline);
    assert!(!flow.app.project.is_dirty());
    assert_eq!(flow.disk(), disk);
}
