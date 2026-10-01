use super::support::*;
use egui::{pos2, vec2, Event};
use worldline_core::project::Project;

#[test]
fn temporal_form_dirty_title_apply_keeps_existing_edges_and_cancel_discards_only_form() {
    let mut h = Harness::new("1.13");
    let edges = h.edges();
    h.replace("档案馆密谈", "新的档案馆标题");
    assert!(h.app.dirty_draft_names().contains(&"事件正文与分支"));
    assert_eq!(h.predecessors(), ["arrival"]);
    h.click("应用更改");
    assert_eq!(
        h.app.project.event_draft("archive").unwrap().1.summary,
        "新的档案馆标题"
    );
    assert_eq!(h.edges(), edges);
    let baseline = h.app.project.content_baseline();
    h.replace("新的档案馆标题", "这份输入会取消");
    h.click("取消编辑");
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h.app.project.is_dirty(), "取消不抹掉已应用但未保存修改");
    assert_eq!(h.app.history.len(), 1);
}

#[test]
fn temporal_form_stale_baseline_preserves_input_and_never_overwrites_new_source() {
    let mut h = Harness::new("1.13");
    h.replace("档案馆密谈", "保留的旧窗输入");
    let entry = h.app.project.entry.clone();
    let changed = format!(
        "{}\n// new revision\n",
        h.app.project.document(&entry).unwrap()
    );
    h.app.project.set_text(&entry, changed).unwrap();
    h.app.recompile();
    let baseline = h.app.project.content_baseline();
    let text = h.text();
    assert!(text.contains("基线已过期"), "{text}");
    h.click("应用更改");
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert_eq!(
        h.app.event_editor.as_ref().unwrap().draft.summary,
        "保留的旧窗输入"
    );
    assert_eq!(h.predecessors(), ["arrival"]);
    assert!(h.app.history.is_empty());
}

#[test]
fn temporal_form_partial_unknown_capability_and_outside_root_never_offer_writable_options() {
    for case in ["partial", "unknown", "outside"] {
        let mut h = Harness::new("1.13");
        match case {
            "partial" => {
                let entry = h.app.project.entry.clone();
                let source = format!(
                    "{}\nevent broken during missing\n  -> END\n",
                    h.app.project.document(&entry).unwrap()
                );
                h.app.project.set_text(&entry, source).unwrap();
            }
            "unknown" => {
                let manifest = h.app.project.root.join(".world/project.json");
                let mut value: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
                value["required_features"] = serde_json::json!(["future.temporal.v999"]);
                std::fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
                h.app.project = Project::open(&h.app.project.root).unwrap();
            }
            _ => {
                h.app.event_editor.as_mut().unwrap().path =
                    h.app.project.root.parent().unwrap().join("outside.wl")
            }
        }
        h.app.recompile();
        // 以当前内容重开只读表单，单独覆盖投影阻断而不是只测旧基线。
        h.app.event_editor.as_mut().unwrap().baseline = h.app.project.content_baseline();
        h.app.event_editor.as_mut().unwrap().draft.summary = "不能丢的表单输入".into();
        let baseline = h.app.project.content_baseline();
        let text = h.text();
        assert!(text.contains("无法应用："), "{case}: {text}");
        assert!(text.contains("工程当前不可写"), "{case}: {text}");
        assert!(
            !text.contains("同名来信 (peer)"),
            "{case}: 不得伪造可写候选"
        );
        h.click("应用更改");
        assert_eq!(h.app.project.content_baseline(), baseline);
        assert_eq!(
            h.app.event_editor.as_ref().unwrap().draft.summary,
            "不能丢的表单输入"
        );
        assert_eq!(h.predecessors(), ["arrival"]);
        assert!(h.app.history.is_empty());
    }
}

#[test]
fn temporal_form_invalid_selected_ids_remain_explicit_until_author_removes_them() {
    let mut h = Harness::new("1.13");
    h.app.event_editor.as_mut().unwrap().draft.predecessors =
        vec!["archive".into(), "successor".into(), "missing".into()];
    let baseline = h.app.project.content_baseline();
    let text = h.text();
    assert!(text.contains("无法应用：3 条已选前驱需处理"));
    assert!(text.contains("事件不能以自身为前驱"));
    h.click("应用更改");
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert_eq!(h.predecessors(), ["archive", "successor", "missing"]);
    h.click("档案馆密谈 (archive)");
    assert_eq!(h.predecessors(), ["successor", "missing"]);
    assert!(h.text().contains("前驱事件不存在或当前分析无法确定其身份"));
}

#[test]
fn temporal_form_small_light_and_dark_layout_keeps_actions_identity_filter_and_scroll_usable() {
    for (size, dark) in [(vec2(1040.0, 660.0), false), (vec2(1188.0, 848.0), true)] {
        let mut h = Harness::new("1.13");
        h.size = size;
        crate::theme::configure(
            &h.ctx,
            if dark {
                crate::theme::ThemeMode::Dark
            } else {
                crate::theme::ThemeMode::Light
            },
        );
        let peer = h.app.project.root.join("chapters/peer.wl");
        let mut source = h.app.project.document(&peer).unwrap().to_owned();
        for index in 0..80 {
            source.push_str(&format!("event candidate_{index:03} during deep as \"同名且非常长的候选名称用于确认窄窗不会隐藏身份或挤走主要操作{index}\"\n  -> END\n"));
        }
        h.app.event_editor = None;
        h.app.project.set_text(&peer, source).unwrap();
        h.app.recompile();
        h.app.select_event("archive");
        let baseline = h.app.project.content_baseline();
        let output = h.output();
        for label in [
            "应用更改",
            "取消编辑",
            "当前事件：archive",
            "筛选候选：名称 / ID / 时段 / 根 / 来源",
        ] {
            assert!(
                position(&output, label).is_some(),
                "{size:?}: {label}必须可达"
            );
        }
        h.filter("candidate_079");
        assert!(h.text().contains("候选匹配 1/81"));
        // 真实滚轮遍历候选区域；无论区域与外层如何接力，顶部动作始终保留。
        for _ in 0..4 {
            h.wheel(pos2(size.x - 100.0, size.y - 130.0), -180.0);
        }
        let output = h.output();
        assert!(position(&output, "应用更改").is_some());
        assert!(position(&output, "取消编辑").is_some());
        assert!(position(&output, "当前事件：archive").is_some());
        assert_eq!(h.app.project.content_baseline(), baseline);
        assert_eq!(h.predecessors(), ["arrival"]);
        // 实际点击固定头部取消操作，不通过直接调用关闭方法。
        h.click("取消编辑");
        assert!(h.app.event_editor.is_none());
    }
}

#[test]
fn temporal_form_keyboard_can_activate_top_apply_without_pointer_and_cache_tracks_input() {
    let mut h = Harness::new("1.13");
    h.period("autumn", "夏 · summer");
    let opened_baseline = h.app.event_editor.as_ref().unwrap().baseline.clone();
    let cache_key = h
        .app
        .event_editor
        .as_ref()
        .unwrap()
        .temporal_cache
        .as_ref()
        .unwrap()
        .0
        .clone();
    h.frame(Vec::new());
    assert_eq!(
        h.app
            .event_editor
            .as_ref()
            .unwrap()
            .temporal_cache
            .as_ref()
            .unwrap()
            .0,
        cache_key
    );
    let mut applied = false;
    for _ in 0..100 {
        let output = h.frame(vec![
            key(egui::Key::Tab, true, egui::Modifiers::NONE),
            key(egui::Key::Tab, false, egui::Modifiers::NONE),
        ]);
        let on_apply = output.platform_output.events.iter().any(|event| matches!(event,
            egui::output::OutputEvent::FocusGained(info) if info.typ == egui::WidgetType::Button && info.label.as_deref() == Some("应用更改")
        ));
        if on_apply {
            h.frame(vec![
                key(egui::Key::Enter, true, egui::Modifiers::NONE),
                key(egui::Key::Enter, false, egui::Modifiers::NONE),
            ]);
            applied = true;
            break;
        }
    }
    assert!(applied, "Tab应可到达应用按钮");
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
    assert_eq!(h.predecessors(), ["arrival"]);
    assert_eq!(h.app.history.len(), 1);
    assert_ne!(
        h.app.event_editor.as_ref().unwrap().baseline,
        opened_baseline
    );
    h.frame(vec![Event::PointerGone]);
}

#[test]
fn temporal_form_same_baseline_registry_diagnostics_invalidate_cached_candidates() {
    let mut h = Harness::new("1.13");
    let manifest = h.app.project.root.join(".world/project.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(h.app.project.authoring_document(&manifest).unwrap().bytes())
            .unwrap();
    value["required_features"] =
        serde_json::json!(["presentation.manuscripts.v1", "collaboration.comments.v1"]);
    value["comments"] = serde_json::json!({
        "a":".world/comments/a.json", "b":".world/comments/b.json"
    });
    h.app
        .project
        .set_authoring_document(&manifest, serde_json::to_vec(&value).unwrap())
        .unwrap();
    let a = h.app.project.root.join(".world/comments/a.json");
    let b = h.app.project.root.join(".world/comments/b.json");
    for path in [&a, &b] {
        h.app
            .project
            .create_authoring_document(path, b"{}".to_vec())
            .unwrap();
    }
    h.app.project.save().unwrap();
    h.app
        .project
        .set_authoring_document(&b, br#"{"local":true}"#.to_vec())
        .unwrap();
    h.app.event_editor = None;
    h.app.recompile();
    h.app.select_event("archive");
    h.replace("档案馆密谈", "应保留的事件输入");
    assert!(h.text().contains("同名来信 (peer)"));
    let baseline = h.app.project.content_baseline();
    std::fs::remove_file(&b).unwrap();
    std::fs::hard_link(&a, &b).unwrap();
    assert!(h.app.project.refresh().unwrap().is_empty());
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h
        .app
        .project
        .authoring_diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.code == "WS004"));
    assert!(!h.app.stale_form);
    // 不重新编译、改草稿基线或手动清缓存；真实注册诊断变化必须使投影失效。
    let text = h.text();
    assert!(
        text.contains("无法应用：工作区清单含有不支持的能力"),
        "{text}"
    );
    assert!(text.contains("工程当前不可写"), "{text}");
    assert!(!text.contains("同名来信 (peer)"), "{text}");
    h.click("应用更改");
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert_eq!(h.predecessors(), ["arrival"]);
    assert_eq!(
        h.app.event_editor.as_ref().unwrap().draft.summary,
        "应保留的事件输入"
    );
    assert_eq!(
        h.app.project.authoring_document(&b).unwrap().bytes(),
        br#"{"local":true}"#
    );
    assert!(h.app.history.is_empty());
}

#[test]
fn temporal_form_stale_flag_disables_new_candidates_but_preserves_explicit_deselection() {
    let mut h = Harness::new("1.13");
    h.replace("档案馆密谈", "外部冲突后保留的输入");
    let baseline = h.app.project.content_baseline();
    h.app.stale_form = true;
    h.filter("peer");
    assert!(h.text().contains("旧基线只读：不能新增前驱"));
    let list_point = position(&h.output(), "同名来信 (arrival)").unwrap();
    for _ in 0..8 {
        if position(&h.output(), "同名来信 (peer)").is_some() {
            break;
        }
        h.wheel(list_point, -80.0);
    }
    let candidate_point =
        position(&h.output(), "同名来信 (peer)").expect("有界真实滚动应显示禁用的候选");
    h.click("同名来信 (peer)");
    assert_eq!(h.predecessors(), ["arrival"]);
    h.click("应用更改");
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h.app.history.is_empty());
    for _ in 0..8 {
        if position(&h.output(), "同名来信 (arrival)").is_some() {
            break;
        }
        h.wheel(candidate_point, 80.0);
    }
    h.click("同名来信 (arrival)");
    assert!(
        h.predecessors().is_empty(),
        "过期输入仍允许明确取消已有前驱"
    );
    assert_eq!(
        h.app.event_editor.as_ref().unwrap().draft.summary,
        "外部冲突后保留的输入"
    );
    assert_eq!(h.app.project.content_baseline(), baseline);
}
