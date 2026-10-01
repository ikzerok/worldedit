//! 正式事件检查器的真实 egui 鼠标、键盘与滚动回归；不代替原生桌面验收。
mod protection;
mod support;
use support::*;
use worldline_core::project::Project;

#[test]
fn temporal_form_preserves_cross_period_edges_through_apply_undo_redo_and_reopen() {
    let mut h = Harness::new("1.13");
    let baseline = h.app.project.content_baseline();
    let original_edges = h.edges();
    let fingerprint = h.fingerprint();
    let entry = h.app.project.entry.clone();
    let manuscript = h.app.project.root.join(".world/manuscripts/book.json");
    let manuscript_bytes = h
        .app
        .project
        .authoring_document(&manuscript)
        .unwrap()
        .bytes()
        .to_vec();
    let text = h.text();
    assert!(text.contains("同一时间根（允许跨直接时段）"));
    assert!(text.contains("同名来信 (arrival)"));
    assert!(text.contains("时段：summer · 根：year"));
    assert!(text.contains("来源：world.wl:"));
    assert!(!text.contains("后继 (successor)"), "成环候选不能作为可选项");
    assert!(!text.contains("独立根事件 (foreign)"));
    h.period("autumn", "夏 · summer");
    assert_eq!(h.predecessors(), ["arrival"]);
    assert_eq!(h.app.project.content_baseline(), baseline);
    h.click("应用更改");
    assert_eq!(h.edges(), original_edges);
    assert_eq!(h.fingerprint(), fingerprint);
    assert_eq!(
        h.app
            .project
            .event_draft("archive")
            .unwrap()
            .1
            .period
            .as_deref(),
        Some("summer")
    );
    assert_eq!(
        h.app.project.event_draft("archive").unwrap().1.order,
        Some(20)
    );
    assert_eq!(
        h.app.project.event_draft("arrival").unwrap().1.order,
        Some(80)
    );
    assert_eq!(h.app.project.entry, entry);
    assert_eq!(
        h.app
            .project
            .authoring_document(&manuscript)
            .unwrap()
            .bytes(),
        manuscript_bytes
    );
    assert_eq!((h.app.history.len(), h.app.redo.len()), (1, 0));
    assert!(h.app.project.is_dirty());
    h.click("撤销");
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert_eq!(h.edges(), original_edges);
    h.click("重做");
    assert_eq!(
        h.app
            .project
            .event_draft("archive")
            .unwrap()
            .1
            .period
            .as_deref(),
        Some("summer")
    );
    assert_eq!(h.edges(), original_edges);
    assert_eq!(h.fingerprint(), fingerprint);
    assert!(h.app.save());
    let mut reopened = Project::open(&h.app.project.root).unwrap();
    let result = reopened.compile();
    let mut reopened_edges: Vec<_> = result
        .analysis
        .timeline
        .edges
        .iter()
        .map(|edge| (edge.before.clone(), edge.after.clone()))
        .collect();
    reopened_edges.sort();
    assert_eq!(reopened_edges, original_edges);
    assert_eq!(result.analysis.fingerprint, fingerprint);
    assert_eq!(
        reopened.authoring_document(&manuscript).unwrap().bytes(),
        manuscript_bytes
    );
}

#[test]
fn temporal_form_filters_metadata_selects_deep_same_root_and_cancels_without_writes() {
    let mut h = Harness::new("1.13");
    let baseline = h.app.project.content_baseline();
    let source_query = std::path::Path::new("chapters")
        .join("peer.wl")
        .display()
        .to_string();
    h.filter(&source_query);
    let text = h.text();
    assert!(text.contains("同名来信 (arrival)"), "筛选不隐藏已选关系");
    assert!(text.contains("候选匹配 1/1"));
    h.click("同名来信 (peer)");
    assert_eq!(h.predecessors(), ["arrival", "peer"]);
    h.click("同名来信 (arrival)");
    assert_eq!(h.predecessors(), ["peer"]);
    h.filter("no-match");
    assert!(h.text().contains("同名来信 (peer)"));
    assert!(h.text().contains("候选匹配 0/1"));
    h.click("取消编辑");
    assert!(h.app.event_editor.is_none());
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h.app.history.is_empty());
    h.app.select_event("archive");
    assert_eq!(h.predecessors(), ["arrival"]);
}

#[test]
fn temporal_form_cross_root_or_unassigned_period_keeps_invalid_selection_and_blocks_apply() {
    for (choice, selected) in [("另根 · other", "other"), ("未分配时段", "未分配时段")]
    {
        let mut h = Harness::new("1.13");
        let baseline = h.app.project.content_baseline();
        h.period("autumn", choice);
        assert_eq!(h.predecessors(), ["arrival"]);
        let text = h.text();
        assert!(text.contains("无法应用：1 条已选前驱需处理"), "{text}");
        assert!(text.contains("已选 · 需处理（1）"), "{text}");
        assert!(text.contains("同名来信 (arrival)"), "{text}");
        h.click("应用更改");
        assert_eq!(h.app.project.content_baseline(), baseline);
        assert!(h.app.history.is_empty());
        assert_eq!(h.predecessors(), ["arrival"]);
        h.period(selected, "秋 · autumn");
        assert_eq!(h.predecessors(), ["arrival"]);
        assert!(!h.text().contains("无法应用：1 条"));
    }
}

#[test]
fn temporal_form_legacy_versions_keep_direct_period_scope_and_draft_edges() {
    for version in ["1.9", "1.10", "1.11", "1.12"] {
        let mut h = Harness::new(version);
        let baseline = h.app.project.content_baseline();
        let text = h.text();
        assert!(text.contains("同一直接时段（当前语言版本）"));
        assert!(!text.contains("同名来信 (peer)"));
        h.period("autumn", "夏 · summer");
        assert_eq!(h.predecessors(), ["arrival"]);
        assert!(h.text().contains("无法应用：1 条已选前驱需处理"));
        h.click("应用更改");
        assert_eq!(h.app.project.content_baseline(), baseline);
        assert_eq!(h.app.project.language_version(), version);
    }
}

#[test]
fn temporal_form_final_transaction_rejects_invalid_successor_and_retains_draft() {
    let mut h = Harness::new("1.13");
    let baseline = h.app.project.content_baseline();
    h.click("同名来信 (arrival)");
    h.period("autumn", "另根 · other");
    assert!(h.predecessors().is_empty());
    h.click("应用更改");
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h.app.history.is_empty());
    assert!(
        h.app
            .io_error
            .as_deref()
            .is_some_and(|error| error.contains("A213")),
        "{:?}",
        h.app.io_error
    );
    let draft = &h.app.event_editor.as_ref().unwrap().draft;
    assert_eq!(draft.period.as_deref(), Some("other"));
    assert!(draft.predecessors.is_empty());
}
