use super::*;

#[test]
fn failed_play_attempt_is_frozen_across_render_frames() {
    let source = "event start\n  choice \"失败\" if rnd(1, 100) / 0 > 0\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    let _ = frame(&ctx, &mut app, Vec::new(), 20);
    assert!(app.play.as_ref().unwrap().error.is_some());
    let before = app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .save()
        .unwrap();
    for _ in 0..5 {
        let _ = frame(&ctx, &mut app, Vec::new(), 20);
    }
    let after = app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .save()
        .unwrap();
    assert_eq!(after, before, "错误后渲染不得重复失败求值或消耗 RNG");
}

#[test]
fn condition_inspector_clears_between_groups_restart_and_workspace_reset() {
    let source = "let score = 1\nevent start\n  choice \"锁定门\" if (score + 2 > 5) and false\n    -> END\n  choice \"继续\"\n    -> next\nevent next\n  choice \"终点\" if score > 0\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    click(&ctx, &mut app, 20, "解释当前条件（只读）");
    assert!(app
        .replay_debugger
        .explanations
        .as_ref()
        .unwrap()
        .iter()
        .any(|c| c.choice.label == "锁定门"));
    click(&ctx, &mut app, 20, "选择：继续");
    assert!(app.replay_debugger.explanations.is_none());
    click(&ctx, &mut app, 20, "解释当前条件（只读）");
    let shown = app.replay_debugger.explanations.as_ref().unwrap();
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].choice.node, "next");
    click(&ctx, &mut app, 20, "↻ 重新开始（已应用稿）");
    assert!(app.replay_debugger.explanations.is_none());
    click(&ctx, &mut app, 20, "解释当前条件（只读）");
    assert_eq!(
        app.replay_debugger.explanations.as_ref().unwrap()[0]
            .choice
            .node,
        "start"
    );
    app.reset_views();
    assert!(app.replay_debugger.explanations.is_none());
    assert!(app.play.is_none());
}

#[test]
fn condition_inspector_is_read_only_and_labels_old_run_after_source_edit() {
    let source = "event start\n  choice \"未满足的分支\" if not (true or false)\n    -> END\n  choice \"继续\"\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    let _ = frame(&ctx, &mut app, Vec::new(), 20);
    let before = app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .save()
        .unwrap();
    let trace = app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .replay_trace();
    let baseline = app.project.content_baseline();
    let history = app.history.len();
    let dirty = app.project.is_dirty();
    let disk_existed = app.project.root.exists();
    for _ in 0..10 {
        click(&ctx, &mut app, 20, "解释当前条件（只读）");
    }
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .save()
            .unwrap(),
        before
    );
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .replay_trace(),
        trace
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.history.len(), history);
    assert_eq!(app.project.is_dirty(), dirty);
    assert_eq!(app.project.root.exists(), disk_existed);
    let output = frame(&ctx, &mut app, Vec::new(), 20);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("未满足"), "{rendered}");
    assert!(rendered.contains("已满足"), "{rendered}");
    assert!(rendered.contains("not ((true or false))"), "{rendered}");
    assert!(
        !rendered.contains("求值错误"),
        "false 不是运行错误：{rendered}"
    );
    let run_version = app.play.as_ref().unwrap().version;
    app.project
        .set_text(
            &app.active_file.clone(),
            source.replace("not (true or false)", "true"),
        )
        .unwrap();
    app.version += 1;
    app.recompile();
    assert_eq!(app.play.as_ref().unwrap().version, run_version);
    let output = frame(&ctx, &mut app, Vec::new(), 20);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("旧运行"), "{rendered}");
    click(&ctx, &mut app, 20, "↻ 重新开始（已应用稿）");
    assert!(app.replay_debugger.explanations.is_none());
}

#[test]
fn failure_inspector_distinguishes_error_and_unexecuted_then_restart_clears_it() {
    let source = "event start\n  choice \"失败步骤\" if (rnd(1, 100) / 0 > 0) and (rnd(1, 100) > 0)\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    click(&ctx, &mut app, 20, "解释当前条件（只读）");
    let output = frame(&ctx, &mut app, Vec::new(), 20);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("求值错误"), "{rendered}");
    assert!(rendered.contains("未求值"), "{rendered}");
    assert!(app.play.as_ref().unwrap().paused);
    click(&ctx, &mut app, 20, "↻ 重新开始（已应用稿）");
    assert!(app.replay_debugger.explanations.is_none());
}

#[test]
fn failed_label_keeps_true_condition_evidence_without_claiming_false() {
    let source = "event start\n  choice \"标签 {1 / 0}\" if rnd(1, 100) > 0\n    -> END\n";
    let (ctx, mut app) = replay_app(source, 20);
    click(&ctx, &mut app, 20, "解释当前条件（只读）");
    let output = frame(&ctx, &mut app, Vec::new(), 20);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("求值错误"), "{rendered}");
    assert!(rendered.contains("已满足"), "{rendered}");
    assert!(!rendered.contains("未满足"), "{rendered}");
    assert!(app.play.as_ref().unwrap().paused);
}
