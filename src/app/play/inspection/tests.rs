use super::*;
use crate::app::Tab;
use worldline_core::project::Project;
fn setup() -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = std::env::temp_dir().join(format!(
        "state-inspection-{}-{}",
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
    app.project.set_text(&app.active_file.clone(),"let count = 0\nevent start\n  choice \"继续\"\n    set count = 2\n    choice \"结束\"\n      -> END\n".into()).unwrap();
    app.recompile();
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
fn request(app: &mut WorldeditApp) -> SourceRequest {
    let story = app.play.as_ref().unwrap().story.as_ref().unwrap();
    app.replay_debugger.inspection.refresh(story);
    let page = app.replay_debugger.inspection.page.as_ref().unwrap();
    let item = &page.items[0];
    SourceRequest {
        stamp: page.stamp,
        key: item.key.clone(),
        source: item.source.clone().unwrap(),
    }
}
fn unchanged(app: &WorldeditApp) -> (String, ReplayTrace, String) {
    let s = app.play.as_ref().unwrap().story.as_ref().unwrap();
    (
        s.save().unwrap(),
        s.replay_trace(),
        app.project.content_baseline(),
    )
}
#[test]
fn inspection_query_and_source_roundtrip_keep_live_story_and_filters() {
    let (ctx, mut app) = setup();
    app.replay_debugger.inspection.query.text = "count".into();
    app.replay_debugger.inspection.open = true;
    let request = request(&mut app);
    let before = unchanged(&app);
    app.jump_to_inspection_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Edit);
    assert!(app.message.as_deref().unwrap().contains("声明头"));
    app.author_back(&ctx);
    assert_eq!(app.tab, Tab::Play);
    assert_eq!(app.replay_debugger.inspection.query.text, "count");
    assert!(app.replay_debugger.inspection.open);
    assert_eq!(unchanged(&app), before);
}
#[test]
fn inspection_navigation_rejects_advanced_restarted_and_forged_bindings() {
    let (ctx, mut app) = setup();
    let mut old = request(&mut app);
    let before = unchanged(&app);
    old.source.id = "other".into();
    app.jump_to_inspection_source(&ctx, &old);
    assert!(app.replay_debugger.inspection.error.is_some());
    assert_eq!(unchanged(&app), before);
    let old = request(&mut app);
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .choose(0)
        .unwrap();
    let before = unchanged(&app);
    app.jump_to_inspection_source(&ctx, &old);
    assert!(app.replay_debugger.inspection.error.is_some());
    assert_eq!(unchanged(&app), before);
    app.start_play();
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .continue_story()
        .unwrap();
    app.jump_to_inspection_source(&ctx, &old);
    assert!(app.replay_debugger.inspection.error.is_some());
    assert_eq!(app.tab, Tab::Play);
}
#[test]
fn inspection_navigation_rejects_unapplied_draft_without_losing_input() {
    let (ctx, mut app) = setup();
    let source = request(&mut app);
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    buffer.replace_source("event unfinished\n  draft".into());
    let draft = buffer.source().to_owned();
    app.manuscript.restore_writing_buffers(&[buffer]);
    let before = unchanged(&app);
    app.jump_to_inspection_source(&ctx, &source);
    assert!(app.replay_debugger.inspection.error.is_some());
    assert_eq!(unchanged(&app), before);
    assert!(app
        .manuscript
        .writing_buffers()
        .iter()
        .any(|b| b.source() == draft));
}
#[test]
fn inspection_window_query_is_read_only_at_wide_and_narrow_sizes() {
    let (ctx, mut app) = setup();
    app.replay_debugger.inspection.open = true;
    app.replay_debugger.inspection.query.text = "count".into();
    let before = unchanged(&app);
    for (width, height) in [(1280.0, 800.0), (800.0, 600.0), (400.0, 300.0)] {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, height),
                )),
                ..Default::default()
            },
            |ctx| app.render_state_inspection(ctx),
        );
        assert_eq!(
            app.replay_debugger
                .inspection
                .page
                .as_ref()
                .unwrap()
                .total_matches,
            1
        );
        assert_eq!(unchanged(&app), before);
    }
}

#[path = "layout_tests.rs"]
mod layout;
