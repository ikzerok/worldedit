//! 从真实 runtime 变量写入到选择、可信来源与作者返回的闭环。
use super::*;
use worldline_core::evidence_source::{EvidenceSourceOwner, VariableWriteOperation};
use worldline_runtime::Value;

pub(super) const WRITE_SOURCE: &str = "let coins = 0\nlet message = \"\"\nlet untouched = 7\nevent start\n  choice \"归还透镜\"\n    set coins = 1\n    set coins = 1\n    set coins = 0\n    let reward = 3\n    const seal = true\n    -> finish\n  choice \"出售透镜\"\n    set coins = 10\n    set coins = 0\n    set message = \"星🌙\"\n    -> finish\nevent finish\n  结果已记录。\n  -> END\n";

pub(super) fn ready() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = setup();
    app.project
        .set_text(&app.active_file.clone(), WRITE_SOURCE.into())
        .unwrap();
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.snapshot.as_ref().unwrap().result.diagnostics
    );
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
    app.comparison.selected_variable = Some("coins".into());
    (ctx, app)
}

pub(super) fn request(
    app: &WorldeditApp,
    right: bool,
    index: usize,
) -> navigation::ComparisonSourceRequest {
    let compared = app.comparison.result.as_ref().unwrap();
    let side = if right {
        &compared.result.right
    } else {
        &compared.result.left
    };
    navigation::ComparisonSourceRequest {
        result_id: compared.id,
        source: side.variable_writes.records[index].source.clone().unwrap(),
    }
}

#[test]
fn variable_writes_keep_real_order_same_values_and_actual_side_when_swapped() {
    let (ctx, mut app) = ready();
    let before = invariant(&app);
    let compared = app.comparison.result.as_ref().unwrap();
    assert!(!compared
        .result
        .variable_differences
        .iter()
        .any(|value| value.id == "coins"));
    let records = &compared.result.left.variable_writes.records;
    assert_eq!(
        records
            .iter()
            .map(|record| record.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5]
    );
    assert_eq!(records[1].before.as_ref(), Some(&records[1].after));
    assert_eq!(records[3].before, None);
    assert_eq!(records[3].operation, VariableWriteOperation::Let);
    assert_eq!(records[4].operation, VariableWriteOperation::Const);
    app.comparison.selected_write = Some((false, 2));
    app.comparison.swap();
    assert_eq!(
        app.comparison
            .result
            .as_ref()
            .unwrap()
            .side(true)
            .variable_writes
            .records[1]
            .after,
        Value::Num(1.0)
    );
    assert_eq!(app.comparison.selected_write, Some((false, 2)));
    frame(&ctx, &mut app, egui::vec2(1040.0, 660.0));
    assert_eq!(invariant(&app), before);
}

#[test]
fn variable_write_sources_reject_forgery_mismatched_owner_and_old_result() {
    for alteration in 0..6 {
        let (ctx, mut app) = ready();
        let mut request = request(&app, false, 0);
        match alteration {
            0 => request.result_id += 1,
            1 => request.source.line += 100,
            2 => request.source.file = app.project.root.join("other.wl").display().to_string(),
            3 => {
                if let EvidenceSourceOwner::VariableWrite { variable, .. } =
                    &mut request.source.owner
                {
                    *variable = "message".into();
                }
            }
            4 => {
                app.comparison
                    .result
                    .as_mut()
                    .unwrap()
                    .result
                    .left
                    .variable_writes
                    .records[0]
                    .variable = "message".into();
            }
            _ => {
                app.comparison
                    .result
                    .as_mut()
                    .unwrap()
                    .result
                    .left
                    .variable_writes
                    .records[0]
                    .node = Some("finish".into());
            }
        }
        let before = invariant(&app);
        let history = app.personal.history.len();
        app.jump_to_comparison_source(&ctx, &request);
        assert_eq!(app.tab, Tab::Play, "source alteration {alteration}");
        assert!(app
            .comparison
            .notice
            .as_deref()
            .unwrap()
            .contains("不属于当前比较"));
        assert_eq!(app.personal.history.len(), history);
        assert_eq!(invariant(&app), before);
    }
}

#[test]
fn variable_write_navigation_preserves_unapplied_draft_ime_and_wrong_compile_guards() {
    for blocker in 0..5 {
        let (ctx, mut app) = ready();
        let request = request(&app, false, 0);
        match blocker {
            0 => {
                let mut buffer = app
                    .project
                    .open_source_writing_buffer(&app.active_file)
                    .unwrap();
                buffer.replace_source(format!("{WRITE_SOURCE}\n# never applied"));
                app.manuscript.restore_writing_buffers(&[buffer]);
            }
            1 => app.ime_composing = true,
            2 => app.command_palette.ime_frame = true,
            3 => app.snapshot.as_mut().unwrap().result.diagnostics.push(
                worldline_core::Diagnostic::error(
                    "E_TEST",
                    "world.wl",
                    worldline_core::Span::new(1, 1, 1),
                    "bad draft",
                ),
            ),
            _ => {
                app.project
                    .set_text(&app.active_file.clone(), format!("\n{WRITE_SOURCE}"))
                    .unwrap();
                app.recompile();
            }
        }
        let before = invariant(&app);
        let history = app.personal.history.len();
        app.jump_to_comparison_source(&ctx, &request);
        assert_eq!(app.tab, Tab::Play, "guard {blocker}");
        assert!(app.comparison.notice.is_some());
        assert_eq!(app.personal.history.len(), history);
        assert_eq!(invariant(&app), before);
        if blocker == 0 {
            assert!(app.manuscript.writing_buffers()[0]
                .source()
                .contains("never applied"));
        }
    }
}

#[test]
fn variable_write_external_change_blocks_source_without_recompiling_or_running() {
    let (ctx, mut app) = ready();
    app.project.save().unwrap();
    let request = request(&app, false, 0);
    std::fs::write(&app.active_file, format!("\n{WRITE_SOURCE}")).unwrap();
    let before = invariant(&app);
    let history = app.personal.history.len();
    app.jump_to_comparison_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Play);
    assert!(app.comparison.notice.as_deref().unwrap().contains("刷新"));
    assert_eq!(app.personal.history.len(), history);
    assert_eq!(invariant(&app), before);
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn variable_write_return_restores_selection_but_never_borrows_new_result_sequence() {
    let (ctx, mut app) = ready();
    app.comparison.selected_write = Some((false, 2));
    app.comparison.scroll = 280.0;
    let id = egui::Id::new("variable-return-focus");
    ctx.memory_mut(|memory| memory.request_focus(id));
    let location = app.comparison_location(Some(&ctx)).unwrap();
    let request = request(&app, false, 1);
    app.jump_to_comparison_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Edit);
    app.author_back(&ctx);
    assert_eq!(app.tab, Tab::Play);
    assert_eq!(app.comparison.selected_variable, location.selected_variable);
    assert_eq!(app.comparison.selected_write, location.selected_write);
    assert_eq!(app.comparison.scroll, location.scroll);
    assert_eq!(app.comparison.restore_focus, Some(id));
    compare(&ctx, &mut app);
    let new_id = app.comparison.result.as_ref().unwrap().id;
    assert_ne!(new_id, request.result_id);
    assert_eq!(app.comparison.selected_write, None);
    assert_eq!(app.comparison.restore_focus, None);
    app.restore_comparison_location(&ctx, Some(location));
    assert_eq!(app.comparison.selected_write, None);
    assert!(app
        .comparison
        .notice
        .as_deref()
        .unwrap()
        .contains("结果已更新"));
}

#[path = "variable_keyboard_tests.rs"]
mod keyboard;

#[test]
fn variable_write_guard_rejects_other_file_manifest_and_new_inventory_changes() {
    for change in 0..4 {
        let (ctx, mut app) = ready();
        let other = app
            .project
            .add_file(std::path::Path::new("other.wl"))
            .unwrap();
        app.project
            .set_text(&other, "event unused\n  -> END\n".into())
            .unwrap();
        app.recompile();
        app.replay_debugger.saved_paths[0].trace = record(&app, 0, 42);
        app.replay_debugger.saved_paths[1].trace = record(&app, 1, 42);
        compare(&ctx, &mut app);
        app.project.save().unwrap();
        let request = request(&app, false, 0);
        match change {
            0 => std::fs::write(&other, "event unused\n  外部稿。\n  -> END\n").unwrap(),
            1 => std::fs::write(
                app.project.root.join(".world/project.json"),
                br#"{"schema_version":1,"language_version":"99.0","required_features":[]}"#,
            )
            .unwrap(),
            2 => std::fs::write(
                app.project.root.join("new.wl"),
                "event new_file\n  -> END\n",
            )
            .unwrap(),
            _ => std::fs::remove_file(&other).unwrap(),
        }
        let before = invariant(&app);
        let history = app.personal.history.len();
        app.jump_to_comparison_source(&ctx, &request);
        assert_eq!(app.tab, Tab::Play, "external change {change}");
        assert!(app.comparison.notice.is_some());
        assert_eq!(app.personal.history.len(), history);
        assert_eq!(invariant(&app), before);
        std::fs::remove_dir_all(&app.project.root).unwrap();
    }
}

#[path = "variable_render_tests.rs"]
mod render;

#[path = "variable_source_tests.rs"]
mod sources;
