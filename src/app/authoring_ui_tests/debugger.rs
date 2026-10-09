use super::*;

#[test]
fn play_tab_exposes_replay_capture_and_condition_inspection_controls() {
    let (ctx, mut app) = app();
    let source =
        "event start\n  choice \"继续\" if false\n    -> END\n  choice \"结束\"\n    -> END\n";
    app.project
        .set_text(&app.active_file.clone(), source.into())
        .unwrap();
    app.recompile();
    app.tab = super::Tab::Play;
    click(&ctx, &mut app, 20, "▶ 开始试玩");

    let output = frame(&ctx, &mut app, Vec::new(), 20);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("保存当前路径"), "{rendered}");
    assert!(rendered.contains("解释当前条件"), "{rendered}");
    assert!(rendered.contains("种子"), "{rendered}");
}

#[test]
fn named_recorded_path_replays_against_the_current_snapshot_without_project_writes() {
    let source = concat!(
        "let score = 0\n",
        "event start\n",
        "  choice \"继续\"\n",
        "    set score = score + 1\n",
        "    -> END\n",
    );
    let (ctx, mut app) = replay_app(source, 20);
    let baseline = app.project.content_baseline();
    let history_len = app.history.len();

    click(&ctx, &mut app, 20, "选择：继续");
    click(&ctx, &mut app, 20, "● 保存当前路径");
    assert_eq!(app.replay_debugger.saved_paths.len(), 1);
    assert_eq!(app.replay_debugger.saved_paths[0].trace.steps.len(), 1);
    assert!(app.replay_debugger.saved_paths[0].trace.complete);

    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    wait_for_replay(&ctx, &mut app);
    let result = app.replay_debugger.result.as_ref().unwrap();
    assert!(matches!(
        result.status,
        worldline_runtime::ReplayStatus::Replayed {
            ended: true,
            complete: true
        }
    ));
    assert!(result.state_diff.contains_key("vars"));
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.history.len(), history_len);
}

#[test]
fn condition_explanation_uses_the_real_ui_action_without_advancing_story() {
    let source = concat!(
        "event start\n",
        "  choice \"不可选\" if false\n",
        "    -> END\n",
        "  choice \"继续\"\n",
        "    -> END\n",
    );
    let (ctx, mut app) = replay_app(source, 20);
    let _ = frame(&ctx, &mut app, Vec::new(), 20);
    let story = app.play.as_ref().unwrap().story.as_ref().unwrap();
    let before_trace = story.replay_trace();
    let before_state = story.state_view();
    let before_turns = story.turns();
    let baseline = app.project.content_baseline();

    click(&ctx, &mut app, 20, "解释当前条件（只读）");

    let story = app.play.as_ref().unwrap().story.as_ref().unwrap();
    assert_eq!(story.replay_trace(), before_trace);
    assert_eq!(story.state_view(), before_state);
    assert_eq!(story.turns(), before_turns);
    let explanations = app.replay_debugger.explanations.as_ref().unwrap();
    assert!(explanations.iter().any(|item| {
        item.choice.label == "不可选" && !item.available && item.unavailable_reason.is_some()
    }));
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn stale_recorded_choice_stops_and_ui_jumps_to_its_source_line() {
    let source = "event start\n  choice \"旧选择\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    click(&ctx, &mut app, 20, "选择：旧选择");
    click(&ctx, &mut app, 20, "● 保存当前路径");

    app.project
        .set_text(
            &app.active_file.clone(),
            "event start\n  choice \"新选择\"\n    -> END\n".into(),
        )
        .unwrap();
    app.recompile();
    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    wait_for_replay(&ctx, &mut app);

    let result = app.replay_debugger.result.as_ref().unwrap();
    let worldline_runtime::ReplayStatus::Diverged {
        reason,
        expected_choice,
        actual_choices,
        ..
    } = &result.status
    else {
        panic!("应停止在缺失的旧选择，实际结果：{:?}", result.status);
    };
    assert!(reason.contains("不匹配") || reason.contains("不存在"));
    assert!(expected_choice.is_none());
    assert_eq!(
        app.replay_debugger.saved_paths[0].trace.steps[0]
            .choice
            .label,
        "旧选择"
    );
    assert_eq!(actual_choices[0].label, "新选择");
    let actual_line = actual_choices[0].line;
    click(&ctx, &mut app, 20, "跳转到失败位置");
    assert_eq!(app.tab, super::Tab::Edit);
    assert_eq!(app.jump, Some((actual_line, 1)));
}

#[test]
fn removed_choice_stops_without_fallback_or_a_fabricated_current_source_location() {
    let source = "event start\n  choice \"待删除\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    click(&ctx, &mut app, 20, "选择：待删除");
    click(&ctx, &mut app, 20, "● 保存当前路径");

    app.project
        .set_text(
            &app.active_file.clone(),
            "event start\n  选择已经删除。\n  -> END\n".into(),
        )
        .unwrap();
    app.recompile();
    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    wait_for_replay(&ctx, &mut app);
    let result = app.replay_debugger.result.as_ref().unwrap();
    let worldline_runtime::ReplayStatus::Diverged { actual_choices, .. } = &result.status else {
        panic!("删除的选择必须停止并报告分歧：{:?}", result.status);
    };
    assert!(actual_choices.is_empty());

    assert!(result.current_node.is_none());
    let output = frame(&ctx, &mut app, Vec::new(), 20);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("当前停止位置不可定位"), "{rendered}");
    assert!(!rendered.contains("跳转到失败位置"), "{rendered}");
    assert_eq!(app.tab, super::Tab::Play);
    assert_eq!(
        app.replay_debugger.saved_paths[0].trace.steps[0]
            .choice
            .line,
        2
    );
}

#[test]
fn changed_node_stops_at_the_new_node_instead_of_reusing_the_old_choice_index() {
    let source = "event start\n  choice \"继续\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    click(&ctx, &mut app, 20, "选择：继续");
    click(&ctx, &mut app, 20, "● 保存当前路径");

    app.project
        .set_text(
            &app.active_file.clone(),
            "event replacement\n  choice \"继续\"\n    -> END\n".into(),
        )
        .unwrap();
    app.recompile();
    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    wait_for_replay(&ctx, &mut app);

    let result = app.replay_debugger.result.as_ref().unwrap();
    let worldline_runtime::ReplayStatus::Diverged { actual_choices, .. } = &result.status else {
        panic!("修改节点后应报告路径分歧：{:?}", result.status);
    };
    assert_eq!(actual_choices[0].node, "replacement");
    assert_eq!(
        app.replay_debugger.saved_paths[0].trace.steps[0]
            .choice
            .node,
        "start"
    );
}

#[test]
fn narrow_play_view_can_switch_between_body_and_debug_information() {
    let source = "event start\n  choice \"继续\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 21);
    click(&ctx, &mut app, 21, "调试信息");
    let output = frame(&ctx, &mut app, Vec::new(), 21);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("叙事调试信息"), "{rendered}");

    click(&ctx, &mut app, 21, "正文");
    let output = frame(&ctx, &mut app, Vec::new(), 21);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("正文"), "{rendered}");
}

#[test]
fn replay_state_delta_keeps_body_visible_in_narrow_play_pane() {
    let source = concat!(
        "let score = 0\n",
        "event start\n",
        "  NARROW_BODY_SENTINEL\n",
        "  choice \"继续\"\n",
        "    set score = score + 1\n",
        "    -> END\n",
    );
    let (ctx, mut app) = replay_app(source, 21);
    click(&ctx, &mut app, 21, "选择：继续");
    let baseline = app.project.content_baseline();
    let version = app.version;
    let history_len = app.history.len();
    let redo_len = app.redo.len();
    let story = app.play.as_ref().unwrap().story.as_ref().unwrap();
    let before_save = story.save().unwrap();
    let before_trace = story.replay_trace();

    // Compact play exposes recording through its real drawer, not a pre-opened test state.
    click(&ctx, &mut app, 21, "调试与路径");
    scroll_from_visible_anchor_to(&ctx, &mut app, 21, "调试与路径", "● 保存当前路径");
    click(&ctx, &mut app, 21, "● 保存当前路径");
    assert_eq!(app.replay_debugger.saved_paths.len(), 1);
    assert_eq!(app.replay_debugger.saved_paths[0].trace, before_trace);
    scroll_from_visible_anchor_to(&ctx, &mut app, 21, "叙事调试器", "▶ 重放所选路径");
    click(&ctx, &mut app, 21, "▶ 重放所选路径");
    // The shared legacy wait uses window 20; this regression must remain 700×640 throughout.
    for _ in 0..500 {
        let _ = frame(&ctx, &mut app, Vec::new(), 21);
        assert_eq!(ctx.screen_rect().size(), vec2(700.0, 640.0));
        if app.replay_debugger.job.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        app.replay_debugger.job.is_none(),
        "narrow replay must finish"
    );

    let output = frame(&ctx, &mut app, Vec::new(), 21);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    let body_visible = output.shapes.iter().any(|clipped| {
        text_position_contains(&clipped.shape, "NARROW_BODY_SENTINEL")
            .is_some_and(|position| clipped.clip_rect.contains(position))
    });
    assert!(
        app.replay_debugger
            .result
            .as_ref()
            .is_some_and(|result| result.state_diff.contains_key("vars")),
        "replay result must include the variable change: {:?}; trace: {:?}",
        app.replay_debugger.result,
        app.replay_debugger.saved_paths[0].trace
    );
    assert!(body_visible, "{rendered}");
    let story = app.play.as_ref().unwrap().story.as_ref().unwrap();
    assert_eq!(story.save().unwrap(), before_save);
    assert_eq!(story.replay_trace(), before_trace);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.version, version);
    assert_eq!(app.history.len(), history_len);
    assert_eq!(app.redo.len(), redo_len);
}

#[test]
fn full_editor_resize_keeps_narrow_debug_switch_accessible() {
    let source = concat!(
        "let score = 0\n",
        "event start\n",
        "  NARROW_RESIZE_BODY_SENTINEL\n",
        "  choice \"继续\"\n",
        "    set score = score + 1\n",
        "    -> END\n",
    );
    let (ctx, mut app) = replay_app(source, 29);
    click(&ctx, &mut app, 29, "选择：继续");
    click(&ctx, &mut app, 29, "● 保存当前路径");
    click(&ctx, &mut app, 29, "▶ 重放所选路径");
    wait_for_replay(&ctx, &mut app);

    let output = frame(&ctx, &mut app, Vec::new(), 30);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    let visible = |label| {
        output.shapes.iter().any(|clipped| {
            text_position(&clipped.shape, label)
                .is_some_and(|position| clipped.clip_rect.contains(position))
        })
    };
    assert!(visible("正文"), "{rendered}");
    assert!(visible("调试信息"), "{rendered}");

    click(&ctx, &mut app, 30, "调试信息");
    let output = frame(&ctx, &mut app, Vec::new(), 30);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("叙事调试信息"), "{rendered}");
    click(&ctx, &mut app, 30, "正文");
    let output = frame(&ctx, &mut app, Vec::new(), 30);
    assert!(output.shapes.iter().any(|clipped| {
        text_position_contains(&clipped.shape, "NARROW_RESIZE_BODY_SENTINEL")
            .is_some_and(|position| clipped.clip_rect.contains(position))
    }));
}

#[test]
fn event_graph_marks_only_visits_from_the_selected_replay() {
    let source = "event start\n  choice \"继续\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    click(&ctx, &mut app, 20, "选择：继续");
    click(&ctx, &mut app, 20, "● 保存当前路径");
    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    wait_for_replay(&ctx, &mut app);

    app.tab = super::Tab::Graph;
    let output = frame(&ctx, &mut app, Vec::new(), 22);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("访问 ×1"), "{rendered}");
    assert!(rendered.contains("无标记表示未测试"), "{rendered}");
}

#[test]
fn imported_trace_size_limit_is_enforced_without_rendering_or_retaining_it() {
    let source = "event start\n  choice \"继续\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    app.replay_debugger.import_json = "x".repeat(worldline_runtime::MAX_REPLAY_EXCHANGE_BYTES + 1);
    click(
        &ctx,
        &mut app,
        20,
        "导入路径 JSON（最多 4 MiB / 20,000 步）",
    );
    click(&ctx, &mut app, 20, "拒绝超限轨迹");

    assert!(app.replay_debugger.saved_paths.is_empty());
    assert!(app
        .replay_debugger
        .notice
        .as_deref()
        .is_some_and(|notice| notice.contains("4 MiB")));
}

#[test]
fn replay_cancellation_button_cancels_a_long_recorded_trace_and_keeps_it() {
    let source = "event start\n  choice \"再来\"\n    -> start\n";
    let (ctx, mut app) = replay_app(source, 20);
    let snapshot = app.snapshot.as_ref().unwrap();
    let mut story = worldline_runtime::Story::new_with_seed(
        &snapshot.result.program,
        &snapshot.result.analysis,
        7,
    )
    .unwrap();
    story.continue_story().unwrap();
    for _ in 0..20_000 {
        story.choose(0).unwrap();
        story.continue_story().unwrap();
    }
    let trace = story.replay_trace();
    assert_eq!(trace.steps.len(), 20_000);
    app.replay_debugger
        .saved_paths
        .push(super::SavedReplayPath {
            name: "长路径".into(),
            trace,
        });
    app.replay_debugger.selected_path = Some(0);
    app.replay_debugger.max_steps = 1_000_000_000;
    app.replay_debugger.time_budget_ms = 600_000;

    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    click_without_settling(&ctx, &mut app, 20, "取消重放");
    wait_for_replay(&ctx, &mut app);

    assert!(matches!(
        app.replay_debugger.result.as_ref().unwrap().status,
        worldline_runtime::ReplayStatus::Cancelled
    ));
    assert_eq!(app.replay_debugger.saved_paths[0].trace.steps.len(), 20_000);
    assert!(app.history.is_empty());
}

#[test]
fn pause_stop_and_checkpoint_import_controls_keep_debug_state_out_of_project() {
    let source = "event start\n  choice \"继续\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    let _ = frame(&ctx, &mut app, Vec::new(), 20);
    let baseline = app.project.content_baseline();
    let story = app.play.as_mut().unwrap().story.as_mut().unwrap();
    story.start_trace_from_here().unwrap();
    let checkpoint_trace = story.replay_trace();
    assert!(matches!(
        checkpoint_trace.origin,
        worldline_runtime::ReplayOrigin::Checkpoint { .. }
    ));
    app.replay_debugger.import_json = serde_json::to_string(&checkpoint_trace).unwrap();
    click(
        &ctx,
        &mut app,
        20,
        "导入路径 JSON（最多 4 MiB / 20,000 步）",
    );
    click(&ctx, &mut app, 20, "检查并导入路径");
    assert_eq!(app.replay_debugger.saved_paths.len(), 1);
    assert!(matches!(
        app.replay_debugger.saved_paths[0].trace.origin,
        worldline_runtime::ReplayOrigin::Checkpoint { .. }
    ));
    click(&ctx, &mut app, 20, "查看检查点状态");
    let output = frame(&ctx, &mut app, Vec::new(), 20);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("检查点起点"), "{rendered}");

    click(&ctx, &mut app, 20, "Ⅱ 暂停");
    assert!(app.play.as_ref().unwrap().paused);
    click(&ctx, &mut app, 20, "▶ 继续");
    assert!(!app.play.as_ref().unwrap().paused);
    click(&ctx, &mut app, 20, "■ 停止");
    assert!(app.play.as_ref().unwrap().stopped);
    assert!(!app.play.as_ref().unwrap().ended);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}

/// 超长轨迹 JSON 只占固定高度：其后的“复制 JSON”等控件必须仍留在调试面板可视区内。
#[test]
fn long_trace_json_keeps_the_following_debugger_controls_reachable() {
    let (ctx, mut app) = replay_app("event start\n  结束。\n  -> END\n", 20);
    app.replay_debugger.export_json = (0..300)
        .map(|index| format!("  \"step{index}\": {index},\n"))
        .collect();

    for _ in 0..3 {
        let _ = frame(&ctx, &mut app, Vec::new(), 20);
    }
    let output = frame(&ctx, &mut app, Vec::new(), 20);
    assert!(
        output
            .shapes
            .iter()
            .any(|shape| text_position(&shape.shape, "复制 JSON").is_some()),
        "超长轨迹 JSON 把后续控件挤出调试面板可视区（导出 JSON {} 字节）",
        app.replay_debugger.export_json.len()
    );

    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(
        !rendered.contains("ScrollArea ID"),
        "导入/导出 JSON 滚动区发生 egui id 冲突"
    );
}

mod exchange;
