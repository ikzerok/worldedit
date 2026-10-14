use super::super::delivery::Action;
use super::*;

#[test]
fn production_flow_cancel_real_job_and_late_result_never_replace_new_scope() {
    let _serial = serial();
    let mut flow = Flow::new(45);
    let disk = flow.disk();
    flow.generate();
    let first = flow.snapshot();
    flow.preview();
    flow.confirm();
    flow.start();
    assert!(!flow.app.manuscript.production.confirmed);
    flow.hold_poll = true;
    flow.click("取消生成");
    flow.hold_poll = false;
    assert!(flow.app.manuscript.production.job.is_none());
    assert!(flow.notice().contains("已取消"));
    assert_eq!(flow.snapshot().key(), first.key());
    assert!(!flow.app.manuscript.production.confirmed);
    // Starting waits for BUSY from the cancelled native worker to be released.
    flow.app.manuscript.production.search = "main_0".into();
    flow.start();
    flow.app.manuscript.production.search = "main_44".into();
    flow.finish_job();
    assert!(flow.notice().contains("已过期"), "{}", flow.notice());
    assert_eq!(flow.snapshot().key(), first.key());
    assert!(!flow.app.production_is_current());
    flow.generate();
    let rows = flow.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].stable_line_id.as_deref(), Some("main_44"));
    let latest = flow.snapshot();
    for _ in 0..3 {
        flow.frame(vec![]);
    }
    assert_eq!(flow.snapshot().key(), latest.key());
    assert!(!flow.app.manuscript.production.confirmed);
    assert_eq!(flow.disk(), disk);
    assert_eq!(fs::read_dir(&flow.directory).unwrap().count(), 1);
}

#[test]
fn production_flow_same_generation_branch_invalid_draft_and_book_draft_reject_old_artifact() {
    let _serial = serial();
    let mut flow = Flow::new(3);
    let path = flow.app.project.entry.clone();
    let original = flow.app.manuscript.writing_buffers[&path].clone();
    flow.stage("Line 00", "BRANCH_A");
    flow.generate();
    flow.preview();
    flow.confirm();
    let artifact = flow.artifact();
    let first = flow.app.manuscript.writing_buffers[&path].clone();
    let mut branch = original;
    branch.replace_source(branch.source().replace("Line 00", "BRANCH_B"));
    assert_eq!(branch.generation(), first.generation());
    flow.app
        .manuscript
        .writing_buffers
        .insert(path.clone(), branch.clone());
    assert!(!flow.app.production_is_current());
    assert!(flow.app.checked_production_artifact(&flow.ctx).is_err());
    assert_eq!(flow.artifact(), artifact);
    flow.generate();
    assert!(flow
        .rows()
        .iter()
        .any(|row| text(&row.source_parts).contains("BRANCH_B")));
    assert!(!flow
        .rows()
        .iter()
        .any(|row| text(&row.source_parts).contains("BRANCH_A")));
    let current = flow.snapshot();
    let local = flow.app.manuscript.books.get_mut("book").unwrap();
    local.draft.title = "Unapplied book draft".into();
    local.changed = true;
    assert!(!flow.app.production_is_current());
    flow.generate();
    assert_ne!(flow.snapshot().key(), current.key());
    flow.stage("BRANCH_B", "BRANCH_NEWER");
    let buffer = flow.app.manuscript.writing_buffers.get_mut(&path).unwrap();
    buffer.replace_source("event start\n  if (\n".into());
    let invalid = buffer.clone();
    flow.start();
    flow.finish_job();
    assert!(!flow.app.production_is_current());
    assert!(flow.app.manuscript.production.captured.is_none());
    flow.app
        .finish_production_export(&flow.ctx, Action::Preview);
    assert!(flow.app.manuscript.production.artifact.is_none());
    assert_eq!(
        flow.app.manuscript.writing_buffers[&path].identity(),
        invalid.identity()
    );
    assert!(!flow.app.project.is_dirty());
}

#[test]
fn production_flow_direction_role_metadata_and_locale_revalidate_at_delivery() {
    let _serial = serial();
    let mut flow = Flow::new(3);
    flow.generate();
    flow.preview();
    flow.confirm();
    let first = flow.snapshot();
    let revision = flow
        .rows()
        .iter()
        .find(|row| row.stable_line_id.as_deref() == Some("main_0"))
        .unwrap()
        .source_revision
        .clone();
    let entry = flow.app.project.entry.clone();
    let source = flow
        .app
        .project
        .document(&entry)
        .unwrap()
        .replace(PRIVATE, "CHANGED_PRIVATE");
    flow.app.project.set_text(&entry, source).unwrap();
    // Even before a host recompile updates the cheap UI version, the final core
    // receipt check rejects changed source and cannot publish old material.
    assert!(flow.app.checked_production_artifact(&flow.ctx).is_err());
    flow.app.recompile();
    assert!(!flow.app.production_is_current());
    flow.generate();
    assert_ne!(flow.snapshot().key(), first.key());
    let row = flow
        .rows()
        .into_iter()
        .find(|row| row.stable_line_id.as_deref() == Some("main_0"))
        .unwrap();
    assert_eq!(row.source_revision, revision);
    let second = flow.snapshot();
    let source = flow
        .app
        .project
        .document(&entry)
        .unwrap()
        .replace("character a as \"同名\"", "character a as \"新显示名\"");
    flow.app.project.set_text(&entry, source).unwrap();
    flow.app.recompile();
    flow.generate();
    assert_ne!(flow.snapshot().key(), second.key());
    assert!(flow
        .rows()
        .iter()
        .all(|row| row.speaker.as_ref().unwrap().display == "新显示名"));
    assert_eq!(
        flow.rows()
            .iter()
            .find(|row| row.stable_line_id.as_deref() == Some("main_0"))
            .unwrap()
            .source_revision,
        revision
    );
    flow.preview();
    flow.confirm();
    let mut locale = flow.locale();
    locale["entries"]["main_0"]["translation_parts"] =
        json!([{"type":"text","text":"Changed translation"}]);
    flow.set_locale(&locale);
    assert!(!flow.app.production_is_current());
    assert!(flow.app.checked_production_artifact(&flow.ctx).is_err());
}

#[test]
fn production_flow_source_navigation_returns_same_query_and_preserves_new_draft() {
    let _serial = serial();
    let mut flow = Flow::new(45);
    let entry = flow.app.project.entry.clone();
    flow.stage("Line 00", "DRAFT_SOURCE");
    let clean_path = flow.app.project.root.join("shared.wl");
    let clean = flow
        .app
        .project
        .open_source_writing_buffer(&clean_path)
        .unwrap();
    flow.app
        .manuscript
        .writing_buffers
        .insert(clean_path.clone(), clean);
    flow.generate();
    flow.click("下一页台词");
    flow.frame(vec![]);
    let snapshot = flow.snapshot();
    let row = flow
        .rows()
        .into_iter()
        .find(|row| row.stable_line_id.as_deref() == Some("main_0"))
        .unwrap();
    let hit = flow
        .app
        .project
        .production_script_source_hit(
            &flow.app.production_buffers(),
            &flow.app.production_drafts(),
            &snapshot,
            &row.row_key,
        )
        .unwrap();
    assert_eq!(hit.path, entry);
    assert!(hit.draft && hit.preview.contains("DRAFT_SOURCE"));
    let draft = flow.app.manuscript.writing_buffers[&entry].clone();
    flow.app.navigate_production_source(&flow.ctx, &row);
    assert!(!flow.app.manuscript.production.open, "{}", flow.notice());
    assert!(flow.app.manuscript.production.return_origin.is_some());
    assert_eq!(
        flow.app.manuscript.writing_buffers[&entry].identity(),
        draft.identity()
    );
    flow.stage("DRAFT_SOURCE", "NEWER_SOURCE");
    let newer = flow.app.manuscript.writing_buffers[&entry].clone();
    flow.app.author_back(&flow.ctx);
    flow.frame(vec![]);
    assert!(flow.app.manuscript.production.open);
    assert_eq!(flow.app.manuscript.production.offset, 40);
    assert_eq!(flow.snapshot().key(), snapshot.key());
    assert!(!flow.app.production_is_current());
    assert_eq!(
        flow.app.manuscript.writing_buffers[&entry].identity(),
        newer.identity()
    );
    let location = flow.app.author_location(Some(&flow.ctx));
    flow.app.navigate_production_source(&flow.ctx, &row);
    assert!(flow.app.author_location(Some(&flow.ctx)) == location);
    assert!(flow.notice().contains("已过期"));
    flow.generate();
    let leaf = flow
        .rows()
        .into_iter()
        .find(|row| row.stable_line_id.as_deref() == Some("leaf"))
        .unwrap();
    let hit = flow
        .app
        .project
        .production_script_source_hit(
            &flow.app.production_buffers(),
            &flow.app.production_drafts(),
            &flow.snapshot(),
            &leaf.row_key,
        )
        .unwrap();
    assert_eq!(hit.path, clean_path);
    assert!(!hit.draft);
    flow.app.navigate_production_source(&flow.ctx, &leaf);
    assert_eq!(flow.app.active_file, clean_path);
    assert!(!flow.app.manuscript.production.open, "{}", flow.notice());
    assert_eq!(
        flow.app.manuscript.writing_buffers[&entry].identity(),
        newer.identity()
    );
    flow.app.author_back(&flow.ctx);
    flow.frame(vec![]);
    assert!(flow.app.manuscript.production.open);
    assert!(flow.app.production_is_current());
}

#[test]
fn production_flow_formal_role_sidebar_uses_id_and_keeps_query_and_draft() {
    let _serial = serial();
    let mut flow = Flow::new(3);
    flow.stage("Line 00", "DRAFT_KEPT");
    flow.app.manuscript.production.speaker = Some(TargetRef::new("character", "b"));
    flow.generate();
    assert_eq!(flow.rows().len(), 1);
    let key = flow.snapshot().key().to_owned();
    let draft = flow.app.manuscript.writing_buffers[&flow.app.project.entry].identity();
    flow.click("同名");
    assert_eq!(
        flow.app.reading_target,
        Some(TargetRef::new("character", "b"))
    );
    assert!(flow.app.manuscript.production.open);
    flow.app.close_transient_reading();
    assert!(flow.app.reading_target.is_none());
    assert_eq!(flow.snapshot().key(), key);
    assert!(flow.app.production_is_current());
    assert_eq!(
        flow.app.manuscript.writing_buffers[&flow.app.project.entry].identity(),
        draft
    );
}

#[test]
fn production_flow_unpolled_disk_change_blocks_copy_save_and_source_navigation() {
    let _serial = serial();
    let mut flow = Flow::new(3);
    flow.generate();
    flow.preview();
    flow.confirm();
    let row = flow.rows()[0].clone();
    let location = flow.app.author_location(Some(&flow.ctx));
    let disk = fs::read_to_string(&flow.app.project.entry)
        .unwrap()
        .replace("Line 00", "EXTERNAL_EDIT");
    fs::write(&flow.app.project.entry, &disk).unwrap();
    assert!(flow.app.checked_production_artifact(&flow.ctx).is_err());
    let output = flow.ctx.run(Default::default(), |ctx| {
        flow.app.finish_production_export(ctx, Action::Copy)
    });
    assert!(copied(&output).is_none());
    let destination = flow.directory.join("blocked.json");
    flow.app.manuscript.production.destination = destination.to_string_lossy().into_owned();
    flow.app.finish_production_export(&flow.ctx, Action::Save);
    assert!(!destination.exists());
    flow.app.navigate_production_source(&flow.ctx, &row);
    assert!(flow.app.author_location(Some(&flow.ctx)) == location);
    assert!(flow.app.manuscript.production.open);
    assert_eq!(fs::read_to_string(&flow.app.project.entry).unwrap(), disk);
}

#[test]
fn production_flow_uninserted_continuation_blocks_generation_and_preserves_clean_buffer() {
    use worldline_core::manuscript::{DialogueEditRequest, DialogueKind, DialogueOperation};
    let _serial = serial();
    let mut flow = Flow::new(3);
    flow.generate();
    flow.preview();
    flow.confirm();
    let disk = flow.disk();
    let buffer = flow.app.manuscript.writing_buffers[&flow.app.project.entry].clone();
    let target = TargetRef::new("event", "start");
    let projection = flow
        .app
        .project
        .project_dialogue_buffer(&buffer, &target)
        .unwrap();
    let statement = projection
        .statements
        .iter()
        .find(|statement| statement.kind == DialogueKind::Say)
        .unwrap();
    let request = DialogueEditRequest {
        schema_version: 1,
        expected_baseline: buffer.baseline().into(),
        target,
        generation: buffer.generation(),
        operation: DialogueOperation::Update {
            statement_id: statement.id.clone(),
            draft: statement.draft.clone(),
        },
        enable_language_1_11: false,
    };
    let plan = flow
        .app
        .project
        .preview_dialogue_edit(&buffer, &request)
        .unwrap();
    assert!(plan.no_change);
    flow.app
        .manuscript
        .writing_view
        .begin_dialogue_continuation(&flow.ctx, &flow.app.project, &buffer, &plan)
        .unwrap();
    assert!(flow.app.manuscript.writing_view.has_retained_input());
    assert!(!flow.app.production_is_current());
    flow.app.begin_production_script(&flow.ctx);
    assert!(flow.app.manuscript.production.job.is_none());
    assert!(flow.notice().contains("未插入"), "{}", flow.notice());
    assert!(flow.app.checked_production_artifact(&flow.ctx).is_err());
    assert!(flow.app.manuscript.writing_view.has_retained_input());
    let actual = &flow.app.manuscript.writing_buffers[&flow.app.project.entry];
    assert_eq!(actual.identity(), buffer.identity());
    assert!(!actual.is_changed());
    assert!(!flow.app.project.is_dirty());
    assert_eq!(flow.disk(), disk);
}

#[test]
fn production_flow_attachment_observation_invalidates_receipt_and_pending_job_without_content_edit()
{
    let _serial = serial();
    let mut flow = Flow::new(3);
    let entry = flow.app.project.entry.clone();
    let source = format!(
        "asset art image \"art.png\"\n{}",
        flow.app.project.document(&entry).unwrap()
    );
    flow.app.project.set_text(&entry, source).unwrap();
    let asset = flow.app.project.root.join("art.png");
    fs::write(&asset, b"fixture image bytes").unwrap();
    flow.app.project.save().unwrap();
    flow.app.project.refresh().unwrap();
    flow.app.recompile();
    let baseline = flow.app.project.content_baseline();
    let version = flow.app.version;
    let buffer_identity = flow.app.manuscript.writing_buffers[&entry].identity();
    let observed = flow.app.project.catalog_scope_observation_key();
    flow.generate();
    flow.preview();
    flow.confirm();
    let first = flow.snapshot();
    let old_row = flow.rows()[0].clone();
    fs::remove_file(&asset).unwrap();
    assert!(flow.app.project.refresh().unwrap().is_empty());
    assert_ne!(flow.app.project.catalog_scope_observation_key(), observed);
    assert_eq!(flow.app.project.content_baseline(), baseline);
    assert_eq!(
        flow.app.manuscript.writing_buffers[&entry].identity(),
        buffer_identity
    );
    assert_eq!(flow.app.version, version);
    assert!(!flow.app.production_is_current());
    assert!(flow.app.checked_production_artifact(&flow.ctx).is_err());
    let location = flow.app.author_location(Some(&flow.ctx));
    flow.app.navigate_production_source(&flow.ctx, &old_row);
    assert!(flow.app.author_location(Some(&flow.ctx)) == location);
    assert_eq!(flow.snapshot().key(), first.key());
    fs::write(&asset, b"fixture image bytes").unwrap();
    assert!(flow.app.project.refresh().unwrap().is_empty());
    assert_eq!(flow.app.project.content_baseline(), baseline);
    assert_eq!(flow.app.version, version);
    flow.generate();
    let restored = flow.snapshot();
    flow.start();
    fs::remove_file(&asset).unwrap();
    assert!(flow.app.project.refresh().unwrap().is_empty());
    assert_eq!(flow.app.project.content_baseline(), baseline);
    assert_eq!(
        flow.app.manuscript.writing_buffers[&entry].identity(),
        buffer_identity
    );
    assert_eq!(flow.app.version, version);
    flow.finish_job();
    assert!(flow.notice().contains("已过期"), "{}", flow.notice());
    assert_eq!(flow.snapshot().key(), restored.key());
    assert!(!flow.app.production_is_current());
    assert!(!flow.app.manuscript.production.confirmed);
    // Drain the cancelled worker through the next admitted real job before cleanup.
    fs::write(&asset, b"fixture image bytes").unwrap();
    flow.app.project.refresh().unwrap();
    flow.generate();
    assert_eq!(flow.app.project.content_baseline(), baseline);
    assert_eq!(flow.app.version, version);
    assert!(!flow.app.project.is_dirty());
}
