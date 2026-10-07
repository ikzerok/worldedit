//! 真实 UI 入口与 live Story、稿件及保存基线隔离。
use super::*;
use worldline_runtime::{decode_replay_trace, encode_replay_trace, ReplayTrace, Story};

fn unchanged(app: &WorldeditApp) -> (String, ReplayTrace, String, bool, u64, usize) {
    let story = app.play.as_ref().unwrap().story.as_ref().unwrap();
    (
        story.save().unwrap(),
        story.replay_trace(),
        app.project.content_baseline(),
        app.project.is_dirty(),
        app.version,
        app.history.len(),
    )
}

#[test]
fn exchange_ui_export_import_roundtrip_preserves_live_rng_and_project() {
    let (ctx, mut app) = replay_app(
        "event start\n  choice \"继续 {rnd(1, 99)}\"\n    -> END\n",
        20,
    );
    click(&ctx, &mut app, 20, "● 保存当前路径");
    let before = unchanged(&app);
    click(&ctx, &mut app, 20, "导出所选路径 JSON");
    let json = app.replay_debugger.export_json.clone();
    assert_eq!(decode_replay_trace(json.as_bytes()).unwrap(), before.1);
    assert_eq!(encode_replay_trace(&before.1).unwrap(), json);
    app.replay_debugger.import_json = json.clone();
    click(
        &ctx,
        &mut app,
        20,
        "导入路径 JSON（最多 4 MiB / 20,000 步）",
    );
    click(&ctx, &mut app, 20, "检查并导入路径");
    assert_eq!(app.replay_debugger.saved_paths.len(), 2);
    assert_eq!(app.replay_debugger.selected_path, Some(1));
    assert!(app.replay_debugger.export_json.is_empty());
    assert_eq!(unchanged(&app), before);
    app.replay_debugger.export_replay_path();
    assert_eq!(app.replay_debugger.export_json, json);
}

#[test]
fn exchange_ui_duplicate_import_failure_preserves_live_story_selection_and_export() {
    let (ctx, mut app) = replay_app(
        "event start\n  choice \"继续 {rnd(1, 99)}\"\n    -> END\n",
        20,
    );
    click(&ctx, &mut app, 20, "● 保存当前路径");
    app.replay_debugger.export_replay_path();
    let before = unchanged(&app);
    let json = app.replay_debugger.export_json.clone();
    app.replay_debugger.import_json = format!("{{\"schema_version\":1,{}", &json[1..]);
    click(
        &ctx,
        &mut app,
        20,
        "导入路径 JSON（最多 4 MiB / 20,000 步）",
    );
    click(&ctx, &mut app, 20, "检查并导入路径");
    assert_eq!(app.replay_debugger.saved_paths.len(), 1);
    assert_eq!(app.replay_debugger.selected_path, Some(0));
    assert_eq!(app.replay_debugger.export_json, json);
    assert!(app
        .replay_debugger
        .notice
        .as_deref()
        .unwrap()
        .contains("invalid_json"));
    assert_eq!(unchanged(&app), before);
}

#[test]
fn exchange_legacy_runtime_is_readable_and_rejected_before_replay_changes_results() {
    let (ctx, mut app) = replay_app("event start\n  choice \"继续\"\n    -> END\n", 20);
    click(&ctx, &mut app, 20, "● 保存当前路径");
    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    wait_for_replay(&ctx, &mut app);
    let result = serde_json::to_value(app.replay_debugger.result.as_ref().unwrap()).unwrap();
    let result_name = app.replay_debugger.result_path_name.clone();
    let before = unchanged(&app);
    let mut trace = before.1.clone();
    trace.runtime_version = "0.0.0-legacy".into();
    app.replay_debugger.import_json = encode_replay_trace(&trace).unwrap();
    app.replay_debugger.import_replay_path();
    assert_eq!(app.replay_debugger.saved_paths[1].trace, trace);
    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    assert!(app.replay_debugger.job.is_none());
    assert!(app
        .replay_debugger
        .notice
        .as_deref()
        .unwrap()
        .contains("只读查看"));
    assert!(app
        .replay_debugger
        .notice
        .as_deref()
        .unwrap()
        .contains("runtime_version"));
    assert_eq!(
        serde_json::to_value(app.replay_debugger.result.as_ref().unwrap()).unwrap(),
        result
    );
    assert_eq!(app.replay_debugger.result_path_name, result_name);
    assert_eq!(unchanged(&app), before);
}

#[test]
fn owned_play_restarts_keep_seeded_semantics_and_old_snapshot_until_explicit_restart() {
    let source = concat!(
        "let score = 0\nfragment inner()\n  choice once \"继续 {rnd(1, 99)}\"\n",
        "    set score = score + 1\n    return\nevent start\n  call inner()\n  -> END\n"
    );
    let (ctx, mut app) = app();
    let manifest_path = app.project.root.join(".world/project.json");
    let mut manifest: serde_json::Value = serde_json::from_slice(
        app.project
            .authoring_document(&manifest_path)
            .unwrap()
            .bytes(),
    )
    .unwrap();
    // 片段属于显式新语言语法；共享 UI fixture 默认仍是 1.10，不隐式升级它。
    manifest["language_version"] = "1.13".into();
    app.project
        .set_authoring_document(&manifest_path, serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    app.project
        .set_text(&app.active_file.clone(), source.into())
        .unwrap();
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.snapshot.as_ref().unwrap().result.diagnostics
    );
    app.tab = Tab::Play;
    click(&ctx, &mut app, 20, "▶ 开始试玩");
    // 松键帧创建会话；下一正常 UI 帧才推进到选择组，与 borrowed 的一次继续对齐。
    let _ = frame(&ctx, &mut app, Vec::new(), 20);
    assert!(app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .is_paused());
    let compiled = &app.snapshot.as_ref().unwrap().result;
    let (program, analysis) = (compiled.program.clone(), compiled.analysis.clone());
    let mut borrowed = Story::new_with_seed(&program, &analysis, app.replay_debugger.seed).unwrap();
    borrowed.continue_story().unwrap();
    let before = unchanged(&app);
    assert_eq!(before.0, borrowed.save().unwrap());
    assert_eq!(before.1, borrowed.replay_trace());
    for _ in 0..40 {
        app.start_play();
        let _ = frame(&ctx, &mut app, Vec::new(), 20);
        let owned = app.play.as_ref().unwrap().story.as_ref().unwrap();
        assert_eq!(owned.save().unwrap(), before.0);
        assert_eq!(owned.replay_trace(), before.1);
    }
    app.project
        .set_text(
            &app.active_file.clone(),
            "event start\n  新稿\n  -> END\n".into(),
        )
        .unwrap();
    app.recompile();
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .save()
            .unwrap(),
        before.0
    );
    let owned = app.play.as_mut().unwrap().story.as_mut().unwrap();
    owned.choose(0).unwrap();
    borrowed.choose(0).unwrap();
    owned.continue_story().unwrap();
    borrowed.continue_story().unwrap();
    assert_eq!(owned.save().unwrap(), borrowed.save().unwrap());
    assert_eq!(owned.replay_trace(), borrowed.replay_trace());
    app.start_play();
    let _ = frame(&ctx, &mut app, Vec::new(), 20);
    assert_eq!(app.play.as_ref().unwrap().transcript, "新稿");
    assert!(app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .is_ended());
}
