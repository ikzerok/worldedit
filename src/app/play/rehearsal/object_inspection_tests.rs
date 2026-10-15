use super::*;
use crate::app::play::{inspection::SourceRequest, object_inspection::RunKind};
use worldline_core::TargetRef;

#[test]
fn object_side_query_uses_existing_draft_run_and_roundtrips_without_restarting() {
    let (ctx, mut app) = app();
    let source = "world room as \"草稿地点\"\ntag calm\ntag alert\nstate mood on world room with calm as \"心境\"\nevent start\n  become mood with alert\n  choice \"继续\"\n    -> END\n";
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    buffer.replace_source(source.into());
    app.manuscript.restore_writing_buffers(&[buffer]);
    start(&ctx, &mut app);
    app.tab = Tab::Manuscript;
    let stamp = app
        .draft_rehearsal
        .running
        .as_ref()
        .unwrap()
        .view
        .inspection
        .as_ref()
        .unwrap()
        .stamp;
    let input = app.manuscript.writing_buffers()[0].source().to_owned();
    let project = app.project.content_baseline();
    let turns = app.draft_rehearsal.running.as_ref().unwrap().view.turns;
    app.open_object_inspection(&ctx, TargetRef::new("world", "room"));
    app.select_object_run(RunKind::Draft);
    app.refresh_object_inspection(&ctx);
    wait_idle(&ctx, &mut app);
    app.refresh_object_inspection(&ctx);
    let page = app.replay_debugger.object_inspection.page.as_ref().unwrap();
    assert_eq!(page.stamp, stamp);
    assert_eq!(page.owner.as_ref().unwrap().display, "草稿地点");
    assert_eq!(page.total_items, 1);
    let item = &page.items[0];
    assert_ne!(item.state_definition.as_ref().unwrap().initial, item.first);
    let request = SourceRequest {
        stamp,
        key: item.key.clone(),
        source: item.source.clone().unwrap(),
    };
    app.jump_to_object_inspection_source(&ctx, &request);
    wait_idle(&ctx, &mut app);
    assert!(
        !app.replay_debugger.object_inspection.open,
        "{:?}",
        app.replay_debugger.object_inspection.error
    );
    app.author_back(&ctx);
    assert!(app.replay_debugger.object_inspection.open);
    assert_eq!(app.replay_debugger.object_inspection.stamp, Some(stamp));
    assert_eq!(app.project.content_baseline(), project);
    assert_eq!(app.manuscript.writing_buffers()[0].source(), input);
    assert_eq!(
        app.draft_rehearsal.running.as_ref().unwrap().view.turns,
        turns
    );
    assert_eq!(
        app.draft_rehearsal
            .running
            .as_ref()
            .unwrap()
            .view
            .inspection
            .as_ref()
            .unwrap()
            .stamp,
        stamp
    );

    // 同一 stamp 下换对象、且仍停在试玩标签：旧异步回包也不能走普通回源分支。
    app.tab = Tab::Play;
    app.jump_to_object_inspection_source(&ctx, &request);
    assert!(app
        .draft_rehearsal
        .running
        .as_ref()
        .unwrap()
        .object_source
        .is_some());
    app.open_object_inspection(&ctx, TargetRef::new("event", "start"));
    app.select_object_run(RunKind::Draft);
    let history = app.personal.history.clone();
    wait_idle(&ctx, &mut app);
    assert_eq!(app.tab, Tab::Play);
    assert!(app.personal.history == history);
    assert_eq!(
        app.replay_debugger.object_inspection.target,
        Some(TargetRef::new("event", "start"))
    );
    assert!(app.replay_debugger.object_inspection.open);
    assert!(app
        .replay_debugger
        .object_inspection
        .error
        .as_deref()
        .is_some_and(|error| error.contains("来源返回时")));
    assert!(app
        .draft_rehearsal
        .running
        .as_ref()
        .unwrap()
        .object_source
        .is_none());
}

#[test]
fn object_draft_query_stays_frozen_after_author_edits_and_refuses_old_source() {
    let (ctx, mut app) = app();
    let source = "world room\ntag calm\nstate mood on world room with calm\nevent start\n  choice \"继续\"\n    -> END\n";
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    buffer.replace_source(source.into());
    app.manuscript.restore_writing_buffers(&[buffer]);
    start(&ctx, &mut app);
    app.open_object_inspection(&ctx, TargetRef::new("world", "room"));
    app.select_object_run(RunKind::Draft);
    app.refresh_object_inspection(&ctx);
    wait_idle(&ctx, &mut app);
    app.refresh_object_inspection(&ctx);
    let page = app.replay_debugger.object_inspection.page.clone().unwrap();
    let mut buffer = app.manuscript.writing_buffers().remove(0);
    buffer.replace_source(source.replacen("world room", "world room as \"新稿\"", 1));
    app.manuscript.restore_writing_buffers(&[buffer]);
    app.refresh_object_inspection(&ctx);
    assert_eq!(
        app.replay_debugger.object_inspection.page.as_ref(),
        Some(&page)
    );
    assert!(app
        .replay_debugger
        .object_inspection
        .source_reason
        .is_some());
    let item = &page.items[0];
    app.jump_to_object_inspection_source(
        &ctx,
        &SourceRequest {
            stamp: page.stamp,
            key: item.key.clone(),
            source: item.source.clone().unwrap(),
        },
    );
    assert!(app.replay_debugger.object_inspection.error.is_some());
    assert!(app
        .draft_rehearsal
        .running
        .as_ref()
        .unwrap()
        .object_source
        .is_none());
}

#[test]
fn object_side_query_never_changes_main_all_global_or_other_owner_state() {
    use worldline_runtime::InspectionGroup;
    let (ctx, mut app) = app();
    let source = "world room\ncharacter hero\ntag calm\nstate room_mood on world room with calm\nstate hero_mood on character hero with calm\nlet n = 7\nevent start\n  choice \"继续\"\n    set n = 8\n    -> END\n";
    let mut buffer = app
        .project
        .open_source_writing_buffer(&app.active_file)
        .unwrap();
    buffer.replace_source(source.into());
    app.manuscript.restore_writing_buffers(&[buffer]);
    start(&ctx, &mut app);
    let running = app.draft_rehearsal.running.as_ref().unwrap();
    let original = running.view.inspection.clone().unwrap();
    let turns = running.view.turns;
    let main_query = running.query.clone();
    app.open_object_inspection(&ctx, TargetRef::new("world", "room"));
    app.select_object_run(RunKind::Draft);
    app.refresh_object_inspection(&ctx);
    wait_idle(&ctx, &mut app);
    app.refresh_object_inspection(&ctx);
    let side = app.replay_debugger.object_inspection.page.as_ref().unwrap();
    assert_eq!(side.total_items, 1);
    assert_eq!(side.items[0].key.name, "room_mood");
    let running = app.draft_rehearsal.running.as_ref().unwrap();
    assert_eq!(running.query, main_query);
    assert_eq!(running.submitted_query, main_query);
    assert_eq!(running.view.inspection.as_ref(), Some(&original));
    app.replay_debugger.object_inspection.open = false;
    for group in [
        None,
        Some(InspectionGroup::Global),
        Some(InspectionGroup::Local),
        None,
    ] {
        app.send_rehearsal_action(Action::Inspect {
            query: StateInspectionQuery {
                group,
                ..Default::default()
            },
        });
        wait_idle(&ctx, &mut app);
        assert!(
            app.draft_rehearsal.notice.is_none(),
            "{:?}",
            app.draft_rehearsal.notice
        );
        let running = app.draft_rehearsal.running.as_ref().unwrap();
        let page = running.view.inspection.as_ref().unwrap();
        assert!(page.owner.is_none());
        assert_eq!(page.stamp, original.stamp);
        assert_eq!(running.view.turns, turns);
        if group.is_none() {
            assert!(page.items.iter().any(|item| item.key.name == "hero_mood"));
            assert!(page.items.iter().any(|item| item.key.name == "n"));
        } else if group == Some(InspectionGroup::Global) {
            assert!(page
                .items
                .iter()
                .all(|item| item.key.group == InspectionGroup::Global));
            assert!(page.items.iter().any(|item| item.key.name == "n"));
        }
    }
}

#[test]
fn object_transport_failure_keeps_old_page_marks_failed_filter_and_clears_source_pending() {
    for source_request in [false, true] {
        let (ctx, mut app) = app();
        let mut buffer = app
            .project
            .open_source_writing_buffer(&app.active_file)
            .unwrap();
        buffer.replace_source("world room\ntag calm\nstate mood on world room with calm\nevent start\n  choice \"继续\"\n    -> END\n".into());
        app.manuscript.restore_writing_buffers(&[buffer]);
        start(&ctx, &mut app);
        app.open_object_inspection(&ctx, TargetRef::new("world", "room"));
        app.select_object_run(RunKind::Draft);
        app.refresh_object_inspection(&ctx);
        wait_idle(&ctx, &mut app);
        app.refresh_object_inspection(&ctx);
        let old = app.object_inspection_page_for_test().unwrap().clone();
        let main = app
            .draft_rehearsal
            .running
            .as_ref()
            .unwrap()
            .view
            .inspection
            .clone();
        let turns = app.draft_rehearsal.running.as_ref().unwrap().view.turns;
        let location = app.author_location(Some(&ctx));
        if source_request {
            app.jump_to_object_inspection_source(
                &ctx,
                &SourceRequest {
                    stamp: old.stamp,
                    key: old.items[0].key.clone(),
                    source: old.items[0].source.clone().unwrap(),
                },
            );
            assert!(app
                .draft_rehearsal
                .running
                .as_ref()
                .unwrap()
                .object_source
                .is_some());
        } else {
            let query = app.object_inspection_query_for_test();
            query.text = "不存在的状态".into();
            query.offset = 1;
            app.refresh_object_inspection(&ctx);
            assert!(
                app.draft_rehearsal
                    .running
                    .as_ref()
                    .unwrap()
                    .object_query_pending
            );
        }
        // 请求经过真实会话；在响应抵达前注入真实 mpsc 断连，走 SessionWorker::poll 的 Err。
        app.draft_rehearsal.running.as_mut().unwrap().worker =
            Some(SessionWorker::disconnected_for_test());
        app.refresh_object_inspection(&ctx);
        let running = app.draft_rehearsal.running.as_ref().unwrap();
        assert!(!running.object_query_pending);
        assert!(running.object_source.is_none());
        assert!(running.worker.is_none());
        assert_eq!(running.view.inspection, main);
        assert_eq!(running.view.turns, turns);
        assert_eq!(running.object_page.as_ref(), Some(&old));
        if !source_request {
            assert!(running
                .object_query_error
                .as_deref()
                .is_some_and(|e| e.contains("意外退出")));
        }
        assert!(app
            .replay_debugger
            .object_inspection
            .error
            .as_deref()
            .is_some_and(|e| e.contains("意外退出")));
        assert_eq!(app.object_inspection_page_for_test(), Some(&old));
        assert!(app.replay_debugger.object_inspection.open);
        assert!(app.author_location(Some(&ctx)) == location);
        // 后续帧不得把同 owner、同 stamp 的旧页误认为新 text/offset 已成功。
        app.refresh_object_inspection(&ctx);
        assert!(app.replay_debugger.object_inspection.error.is_some());
        assert_eq!(app.object_inspection_page_for_test(), Some(&old));
    }
}
