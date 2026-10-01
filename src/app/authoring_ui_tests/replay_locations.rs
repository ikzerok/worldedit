//! 真实调试按钮走共用 runtime；离屏回归不替代原生验收。
use super::*;
use std::path::{Path, PathBuf};
use worldline_runtime::ReplayStatus;

const FRAGMENTS: &str = "fragment outer(file: str)\n  call gate(file)\n  return\nfragment gate(file: str)\n  local line: num = 7\n  choice \"继续\"\n    return\n";
const ENTRY: &str =
    "include \"shared/gate.wl\"\nevent start\n  call outer(\"地图\")\n  结束。\n  -> END\n";

#[test]
fn replay_buttons_accept_moved_fragment_locations_and_keep_current_source_navigation() {
    let (ctx, mut app) = app();
    app.project
        .set_authoring_document(
            &app.project.root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.11","required_features":[]}"#.to_vec(),
        )
        .unwrap();
    let original_file = app.project.add_file(Path::new("shared/gate.wl")).unwrap();
    let entry = app.project.entry.clone();
    app.project.set_text(&entry, ENTRY.into()).unwrap();
    app.project
        .set_text(&original_file, FRAGMENTS.into())
        .unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    app.tab = Tab::Play;
    click(&ctx, &mut app, 20, "▶ 开始试玩");
    click(&ctx, &mut app, 20, "● 保存当前路径");
    click(&ctx, &mut app, 20, "选择：继续");
    click(&ctx, &mut app, 20, "● 保存当前路径");
    assert_eq!(app.replay_debugger.saved_paths.len(), 2);
    let traces = app
        .replay_debugger
        .saved_paths
        .iter()
        .map(|path| path.trace.clone())
        .collect::<Vec<_>>();
    assert!(!traces[0].complete);
    assert!(traces[1].complete);
    assert_eq!(
        traces[0].initial_observation.as_ref().unwrap().state["calls"][1]["line"],
        6
    );

    let moved_file = app
        .project
        .add_file(Path::new("parts/checkpoint.wl"))
        .unwrap();
    app.project
        .set_text(
            &moved_file,
            format!("\n// 仅调整作者文件和定位\n{FRAGMENTS}"),
        )
        .unwrap();
    app.project.delete_document(&original_file).unwrap();
    app.project
        .set_text(
            &entry,
            ENTRY.replace("shared/gate.wl", "parts/checkpoint.wl"),
        )
        .unwrap();
    app.recompile();
    let latest = app.snapshot.as_ref().unwrap();
    assert!(
        !latest.result.has_errors(),
        "{:?}",
        latest.result.diagnostics
    );
    assert_eq!(latest.result.analysis.fingerprint, traces[0].fingerprint);
    let baseline = app.project.content_baseline();
    let history_len = app.history.len();
    let live_save = app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .save()
        .unwrap();

    for (index, trace) in traces.iter().enumerate() {
        app.tab = Tab::Play;
        app.replay_debugger.selected_path = Some(index);
        click(&ctx, &mut app, 20, "▶ 重放所选路径");
        wait_for_replay(&ctx, &mut app);
        assert!(app.replay_debugger.job.is_none());
        assert!(
            app.replay_debugger.notice.is_none(),
            "{:?}",
            app.replay_debugger.notice
        );
        let result = app.replay_debugger.result.as_ref().unwrap();
        assert_eq!(
            result.status,
            ReplayStatus::Replayed {
                ended: trace.complete,
                complete: trace.complete
            }
        );
        assert_eq!(app.replay_debugger.result_version, Some(app.version));
        if !trace.complete {
            let calls = result.current_state["calls"].as_array().unwrap();
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0]["file"], moved_file.to_string_lossy().as_ref());
            assert_eq!(calls[1]["file"], moved_file.to_string_lossy().as_ref());
            assert_eq!(calls[1]["line"], 8);
            assert_eq!(calls[1]["locals"]["file"]["Str"], "地图");
            assert_eq!(calls[1]["locals"]["line"]["Num"], 7.0);
            let file = calls[1]["file"].as_str().unwrap().to_owned();
            let line = calls[1]["line"].as_u64().unwrap() as u32;
            // 验证当前 runtime 定位可由现有源码导航消费；不是失败跳转按钮测试。
            app.jump_to_file(&file, line, 1);
            assert_eq!(app.tab, Tab::Edit);
            assert_eq!(app.active_file, PathBuf::from(file));
            assert_eq!(app.jump, Some((8, 1)));
            assert_eq!(
                app.project
                    .document(&app.active_file)
                    .unwrap()
                    .lines()
                    .nth(7),
                Some("  choice \"继续\"")
            );
        }
        assert_eq!(app.replay_debugger.saved_paths[index].trace, *trace);
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(app.history.len(), history_len);
        assert_eq!(
            app.play
                .as_ref()
                .unwrap()
                .story
                .as_ref()
                .unwrap()
                .save()
                .unwrap(),
            live_save
        );
    }
}

fn divergent_fragment_app(later_step: bool) -> (egui::Context, WorldeditApp, PathBuf) {
    let (ctx, mut app) = app();
    app.project
        .set_authoring_document(
            &app.project.root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.11","required_features":[]}"#.to_vec(),
        )
        .unwrap();
    let fragments = FRAGMENTS.replace(
        "  choice \"继续\"\n    return",
        "  choice \"继续\"\n    第一步。\n  汇合。\n  choice \"离开\"\n    return",
    );
    let old_file = app.project.add_file(Path::new("shared/gate.wl")).unwrap();
    let entry = app.project.entry.clone();
    app.project.set_text(&entry, ENTRY.into()).unwrap();
    app.project.set_text(&old_file, fragments.clone()).unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    app.tab = Tab::Play;
    click(&ctx, &mut app, 20, "▶ 开始试玩");
    click(&ctx, &mut app, 20, "选择：继续");
    click(&ctx, &mut app, 20, "选择：离开");
    click(&ctx, &mut app, 20, "● 保存当前路径");
    assert_eq!(app.replay_debugger.saved_paths[0].trace.steps.len(), 2);

    let moved_file = app
        .project
        .add_file(Path::new("parts/checkpoint.wl"))
        .unwrap();
    let changed = if later_step {
        fragments.replace("\"离开\"", "\"新版离开\"")
    } else {
        fragments.replace("= 7", "= 8")
    };
    app.project
        .set_text(&moved_file, format!("\n// 位置已变化\n{changed}"))
        .unwrap();
    app.project.delete_document(&old_file).unwrap();
    app.project
        .set_text(
            &entry,
            ENTRY.replace("shared/gate.wl", "parts/checkpoint.wl"),
        )
        .unwrap();
    app.recompile();
    assert!(!app.snapshot.as_ref().unwrap().result.has_errors());
    click(&ctx, &mut app, 20, "▶ 重放所选路径");
    wait_for_replay(&ctx, &mut app);
    assert!(app.replay_debugger.job.is_none());
    assert!(
        app.replay_debugger.notice.is_none(),
        "{:?}",
        app.replay_debugger.notice
    );
    (ctx, app, moved_file)
}

#[test]
fn actual_failure_button_uses_moved_fragment_file_and_current_choice_not_recorded_choice() {
    for later_step in [false, true] {
        let (ctx, mut app, moved_file) = divergent_fragment_app(later_step);
        let raw = app.replay_debugger.saved_paths[0].trace.clone();
        let baseline = app.project.content_baseline();
        let result = app.replay_debugger.result.as_ref().unwrap();
        let ReplayStatus::Diverged {
            step_index,
            actual_choices,
            expected_choice,
            ..
        } = &result.status
        else {
            panic!("应在真实改稿处停止：{:?}", result.status);
        };
        assert_eq!(*step_index, usize::from(later_step));
        let actual_line = actual_choices[0].line;
        assert_eq!(actual_line, if later_step { 11 } else { 8 });
        if later_step {
            let expected = expected_choice.as_ref().unwrap();
            assert_eq!(expected.label, "继续");
            assert_eq!(expected.line, 6);
            assert_eq!(actual_choices[0].label, "新版离开");
        }
        click(&ctx, &mut app, 20, "跳转到失败位置");
        assert_eq!(app.tab, Tab::Edit);
        assert_eq!(app.active_file, moved_file);
        assert_eq!(app.jump, Some((actual_line, 1)));
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(app.replay_debugger.saved_paths[0].trace, raw);
    }
}

#[test]
fn outdated_replay_result_cannot_jump_into_a_newer_source_revision() {
    let (ctx, mut app, moved_file) = divergent_fragment_app(true);
    let original_file = app.active_file.clone();
    let source = app.project.document(&moved_file).unwrap().to_owned();
    app.project
        .set_text(&moved_file, format!("\n{source}"))
        .unwrap();
    app.recompile();
    assert_ne!(app.replay_debugger.result_version, Some(app.version));
    let baseline = app.project.content_baseline();
    let before_jump = app.jump;
    click(&ctx, &mut app, 20, "跳转到失败位置");
    assert_eq!(app.tab, Tab::Play);
    assert_eq!(app.active_file, original_file);
    assert_eq!(app.jump, before_jump);
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn invalid_current_fragment_files_are_rejected_without_open_file_fallback() {
    let (ctx, mut app, moved_file) = divergent_fragment_app(true);
    let result = app.replay_debugger.result.as_ref().unwrap().clone();
    let original_file = app.active_file.clone();
    let outside = app.project.root.parent().unwrap().join("outside.wl");
    let missing = app.project.root.join("missing.wl");
    let deleted = app.project.root.join("shared/gate.wl");
    for file in [outside, missing, deleted] {
        let mut invalid = result.clone();
        invalid.current_state["calls"][1]["file"] = serde_json::json!(file);
        app.replay_debugger.result = Some(invalid);
        app.replay_debugger.notice = None;
        click(&ctx, &mut app, 20, "跳转到失败位置");
        assert_eq!(app.tab, Tab::Play);
        assert_eq!(app.active_file, original_file);
        assert!(app
            .replay_debugger
            .notice
            .as_deref()
            .unwrap()
            .contains("不可用"));
    }
    let mut ambiguous = result;
    let duplicate = ambiguous.current_state["calls"][1].clone();
    ambiguous.current_state["calls"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    app.replay_debugger.result = Some(ambiguous);
    let output = frame(&ctx, &mut app, Vec::new(), 20);
    let mut rendered = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut rendered);
    }
    assert!(rendered.contains("当前停止位置不可定位"), "{rendered}");
    assert!(!rendered.contains("跳转到失败位置"), "{rendered}");
    assert_ne!(app.active_file, moved_file);
}
