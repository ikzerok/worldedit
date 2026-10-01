//! 快照/草稿/来源身份与返回保护；不以离屏测试冒充原生操作验收。
use super::*;
use crate::app::{writing_workspace::Mode, Tab};
use worldline_core::project::Project;
use worldline_runtime::{EvidenceOutcome, Story};

fn setup(source: &str) -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.0);
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = std::env::temp_dir().join(format!(
        "evidence-navigation-{}-{}",
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
        .set_text(&app.active_file.clone(), source.into())
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
    app.tab = Tab::Play;
    app.start_play();
    let _ = app
        .play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .continue_story();
    (ctx, app)
}

fn source_request(app: &WorldeditApp, rule: bool) -> EvidenceNavigationRequest {
    let cached = app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .choice_evidence()
        .unwrap()
        .to_vec();
    let source = sources(&cached)
        .into_iter()
        .find(|source| {
            matches!(
                source.owner,
                worldline_core::evidence_source::EvidenceSourceOwner::Rule { .. }
            ) == rule
        })
        .unwrap();
    EvidenceNavigationRequest { source, cached }
}

fn state(app: &WorldeditApp) -> (String, worldline_runtime::ReplayTrace, String, bool) {
    let story: &Story<'_> = app.play.as_ref().unwrap().story.as_ref().unwrap();
    (
        story.save().unwrap(),
        story.replay_trace(),
        app.project.content_baseline(),
        app.project.is_dirty(),
    )
}

#[test]
fn cached_evidence_source_navigation_is_read_only_and_returns_through_author_history() {
    let (ctx, mut app) = setup("event start\n  choice \"阻断\" if rnd(1, 9) < 0\n    -> END\n  choice \"继续\"\n    -> END\n");
    let request = source_request(&app, false);
    let before = state(&app);
    let history = app.personal.history.len();
    assert!(app
        .evidence_navigation_access()
        .source_reason(Some(&request.source))
        .is_none());
    app.jump_to_evidence_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Edit);
    assert_eq!(app.personal.history.len(), history + 1);
    assert!(app.message.as_deref().unwrap().contains("声明头"));
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
    assert_eq!(state(&app), before);
    assert_eq!(app.personal.history.len(), history);
}

#[test]
fn newer_snapshot_and_unapplied_source_are_rejected_without_losing_drafts() {
    let source = "event start\n  choice \"继续\" if true\n    -> END\n";
    let (ctx, mut app) = setup(source);
    let request = source_request(&app, false);
    let before = state(&app);
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    buffer.replace_source(source.replace("true", "unfinished("));
    let draft = buffer.source().to_owned();
    app.manuscript.restore_writing_buffers(&[buffer]);
    assert!(app
        .evidence_navigation_access()
        .reason
        .as_deref()
        .unwrap()
        .contains("未应用"));
    app.jump_to_evidence_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Play);
    assert_eq!(app.manuscript.writing_buffers()[0].source(), draft);
    assert_eq!(state(&app), before);

    let (ctx, mut app) = setup(source);
    let request = source_request(&app, false);
    app.project
        .set_text(&app.active_file.clone(), format!("\n{source}"))
        .unwrap();
    app.recompile();
    let before = state(&app);
    app.jump_to_evidence_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Play);
    assert!(app
        .replay_debugger
        .notice
        .as_deref()
        .unwrap()
        .contains("快照"));
    assert_eq!(state(&app), before);
}

#[test]
fn rule_error_and_not_evaluated_source_can_be_located_without_retrying_failure() {
    let (ctx, mut app) = setup("rule broken(n: num) -> bool = (n / 0 > 0) and (n > 2)\nevent start\n  choice \"失败\" enable broken(1) disabled \"阻断\"\n    -> END\n");
    let request = source_request(&app, true);
    let evidence = request.cached[0]
        .enable_condition
        .as_ref()
        .unwrap()
        .evidence
        .as_ref()
        .unwrap();
    assert!(evidence
        .nodes
        .iter()
        .any(|node| node.outcome == EvidenceOutcome::NotEvaluated));
    let before = state(&app);
    app.jump_to_evidence_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Edit);
    // Pending selection is applied by the next source frame; exact range comes from core already.
    assert_eq!(app.evidence_source_hit(&request.source).unwrap().line, 1);
    assert_eq!(state(&app), before);
}

#[test]
fn old_group_renamed_or_deleted_source_requests_are_rejected() {
    let source = "event start\n  choice \"继续\" if true\n    -> next\nevent next\n  choice \"结束\"\n    -> END\n";
    let (ctx, mut app) = setup(source);
    let request = source_request(&app, false);
    let story = app.play.as_mut().unwrap().story.as_mut().unwrap();
    story.choose(0).unwrap();
    story.continue_story().unwrap();
    app.jump_to_evidence_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Play);
    assert!(app
        .replay_debugger
        .notice
        .as_deref()
        .unwrap()
        .contains("证据已更新"));

    for delete in [false, true] {
        let (ctx, mut app) = setup(source);
        let mut request = source_request(&app, false);
        if delete {
            app.project
                .delete_document(&app.active_file.clone())
                .unwrap();
        } else {
            request.source.file = app.project.root.join("renamed.wl").display().to_string();
        }
        let before = state(&app);
        app.jump_to_evidence_source(&ctx, &request);
        assert_eq!(app.tab, Tab::Play);
        assert_eq!(state(&app), before);
    }
}

#[test]
fn saved_source_external_move_is_rejected_but_applied_unsaved_source_can_navigate() {
    let source = "event start\n  choice \"继续\" if true\n    -> END\n";
    let (ctx, mut app) = setup(source);
    app.project.save().unwrap();
    let request = source_request(&app, false);
    std::fs::rename(&app.active_file, app.project.root.join("moved.wl")).unwrap();
    let before = state(&app);
    app.jump_to_evidence_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Play);
    assert!(app
        .replay_debugger
        .notice
        .as_deref()
        .unwrap()
        .contains("外部"));
    assert_eq!(state(&app), before);
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn manuscript_structure_mode_and_safe_back_share_the_existing_navigation() {
    let source = "event start\n  choice \"继续\" if true\n    -> END\n";
    let (ctx, mut app) = setup(source);
    let root = app.project.root.clone();
    app.project.set_authoring_document(&root.join(".world/project.json"),
        br#"{"schema_version":1,"language_version":"1.12","required_features":["presentation.manuscripts.v1"],"manuscripts":{"book":".world/manuscripts/book.json"}}"#.to_vec()).unwrap();
    app.project.create_authoring_document(&root.join(".world/manuscripts/book.json"),
        br#"{"schema_version":1,"id":"book","title":"Book","entries":[{"id":"start","kind":"chapter","title":"Start","target_ref":{"kind":"event","id":"start"}}]}"#.to_vec()).unwrap();
    app.recompile();
    app.restore_manuscript_session(crate::app::manuscript::ManuscriptSession {
        manuscript_id: Some("book".into()),
        selected_id: Some("start".into()),
        mode: Mode::Structure,
        ..Default::default()
    });
    let _ = ctx.run(egui::RawInput::default(), |ctx| app.manuscript_tab(ctx));
    app.start_play();
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .continue_story()
        .unwrap();
    let request = source_request(&app, false);
    let before = state(&app);
    app.jump_to_evidence_source(&ctx, &request);
    assert_eq!(app.tab, Tab::Manuscript);
    assert_eq!(app.manuscript_session().mode, Mode::Structure);
    app.author_back(&ctx);
    assert_eq!(app.tab, Tab::Play);
    assert_eq!(state(&app), before);
}

#[test]
fn rejected_author_bridge_and_evidence_request_never_claim_success_or_add_history() {
    let (ctx, mut app) = setup("event start\n  choice \"继续\" if true\n    -> END\n");
    let mut request = source_request(&app, false);
    let mut hit = app.evidence_source_hit(&request.source).unwrap();
    hit.path = app.project.root.join("missing.wl");
    let before = state(&app);
    let history = app.personal.history.len();
    assert!(app.go_author_source_position(&ctx, &hit, true).is_err());
    assert_eq!(app.personal.history.len(), history);
    assert_eq!(app.tab, Tab::Play);
    request.source.file = hit.path.display().to_string();
    app.message = None;
    app.jump_to_evidence_source(&ctx, &request);
    assert!(app.replay_debugger.notice.is_some());
    assert!(app.message.is_none());
    assert_eq!(app.personal.history.len(), history);
    assert_eq!(state(&app), before);
}
