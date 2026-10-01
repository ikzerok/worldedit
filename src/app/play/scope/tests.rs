//! 仅保留稿件边界的最小回归；离屏确认不冒充原生 IME 验收。
use super::*;
use crate::app::{SavedReplayPath, Tab};
use worldline_runtime::{Output, ReplayBudget, ReplayCancellation, ReplayStatus};

fn app() -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    ctx.style_mut(|s| s.animation_time = 0.0);
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = std::env::temp_dir().join(format!(
        "play-scope-{}-{}",
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
        .set_text(
            &app.active_file.clone(),
            "event start\n  applied_A\n  -> END\n".into(),
        )
        .unwrap();
    app.recompile();
    app.tab = Tab::Play;
    (ctx, app)
}

fn stage(app: &mut WorldeditApp, text: &str) {
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    buffer.replace_source(text.into());
    // 同一源在不同章节恢复仍为单一 buffer，不按章节计数。
    app.manuscript
        .restore_writing_buffers(&[buffer.clone(), buffer]);
}

fn text(app: &mut WorldeditApp) -> String {
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .continue_story_bounded(ReplayBudget::new(100, 2_000), &ReplayCancellation::new())
        .unwrap()
        .outputs
        .into_iter()
        .filter_map(|output| match output {
            Output::Text { content, .. } => Some(content),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ctx| app.play_scope_dialog(ctx),
    )
}

fn position(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text == label => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|s| position(s, label)),
        _ => None,
    }
}

fn click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for _ in 0..3 {
        frame(ctx, app, vec![]);
    }
    let output = frame(ctx, app, vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|s| position(&s.shape, label))
        .unwrap();
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

#[test]
fn explicit_applied_scope_cancel_return_and_unsaved_apply_keep_author_drafts() {
    let (ctx, mut app) = app();
    app.project.save().unwrap();
    let disk = std::fs::read(&app.active_file).unwrap();
    stage(&mut app, "event start\n  draft_B\n  -> END\n");
    let baseline = app.project.content_baseline();
    app.start_play();
    assert!(app.play.is_none());
    assert_eq!(app.unapplied_play_inputs().len(), 1);
    click(&ctx, &mut app, "取消运行");
    assert!(app.play.is_none());
    assert!(app.play_confirmation.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    app.start_play();
    click(&ctx, &mut app, "返回处理");
    assert_eq!(app.tab, Tab::Manuscript);
    assert!(app.play.is_none());
    assert!(app.play_confirmation.is_none());
    app.start_play();
    click(&ctx, &mut app, "明确运行已应用稿");
    assert_eq!(text(&mut app), "applied_A");
    let scope = &app.play.as_ref().unwrap().scope;
    assert_eq!(scope.excluded_inputs.len(), 1);
    assert!(scope.sources[&app.active_file].contains("applied_A"));
    let buffer = app.manuscript.writing_buffers().remove(0);
    assert!(buffer.source().contains("draft_B"));
    app.project.apply_source_writing_buffer(&buffer).unwrap();
    app.manuscript
        .clear_applied_writing_buffers(&[app.active_file.clone()]);
    app.recompile();
    app.start_play();
    assert!(app.play_confirmation.is_none());
    assert_eq!(text(&mut app), "draft_B");
    assert!(app.project.is_dirty());
    assert_eq!(std::fs::read(&app.active_file).unwrap(), disk);
    let _ = std::fs::remove_dir_all(app.project.root);
}

#[test]
fn unchanged_dirty_list_does_not_reuse_permission_after_content_version_or_baseline_changes() {
    let (ctx, mut app) = app();
    stage(&mut app, "event start\n  draft_B\n  -> END\n");
    app.start_play();
    let shown = app.play_confirmation.take().unwrap();
    stage(&mut app, "event start\n  draft_C\n  -> END\n");
    app.confirm_play_scope(&ctx, shown);
    assert!(app.play.is_none());
    assert!(app.play_confirmation.as_ref().unwrap().refreshed);
    let shown = app.play_confirmation.take().unwrap();
    app.recompile();
    app.confirm_play_scope(&ctx, shown);
    assert!(app.play.is_none());
    let shown = app.play_confirmation.take().unwrap();
    app.project
        .set_text(
            &app.active_file.clone(),
            "event start\n  changed_applied\n  -> END\n".into(),
        )
        .unwrap();
    app.confirm_play_scope(&ctx, shown);
    assert!(app.play.is_none());
    assert!(app.play_confirmation.is_some());
    app.recompile();
    let _ = frame(&ctx, &mut app, vec![]);
    click(&ctx, &mut app, "明确运行已应用稿");
    assert_eq!(text(&mut app), "changed_applied");
    assert!(app.manuscript.writing_buffers()[0]
        .source()
        .contains("draft_C"));
}

#[test]
fn hidden_event_condition_state_and_ime_are_inventory_and_content_guarded() {
    let (ctx, mut app) = app();
    app.select_event("start");
    app.event_editor.as_mut().unwrap().draft.perm = "bad condition (".into();
    app.state_editor = Some((
        None,
        worldline_core::states::StateDraft {
            id: "pending_state".into(),
            display: "状态".into(),
            target: worldline_core::TargetRef::new("world", "world"),
            tags: vec!["new_tag".into()],
        },
    ));
    app.ime_source_draft = Some((
        app.active_file.clone(),
        "unfinished IME".into(),
        "original".into(),
    ));
    app.ime_composing = true;
    app.tab = Tab::Play;
    let inputs = app.unapplied_play_inputs();
    for kind in ["事件正文与分支", "状态", "正在输入的源码 / 输入法"] {
        assert!(inputs.iter().any(|input| input.kind == kind));
    }
    app.start_play();
    let shown = app.play_confirmation.take().unwrap();
    app.event_editor
        .as_mut()
        .unwrap()
        .draft
        .perm
        .push_str("more");
    app.confirm_play_scope(&ctx, shown);
    assert!(app.play.is_none());
    let shown = app.play_confirmation.take().unwrap();
    app.state_editor
        .as_mut()
        .unwrap()
        .1
        .tags
        .push("another_tag".into());
    app.confirm_play_scope(&ctx, shown);
    assert!(app.play.is_none());
    let shown = app.play_confirmation.take().unwrap();
    app.ime_source_draft.as_mut().unwrap().1.push_str("more");
    app.confirm_play_scope(&ctx, shown);
    assert!(app.play.is_none());
    let shown = app.play_confirmation.take().unwrap();
    app.confirm_play_scope(&ctx, shown);
    assert_eq!(text(&mut app), "applied_A");
    assert!(app
        .event_editor
        .as_ref()
        .unwrap()
        .draft
        .perm
        .contains("bad condition ("));
}

#[test]
fn pure_display_drafts_do_not_require_runtime_confirmation_and_clean_event_is_not_dirty() {
    let (_ctx, mut app) = app();
    app.select_event("start");
    app.map_selection = Some("display_map".into());
    app.map_form.annotation = "unapplied display note".into();
    app.network_view_id = "display_layout".into();
    app.review.reason = "unapplied review note".into();
    assert!(!app.unapplied_export_inputs().is_empty());
    assert!(app.unapplied_play_inputs().is_empty());
    app.start_play();
    assert!(app.play_confirmation.is_none());
    assert_eq!(text(&mut app), "applied_A");
}

#[test]
fn replay_scope_cancel_and_confirm_do_not_mutate_live_rng_or_original_trace() {
    let (ctx, mut app) = app();
    app.project
        .set_text(
            &app.active_file.clone(),
            "let roll = 0\nevent start\n  set roll = rnd(1, 100)\n  applied_A\n  -> END\n".into(),
        )
        .unwrap();
    app.recompile();
    app.start_play();
    assert_eq!(text(&mut app), "applied_A");
    let story = app.play.as_ref().unwrap().story.as_ref().unwrap();
    let trace = story.replay_trace();
    let live = story.save().unwrap();
    app.replay_debugger.saved_paths.push(SavedReplayPath {
        name: "路线".into(),
        trace: trace.clone(),
    });
    app.replay_debugger.selected_path = Some(0);
    stage(&mut app, "event start\n  broken_draft\n  -> nonexistent\n");
    app.begin_replay(&ctx);
    assert!(app.replay_debugger.job.is_none());
    click(&ctx, &mut app, "取消运行");
    assert!(app.replay_debugger.result.is_none());
    app.begin_replay(&ctx);
    click(&ctx, &mut app, "明确运行已应用稿");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while app.replay_debugger.job.is_some() && std::time::Instant::now() < deadline {
        app.poll_replay(&ctx);
        std::thread::yield_now();
    }
    assert!(matches!(
        app.replay_debugger.result.as_ref().unwrap().status,
        ReplayStatus::Replayed { .. }
    ));
    assert_eq!(
        app.replay_debugger
            .result_scope
            .as_ref()
            .unwrap()
            .excluded_inputs
            .len(),
        1
    );
    assert_eq!(app.replay_debugger.saved_paths[0].trace, trace);
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .save()
            .unwrap(),
        live
    );
    assert!(app.manuscript.writing_buffers()[0]
        .source()
        .contains("nonexistent"));
}

#[test]
fn changed_manifest_options_invalidate_confirmation_and_cannot_run_uncompiled_options() {
    let (ctx, mut app) = app();
    stage(&mut app, "event start\n  draft_B\n  -> END\n");
    app.start_play();
    let shown = app.play_confirmation.take().unwrap();
    let manifest = app.project.root.join(".world/project.json");
    let bytes = br#"{"schema_version":1,"language_version":"1.10","entry":"world.wl","required_features":[],"maps":{},"graph_views":{}}"#.to_vec();
    if app.project.authoring_document(&manifest).is_ok() {
        app.project
            .set_authoring_document(&manifest, bytes)
            .unwrap();
    } else {
        app.project
            .create_authoring_document(&manifest, bytes)
            .unwrap();
    }
    app.confirm_play_scope(&ctx, shown);
    assert!(app.play.is_none());
    let shown = app.play_confirmation.take().unwrap();
    app.confirm_play_scope(&ctx, shown);
    assert!(app.play.is_none(), "旧编译 options 不得用于新的已应用清单");
    app.recompile();
    app.start_play();
    let shown = app.play_confirmation.take().unwrap();
    app.confirm_play_scope(&ctx, shown);
    assert_eq!(
        app.play.as_ref().unwrap().scope.options,
        app.project.compile_options()
    );
    assert_eq!(text(&mut app), "applied_A");
}

#[test]
fn newly_active_source_without_recompile_cannot_silently_run_previous_source_set() {
    let (_ctx, mut app) = app();
    let original = app.project.document(&app.active_file).unwrap().to_owned();
    let extra = app.project.add_file(Path::new("new-active.wl")).unwrap();
    // add_file 会添加 include；恢复旧文件字节，专门覆盖新增源集合的守卫。
    app.project
        .set_text(&app.active_file.clone(), original)
        .unwrap();
    app.project
        .set_text(
            &extra,
            "event newly_active\n  Added source\n  -> END\n".into(),
        )
        .unwrap();
    assert!(app.project.sources().contains_key(&extra));
    assert!(!app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .sources
        .contains_key(&extra));
    app.start_play();
    assert!(app.play.is_none(), "新活动源码尚未编译，不得运行旧源码集合");
    assert!(app
        .replay_debugger
        .notice
        .as_ref()
        .unwrap()
        .contains("编译快照不一致"));
    app.recompile();
    app.start_play();
    assert!(app
        .play
        .as_ref()
        .unwrap()
        .scope
        .sources
        .contains_key(&extra));
    assert_eq!(text(&mut app), "applied_A");
}
