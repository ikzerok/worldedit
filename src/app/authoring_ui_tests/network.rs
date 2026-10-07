use super::*;

#[test]
fn network_browsing_is_personal_until_shared_layout_is_saved() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let mut source = app.project.sources()[&entry].clone();
    source.push_str("relation_def r type knows from entity a to entity b\n");
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    app.open_network(TargetRef::new("entity", "a"));
    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 4);
    }
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
    assert_eq!(app.network_state.result.as_ref().unwrap().edges.len(), 1);

    app.network_view_id = "view_a".into();
    app.network_view_title = "A 的关联".into();
    click(&ctx, &mut app, 4, "保存共享布局");
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), 1);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .graph_index
        .views
        .contains_key("view_a"));
    assert_ne!(app.project.content_baseline(), baseline);
}

#[test]
fn presentation_preset_save_is_explicit_and_apply_is_personal() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let mut source = app.project.sources()[&entry].clone();
    source.push_str("relation_def r type knows from entity a to entity b\n");
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    app.open_network(TargetRef::new("entity", "a"));
    app.network_view_id = "view_a".into();
    app.network_view_title = "A 的关联".into();
    click(&ctx, &mut app, 4, "保存共享布局");

    let before_preset = app.project.content_baseline();
    let history_before = app.history.len();
    app.open_preset_editor(None);
    {
        let form = app.preset_editor.as_mut().unwrap();
        form.draft.id = "preset_a".into();
        form.draft.title = "A 的专题".into();
        form.draft.map_id = None;
        form.draft.graph_view_id = Some("view_a".into());
    }
    click(&ctx, &mut app, 6, "保存展示预设");
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.history.len(), history_before + 1);
    assert_ne!(app.project.content_baseline(), before_preset);
    assert!(app
        .snapshot
        .as_ref()
        .unwrap()
        .preset_index
        .presets
        .contains_key("preset_a"));

    let saved = app.project.content_baseline();
    let history_after_save = app.history.len();
    app.apply_presentation_preset("preset_a");
    assert_eq!(app.project.content_baseline(), saved);
    assert_eq!(app.history.len(), history_after_save);
    assert_eq!(app.network_state.focus, Some(TargetRef::new("entity", "a")));
}

#[cfg(not(debug_assertions))]
#[test]
fn network_release_profile_meets_m2_frame_and_reading_gates() {
    let (ctx, mut app) = app();
    let mut source = String::from("relation_type links as \"连接\"\n");
    for index in 0..1000 {
        source.push_str(&format!("entity e{index} kind place as \"对象 {index}\"\n"));
    }
    for index in 0..3000 {
        let from = if index < 600 { 0 } else { index % 1000 };
        let to = (index * 37 + 1) % 1000;
        source.push_str(&format!(
            "relation_def r{index} type links from entity e{from} to entity e{to}\n"
        ));
    }
    let entry = app.project.entry.clone();
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    app.open_network(TargetRef::new("entity", "e0"));

    for _ in 0..8 {
        let _ = frame(&ctx, &mut app, Vec::new(), 4);
    }
    let mut frame_ms = Vec::new();
    for _ in 0..160 {
        let started = std::time::Instant::now();
        let _ = frame(&ctx, &mut app, Vec::new(), 4);
        frame_ms.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    frame_ms.sort_by(f64::total_cmp);
    let p95_frame = frame_ms[(frame_ms.len() * 95 / 100).min(frame_ms.len() - 1)];
    let max_frame = *frame_ms.last().unwrap();

    let mut reading_ms = Vec::new();
    for index in 0..160 {
        app.open_reading(TargetRef::new("entity", &format!("e{}", index % 1000)));
        let started = std::time::Instant::now();
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1700.0, 1400.0))),
                ..Default::default()
            },
            |ctx| app.reading_window(ctx),
        );
        reading_ms.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    reading_ms.sort_by(f64::total_cmp);
    let p95_reading = reading_ms[(reading_ms.len() * 95 / 100).min(reading_ms.len() - 1)];
    let max_reading = *reading_ms.last().unwrap();

    println!(
        "WP10_PROFILE frame_p95_ms={p95_frame:.3} frame_max_ms={max_frame:.3} reading_p95_ms={p95_reading:.3} reading_max_ms={max_reading:.3}"
    );
    assert!(p95_frame <= 33.0, "网络帧 P95 {p95_frame:.3}ms 超过 33ms");
    assert!(
        p95_reading <= 200.0,
        "暖态资料切换 P95 {p95_reading:.3}ms 超过 200ms"
    );
}

#[test]
fn empty_network_has_a_real_searchable_center_picker_and_does_not_edit_the_project() {
    let (ctx, mut app) = app();
    super::super::object_picker::set_workspace_root(&ctx, &app.project.root);
    let baseline = app.project.content_baseline();
    let output = frame(&ctx, &mut app, Vec::new(), 37);
    assert!(visible_text_position(&output, "搜索中心对象").is_some());
    assert!(app.network_state.focus.is_none());
    click(&ctx, &mut app, 37, "请选择");
    enter_text_at_placeholder_in_window(&ctx, &mut app, 37, "搜索名称、类型、ID或来源", "a");
    let target = TargetRef::new("entity", "a");
    let label = super::super::object_picker::candidate_caption(
        app.snapshot
            .as_ref()
            .unwrap()
            .result
            .analysis
            .catalog
            .object(&target)
            .unwrap(),
        Some(&app.project.root),
    );
    click(&ctx, &mut app, 37, &label);
    assert_eq!(app.network_state.focus.as_ref(), Some(&target));
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

#[test]
fn network_borrowed_snapshot_keeps_live_relation_actions_and_refreshed_labels() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let mut source = app.project.sources()[&entry].clone();
    source.push_str("relation_def r type knows from entity a to entity b\n");
    app.project.set_text(&entry, source.clone()).unwrap();
    app.recompile();
    app.open_network(TargetRef::new("entity", "a"));
    let baseline = app.project.content_baseline();
    click(&ctx, &mut app, 4, "隐藏");
    assert!(app.network_state.hidden.contains("r"));
    click(&ctx, &mut app, 4, "显示");
    assert!(!app.network_state.hidden.contains("r"));
    click(&ctx, &mut app, 4, "认识 · r");
    assert_eq!(app.reading_target, Some(TargetRef::new("relation", "r")));
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());

    app.project
        .set_text(&entry, source.replace("认识", "已更新关系"))
        .unwrap();
    app.recompile();
    let after = app.project.content_baseline();
    app.reading_target = None;
    click(&ctx, &mut app, 4, "已更新关系 · r");
    assert_eq!(app.reading_target, Some(TargetRef::new("relation", "r")));
    assert_eq!(app.project.content_baseline(), after);
    assert!(app.history.is_empty());
}
