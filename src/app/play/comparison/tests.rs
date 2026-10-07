//! 真实 runtime DTO 与作者闭环回归；离屏测试不冒充原生操作。
use super::*;
use crate::app::{SavedReplayPath, Tab};
use worldline_core::{project::Project, TargetRef};
use worldline_runtime::{ReplayTrace, Story};

const SOURCE: &str = "entity lens kind artifact as \"信号透镜\"\ntag returned as \"归还\"\ntag sold as \"售出\"\nstate lens_fate on entity lens with [] as \"透镜去向\"\nlet coins = 0\nevent start\n  choice \"归还透镜\"\n    become lens_fate with returned\n    -> finish\n  choice \"出售透镜\"\n    become lens_fate with sold\n    set coins = 10\n    -> finish\nevent finish\n  结果已记录。\n  -> END\n";

fn setup() -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = std::env::temp_dir().join(format!(
        "route-comparison-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.project = Project::new(&root);
    app.active_file = app.project.entry.clone();
    app.project
        .documents
        .retain(|path, _| path == &app.active_file);
    app.project
        .set_text(&app.active_file.clone(), SOURCE.into())
        .unwrap();
    app.project
        .create_authoring_document(
            &root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.12","required_features":[]}"#.to_vec(),
        )
        .unwrap();
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.snapshot.as_ref().unwrap().result.diagnostics
    );
    let paths = [record(&app, 0, 42), record(&app, 1, 42)];
    app.replay_debugger.saved_paths = paths
        .into_iter()
        .enumerate()
        .map(|(i, trace)| SavedReplayPath {
            name: format!("路径 {}", i + 1),
            trace,
        })
        .collect();
    app.comparison.a = Some(0);
    app.comparison.b = Some(1);
    app.comparison.active = true;
    app.tab = Tab::Play;
    app.start_play();
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .continue_story()
        .unwrap();
    (ctx, app)
}
fn record(app: &WorldeditApp, choice: usize, seed: u64) -> ReplayTrace {
    let snapshot = &app.snapshot.as_ref().unwrap().result;
    let mut story = Story::new_with_seed(&snapshot.program, &snapshot.analysis, seed).unwrap();
    story.continue_story().unwrap();
    story.choose(choice).unwrap();
    story.continue_story().unwrap();
    story.replay_trace()
}
fn compare(ctx: &egui::Context, app: &mut WorldeditApp) {
    app.request_comparison(ctx);
    assert!(app.play_confirmation.is_none());
    let start = std::time::Instant::now();
    while app.comparison.job.is_some() {
        assert!(
            start.elapsed() < std::time::Duration::from_secs(10),
            "comparison did not finish"
        );
        app.poll_comparison(ctx);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        app.comparison.result.is_some(),
        "{:?}",
        app.comparison.notice
    );
}
fn invariant(app: &WorldeditApp) -> (String, bool, String, ReplayTrace) {
    let story = app.play.as_ref().unwrap().story.as_ref().unwrap();
    (
        app.project.content_baseline(),
        app.project.is_dirty(),
        story.save().unwrap(),
        story.replay_trace(),
    )
}
fn action_request(app: &WorldeditApp) -> navigation::ComparisonSourceRequest {
    let result = app.comparison.result.as_ref().unwrap();
    navigation::ComparisonSourceRequest {
        result_id: result.id,
        source: result.result.left.state_actions.records[0]
            .source
            .clone()
            .unwrap(),
    }
}
fn frame(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2) -> egui::FullOutput {
    frame_events(ctx, app, size, Vec::new())
}
fn frame_events(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::TopBottomPanel::top("test-app-toolbar")
                .exact_height(38.0)
                .show(ctx, |_| {});
            egui::TopBottomPanel::bottom("test-app-status")
                .exact_height(24.0)
                .show(ctx, |_| {});
            if !app.compact_reference_navigation(ctx) {
                egui::SidePanel::left("test-app-navigation")
                    .exact_width(212.0)
                    .show(ctx, |_| {});
            }
            app.docked_reading(ctx);
            app.play_tab(ctx);
        },
    )
}
fn labels(shape: &egui::Shape, found: &mut Vec<(String, egui::Rect)>) {
    match shape {
        egui::Shape::Text(text) => found.push((
            text.galley.job.text.clone(),
            text.galley.rect.translate(text.pos.to_vec2()),
        )),
        egui::Shape::Vec(shapes) => {
            for shape in shapes {
                labels(shape, found);
            }
        }
        _ => {}
    }
}

#[test]
fn comparison_uses_real_routes_and_is_read_only_across_frames_swap_and_source_back() {
    let (ctx, mut app) = setup();
    app.reading_panels.pin(TargetRef::new("entity", "lens"));
    let references = app.reading_panels.ids().to_vec();
    let before = invariant(&app);
    compare(&ctx, &mut app);
    let result = &app.comparison.result.as_ref().unwrap().result;
    assert_eq!(
        result.left.coverage.total.visited_nodes,
        result.right.coverage.total.visited_nodes
    );
    assert!(result.alignment.first_difference.is_some());
    assert!(result.state_differences.iter().any(|d| d.id == "lens_fate"));
    assert!(result.variable_differences.iter().any(|d| d.id == "coins"));
    let generation = app.comparison.generation;
    for _ in 0..4 {
        frame(&ctx, &mut app, egui::vec2(1040.0, 660.0));
    }
    app.comparison.swap();
    assert_eq!(app.comparison.generation, generation);
    assert_eq!(app.comparison.a, Some(1));
    assert_eq!(
        app.comparison.result.as_ref().unwrap().name(false),
        "路径 2"
    );
    app.comparison.selected_state = Some("lens_fate".into());
    app.comparison.selected_action = Some((false, 1));
    app.comparison.scroll = 185.0;
    let focus = egui::Id::new("comparison-action-focus");
    ctx.memory_mut(|memory| memory.request_focus(focus));
    let location = app.comparison_location(Some(&ctx));
    let request = action_request(&app);
    app.jump_to_comparison_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Edit);
    let _ = ctx.run(
        egui::RawInput {
            modifiers: egui::Modifiers::ALT,
            events: vec![egui::Event::Key {
                key: egui::Key::ArrowLeft,
                physical_key: Some(egui::Key::ArrowLeft),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::ALT,
            }],
            ..Default::default()
        },
        |ctx| app.author_shortcuts(ctx),
    );
    assert_eq!(app.tab, Tab::Play);
    assert_eq!(
        app.comparison_location(Some(&ctx)).as_ref().map(|l| (
            l.a,
            l.b,
            l.selected_state.clone(),
            l.selected_action,
            l.scroll
        )),
        location.as_ref().map(|l| (
            l.a,
            l.b,
            l.selected_state.clone(),
            l.selected_action,
            l.scroll
        ))
    );
    assert_eq!(app.comparison.restore_focus, Some(focus));
    assert_eq!(app.reading_panels.ids(), references);
    assert_eq!(invariant(&app), before);
}

#[test]
fn old_comment_shift_and_unapplied_draft_sources_are_rejected_without_history() {
    let (ctx, mut app) = setup();
    compare(&ctx, &mut app);
    let request = action_request(&app);
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    buffer.replace_source(format!("{SOURCE}\n# unsubmitted"));
    app.manuscript.restore_writing_buffers(&[buffer]);
    let history = app.personal.history.len();
    let before = invariant(&app);
    app.jump_to_comparison_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Play);
    assert!(app.comparison.notice.as_deref().unwrap().contains("未应用"));
    assert_eq!(app.personal.history.len(), history);
    assert_eq!(invariant(&app), before);
    assert!(app.manuscript.writing_buffers()[0]
        .source()
        .contains("unsubmitted"));

    let (ctx, mut app) = setup();
    compare(&ctx, &mut app);
    let request = action_request(&app);
    let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
    app.project
        .set_text(&app.active_file.clone(), format!("\n\n{SOURCE}"))
        .unwrap();
    app.recompile();
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
    app.jump_to_comparison_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Play);
    assert!(app.comparison.notice.as_deref().unwrap().contains("快照"));
    compare(&ctx, &mut app);
    let current = action_request(&app);
    assert_eq!(current.source.line, request.source.line + 2);
    app.jump_to_comparison_source(&ctx, &current);
    assert_eq!(app.tab, Tab::Edit);
}

#[test]
fn changed_selection_keeps_frozen_result_identity_and_cancel_is_explicit() {
    let (ctx, mut app) = setup();
    compare(&ctx, &mut app);
    app.comparison.b = Some(0);
    let result = app.comparison.result.as_ref().unwrap();
    assert!(!result.selections_match(app.comparison.a, app.comparison.b));
    assert_eq!(result.name(true), "路径 2");
    let id = result.id;
    frame(&ctx, &mut app, egui::vec2(1040.0, 660.0));
    assert_eq!(app.comparison.result.as_ref().unwrap().id, id);
    app.comparison.max_steps = 0;
    compare(&ctx, &mut app);
    assert!(
        view::status_text(&app.comparison.result.as_ref().unwrap().result.left).contains("预算")
    );
}

#[test]
fn both_route_statuses_and_first_difference_are_visible_in_both_themes_at_all_sizes() {
    let (ctx, mut app) = setup();
    compare(&ctx, &mut app);
    for mode in [
        crate::theme::ThemeMode::Dark,
        crate::theme::ThemeMode::Light,
    ] {
        let _theme = crate::theme::configure(&ctx, mode);
        for size in [
            egui::vec2(1280.0, 800.0),
            egui::vec2(1188.0, 848.0),
            egui::vec2(1040.0, 660.0),
        ] {
            frame(&ctx, &mut app, size);
            let output = frame(&ctx, &mut app, size);
            let mut found = Vec::new();
            for shape in &output.shapes {
                labels(&shape.shape, &mut found);
            }
            for expected in [
                "A  路径 1",
                "B  路径 2",
                "首个不同选择",
                "A  归还透镜",
                "B  出售透镜",
            ] {
                let (_, rect) = found
                    .iter()
                    .find(|(text, _)| text == expected)
                    .unwrap_or_else(|| panic!("missing {expected} at {size:?}"));
                assert!(
                    rect.bottom() < size.y && rect.left() >= 0.0 && rect.right() <= size.x,
                    "{expected} clipped at {rect:?}"
                );
            }
            assert_eq!(
                found
                    .iter()
                    .filter(|(text, rect)| text.contains("完整结束并验证通过")
                        && rect.bottom() < size.y)
                    .count(),
                2
            );
        }
    }
}

#[test]
fn comparison_history_is_session_only_and_stale_request_does_not_claim_success() {
    let (ctx, mut app) = setup();
    compare(&ctx, &mut app);
    let mut request = action_request(&app);
    request.result_id += 1;
    let before = app.personal.history.len();
    app.message = None;
    app.jump_to_comparison_source(&ctx, &request);
    assert_eq!(app.personal.history.len(), before);
    assert!(app.message.is_none());
    let location = app.author_location(Some(&ctx));
    assert!(serde_json::to_value(&location)
        .unwrap()
        .get("comparison")
        .is_none());
}

#[test]
fn pending_scope_compile_error_is_reported_in_comparison_and_keeps_draft() {
    let (ctx, mut app) = setup();
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    buffer.replace_source(format!("{SOURCE}\n# kept draft"));
    app.manuscript.restore_writing_buffers(&[buffer]);
    app.request_comparison(&ctx);
    assert!(app.play_confirmation.is_some());
    assert!(app.comparison.job.is_none());
    app.snapshot
        .as_mut()
        .unwrap()
        .result
        .diagnostics
        .push(worldline_core::Diagnostic::error(
            "E_TEST",
            "test.wl",
            worldline_core::Span::new(1, 1, 1),
            "test compile error",
        ));
    let _ = ctx.run(egui::RawInput::default(), |ctx| app.play_scope_dialog(ctx));
    assert!(app.play_confirmation.is_none());
    assert!(app
        .comparison
        .notice
        .as_deref()
        .unwrap()
        .contains("编译错误"));
    assert!(app.manuscript.writing_buffers()[0]
        .source()
        .contains("kept draft"));
}

#[test]
fn partial_and_different_seed_results_have_honest_first_screen_risks() {
    let (ctx, mut app) = setup();
    let snapshot = &app.snapshot.as_ref().unwrap().result;
    let mut partial = Story::new_with_seed(&snapshot.program, &snapshot.analysis, 42).unwrap();
    partial.continue_story().unwrap();
    app.replay_debugger.saved_paths[0].trace = partial.replay_trace();
    compare(&ctx, &mut app);
    let result = &app.comparison.result.as_ref().unwrap().result;
    assert!(!result.left.complete);
    assert!(view::status_text(&result.left).contains("非完整结局"));
    frame(&ctx, &mut app, egui::vec2(1040.0, 660.0));
    let output = frame(&ctx, &mut app, egui::vec2(1040.0, 660.0));
    let mut found = Vec::new();
    for shape in &output.shapes {
        labels(&shape.shape, &mut found);
    }
    assert!(found
        .iter()
        .any(|(text, rect)| text.contains("非完整结局") && rect.bottom() < 660.0));

    app.replay_debugger.saved_paths[0].trace = record(&app, 0, 7);
    compare(&ctx, &mut app);
    assert!(
        !app.comparison
            .result
            .as_ref()
            .unwrap()
            .result
            .alignment
            .comparable
    );
    let reason = app
        .comparison
        .result
        .as_ref()
        .unwrap()
        .result
        .alignment
        .reason
        .clone()
        .unwrap();
    frame(&ctx, &mut app, egui::vec2(1040.0, 660.0));
    let output = frame(&ctx, &mut app, egui::vec2(1040.0, 660.0));
    let mut found = Vec::new();
    for shape in &output.shapes {
        labels(&shape.shape, &mut found);
    }
    assert!(found
        .iter()
        .any(|(text, rect)| text == &reason && rect.bottom() < 660.0));
}

#[test]
fn workspace_reset_cancels_comparison_and_forgets_session_results() {
    let (ctx, mut app) = setup();
    app.request_comparison(&ctx);
    let token = app.comparison.job.as_ref().unwrap().cancellation.clone();
    app.reset_views();
    assert!(token.is_cancelled());
    assert!(app.comparison.job.is_none());
    assert!(app.comparison.result.is_none());
}

#[test]
fn long_route_names_keep_status_and_relative_source_on_first_screen() {
    let (ctx, mut app) = setup();
    let long = "这是一条带有很长中文标题以及long_identifier_的真实作者路线".repeat(12);
    let source = SOURCE.replace("归还透镜", &format!("归还透镜 {long}"));
    app.project
        .set_text(&app.active_file.clone(), source)
        .unwrap();
    app.recompile();
    app.replay_debugger.saved_paths[0].trace = record(&app, 0, 42);
    app.replay_debugger.saved_paths[1].trace = record(&app, 1, 42);
    app.replay_debugger.saved_paths[0].name = format!("归还 {long}");
    app.replay_debugger.saved_paths[1].name = format!("出售 {long}");
    compare(&ctx, &mut app);
    let first = app
        .comparison
        .result
        .as_ref()
        .unwrap()
        .result
        .alignment
        .first_difference
        .as_ref()
        .unwrap();
    let source = first.left.source.as_ref().unwrap();
    let expected = format!(
        "{} · 第 {} 行",
        crate::theme::relative_source(&app.project.root, std::path::Path::new(&source.file)),
        source.line
    );
    assert!(!expected.starts_with("/tmp/"));
    for mode in [
        crate::theme::ThemeMode::Dark,
        crate::theme::ThemeMode::Light,
    ] {
        let _theme = crate::theme::configure(&ctx, mode);
        for size in [
            egui::vec2(1280.0, 800.0),
            egui::vec2(1188.0, 848.0),
            egui::vec2(1040.0, 660.0),
        ] {
            frame(&ctx, &mut app, size);
            let output = frame(&ctx, &mut app, size);
            let mut found = Vec::new();
            for shape in &output.shapes {
                labels(&shape.shape, &mut found);
            }
            for caption in ["首个不同选择", expected.as_str()] {
                let (_, rect) = found
                    .iter()
                    .find(|(text, _)| text == caption)
                    .unwrap_or_else(|| panic!("missing {caption}"));
                assert!(
                    rect.bottom() < size.y && rect.right() <= size.x,
                    "long label displaced {caption}: {rect:?}"
                );
            }
            assert_eq!(
                found
                    .iter()
                    .filter(|(text, rect)| text.contains("完整结束并验证通过")
                        && rect.bottom() < size.y)
                    .count(),
                2
            );
        }
    }
}

#[test]
fn stale_comparison_does_not_borrow_renamed_catalog_displays() {
    let (ctx, mut app) = setup();
    compare(&ctx, &mut app);
    frame(&ctx, &mut app, egui::vec2(1040.0, 660.0));
    let output = frame(&ctx, &mut app, egui::vec2(1040.0, 660.0));
    let mut found = Vec::new();
    for shape in &output.shapes {
        labels(&shape.shape, &mut found);
    }
    assert!(found.iter().any(|(text, _)| text == "透镜去向 · 信号透镜"));
    app.project
        .set_text(
            &app.active_file.clone(),
            SOURCE.replace("信号透镜", "后来改名的对象"),
        )
        .unwrap();
    app.recompile();
    let output = frame(&ctx, &mut app, egui::vec2(1040.0, 660.0));
    let mut found = Vec::new();
    for shape in &output.shapes {
        labels(&shape.shape, &mut found);
    }
    assert!(!found
        .iter()
        .any(|(text, _)| text.contains("后来改名的对象")));
    assert!(found.iter().any(|(text, _)| text == "状态 · lens_fate"));
    assert!(found
        .iter()
        .any(|(text, rect)| text.contains("结果已过期") && rect.bottom() < 660.0));
}

#[path = "interaction_tests.rs"]
mod interaction;

#[path = "keyboard_tests.rs"]
mod keyboard;

#[path = "return_focus_tests.rs"]
mod return_focus;

#[path = "saltbell_focus_tests.rs"]
mod saltbell_focus;

#[path = "ime_focus_tests.rs"]
mod ime_focus;

#[path = "variable_write_tests.rs"]
mod variable_writes;
