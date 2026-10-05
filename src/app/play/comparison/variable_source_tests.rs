//! 跨文件片段与全快照来源权限，不把可读 DTO 当成跳转授权。
use super::*;

#[test]
fn variable_write_fragment_source_opens_the_real_file_and_statement_header() {
    let (ctx, mut app) = ready();
    let shared = app
        .project
        .add_file(std::path::Path::new("shared.wl"))
        .unwrap();
    app.project
        .set_text(
            &shared,
            "fragment receipt_steps()\n  set coins = coins + 1\n  set coins = coins\n  return\n"
                .into(),
        )
        .unwrap();
    let source = "include \"shared.wl\"\nlet coins = 0\nevent start\n  choice \"A\"\n    call receipt_steps()\n    -> finish\n  choice \"B\"\n    set coins = 2\n    -> finish\nevent finish\n  -> END\n";
    app.project
        .set_text(&app.active_file.clone(), source.into())
        .unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    app.replay_debugger.saved_paths[0].trace = record(&app, 0, 42);
    app.replay_debugger.saved_paths[1].trace = record(&app, 1, 42);
    app.start_play();
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .continue_story()
        .unwrap();
    compare(&ctx, &mut app);
    let record = &app
        .comparison
        .result
        .as_ref()
        .unwrap()
        .result
        .left
        .variable_writes
        .records[0];
    assert_eq!(record.node.as_deref(), Some("fragment:receipt_steps"));
    assert_eq!(
        record.source.as_ref().unwrap().file,
        shared.display().to_string()
    );
    let request = request(&app, false, 0);
    let hit = app
        .play_source_hit(
            &app.comparison.result.as_ref().unwrap().scope,
            &request.source,
        )
        .unwrap();
    let before = invariant(&app);
    app.jump_to_comparison_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Edit);
    assert_eq!(app.active_file, shared);
    assert_eq!(hit.line, 2);
    assert!(hit.preview.trim_start().starts_with("set coins"));
    assert_eq!(invariant(&app), before);
    app.author_back(&ctx);
    assert_eq!(app.tab, Tab::Play);
}

#[test]
fn variable_write_source_cannot_gain_authorization_through_a_choice_field() {
    let (ctx, mut app) = ready();
    let request = request(&app, false, 0);
    let result = &mut app.comparison.result.as_mut().unwrap().result;
    result.left.variable_writes.records.clear();
    result
        .alignment
        .first_difference
        .as_mut()
        .unwrap()
        .left
        .source = Some(request.source.clone());
    app.jump_to_comparison_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Play);
    assert!(app
        .comparison
        .notice
        .as_deref()
        .unwrap()
        .contains("不属于当前比较"));
}

#[test]
fn variable_write_invalid_current_manifest_keeps_old_evidence_readable_but_not_navigable() {
    for manifest in [
        br#"{"schema_version":1,"language_version":"99.0","required_features":[]}"#.as_slice(),
        br#"{"schema_version":1,"language_version":"1.12","required_features":["unknown.feature"]}"#.as_slice(),
        b"{ invalid manifest".as_slice(),
    ] {
        let (ctx, mut app) = ready();
        let request = request(&app, false, 0);
        app.project.save().unwrap();
        std::fs::write(app.project.root.join(".world/project.json"), manifest).unwrap();
        app.project.refresh().unwrap();
        app.recompile();
        let before = invariant(&app);
        let history = app.personal.history.len();
        app.jump_to_comparison_source(&ctx, &request);
        assert_eq!(app.tab, Tab::Play);
        assert!(app.comparison.result.is_some());
        assert!(app.comparison.notice.is_some());
        assert_eq!(app.personal.history.len(), history);
        assert_eq!(invariant(&app), before);
        std::fs::remove_dir_all(&app.project.root).unwrap();
    }
}

#[test]
fn variable_write_render_guard_stays_in_memory_but_navigation_checks_fresh_disk() {
    let (ctx, mut app) = ready();
    app.project.save().unwrap();
    let request = request(&app, false, 0);
    let transaction = app.project.root.join(".world/.transactions/pending");
    std::fs::create_dir_all(&transaction).unwrap();
    std::fs::write(transaction.join("marker"), "must not recover").unwrap();
    let scope = app.comparison.result.as_ref().unwrap().scope.clone();
    let before = invariant(&app);
    // 未刷新外部状态留到动作时读取；用于每帧按钮状态的守卫只核验内存。
    assert!(app.play_source_guard(&scope).is_ok());
    for _ in 0..3 {
        frame(&ctx, &mut app, egui::vec2(1040.0, 660.0));
    }
    assert!(app.comparison.notice.is_none());
    assert_eq!(
        std::fs::read_to_string(transaction.join("marker")).unwrap(),
        "must not recover"
    );
    app.jump_to_comparison_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Play);
    assert!(app
        .comparison
        .notice
        .as_deref()
        .unwrap()
        .contains("保存事务"));
    assert_eq!(invariant(&app), before);
    assert_eq!(
        std::fs::read_to_string(transaction.join("marker")).unwrap(),
        "must not recover"
    );
    std::fs::remove_dir_all(&app.project.root).unwrap();
}
