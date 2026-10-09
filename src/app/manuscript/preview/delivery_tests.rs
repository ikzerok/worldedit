//! 真实core生成+离屏egui回归；不冒称物理IME、原生窗口或浏览器落盘验收。
use super::*;
use crate::app::{Tab, WorldeditApp};
use std::sync::atomic::{AtomicU64, Ordering};
use worldline_core::manuscript::{generate_manuscript_delivery, ManuscriptDeliverySnapshot};
use worldline_core::project::Project;
mod responsive;
const SOURCE: &str = "event start\n  if true\n    甲😀\n  else\n    乙\n  -> END\nevent second\n  第二章\n  -> END\n";

fn fixture() -> (egui::Context, WorldeditApp) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let ctx = egui::Context::default();
    let creation = eframe::CreationContext::_new_kittest(ctx.clone());
    let mut app = WorldeditApp::new(&creation, None);
    let root = std::env::temp_dir().join(format!(
        "editor-delivery-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    app.project = Project::new(&root);
    let path = app.project.entry.clone();
    app.project.documents.retain(|entry, _| entry == &path);
    app.project.set_text(&path, SOURCE.into()).unwrap();
    app.project.create_authoring_document(&root.join(".world/project.json"), br#"{"schema_version":1,"required_features":["presentation.manuscripts.v1"],"manuscripts":{"book":".world/manuscripts/book.json"}}"#.to_vec()).unwrap();
    app.project.create_authoring_document(&root.join(".world/manuscripts/book.json"), br#"{"schema_version":1,"id":"book","title":"Book","entries":[{"id":"one","kind":"chapter","title":"One","status":"review","target_ref":{"kind":"event","id":"start"}},{"id":"two","kind":"chapter","title":"Two","status":"draft","target_ref":{"kind":"event","id":"second"}}]}"#.to_vec()).unwrap();
    app.active_file = path;
    app.reset_views();
    app.recompile();
    app.tab = Tab::Manuscript;
    frame(&ctx, &mut app, egui::vec2(1200.0, 800.0));
    (ctx, app)
}
fn frame(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            ..Default::default()
        },
        |ctx| app.manuscript_tab(ctx),
    )
}
fn generate(app: &mut WorldeditApp) {
    let input = app.manuscript_delivery_input().unwrap();
    let snapshot = ManuscriptDeliverySnapshot::new(
        app.manuscript_delivery_snapshot().unwrap(),
        &input.request,
    )
    .unwrap();
    let report = generate_manuscript_delivery(snapshot, &mut |_| true).unwrap();
    assert!(report.complete());
    let state = &mut app.manuscript.preview_cache.delivery;
    state.review_pages = super::delivery_view::bounded_pages(&report);
    state.reviewed = Some(Arc::new(report));
    state.captured = Some(input);
    state.confirmed = false;
    app.manuscript.preview_cache.scoped = true;
}

#[test]
fn current_filter_material_and_copy_use_same_bytes_and_leave_author_state_untouched() {
    let (ctx, mut app) = fixture();
    app.manuscript.navigation.session.status = "review".into();
    frame(&ctx, &mut app, egui::vec2(800.0, 600.0));
    generate(&mut app);
    let before = app.project.content_baseline();
    let history = app.history.len();
    let expected = app
        .manuscript
        .preview_cache
        .delivery
        .reviewed
        .as_ref()
        .unwrap()
        .markdown()
        .unwrap()
        .to_owned();
    assert!(!expected.contains("第二章"));
    assert!(expected.contains("甲😀") && expected.contains("乙"));
    assert!(app.checked_manuscript_markdown(&ctx).is_err());
    app.manuscript.preview_cache.delivery.confirmed = true;
    let output = ctx.run(Default::default(), |ctx| app.copy_manuscript_markdown(ctx));
    assert!(output.platform_output.commands.iter().any(
        |command| matches!(command, egui::OutputCommand::CopyText(text) if text == &expected)
    ));
    assert_eq!(app.project.content_baseline(), before);
    assert_eq!(app.history.len(), history);
    assert_eq!(app.manuscript.navigation.session.status, "review");
}

#[test]
fn scoped_draft_source_and_return_preserve_query_page_and_exact_source_then_regenerate() {
    let (ctx, mut app) = fixture();
    let path = app.project.entry.clone();
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source(SOURCE.replace("甲😀", "新草稿😀"));
    app.manuscript.navigation.session.status = "review".into();
    app.manuscript.navigation.session.scroll_y = 32.0;
    frame(&ctx, &mut app, egui::vec2(1200.0, 800.0));
    generate(&mut app);
    let report = app
        .manuscript
        .preview_cache
        .delivery
        .reviewed
        .as_ref()
        .unwrap()
        .clone();
    let review = report.chapters()[0].review.as_ref().unwrap().clone();
    fn find(
        nodes: &[worldline_core::manuscript::ReviewNode],
    ) -> Option<worldline_core::manuscript::ReviewSource> {
        nodes.iter().find_map(|node| {
            node.source
                .as_ref()
                .filter(|source| source.excerpt.contains("新草稿"))
                .cloned()
                .or_else(|| find(&node.children))
        })
    }
    let request = super::super::review_navigation::ReviewRequest {
        key: report.scope().snapshot_key.clone(),
        source: find(&review.nodes).unwrap(),
        review,
    };
    let before = app.manuscript_session();
    let baseline = app.project.content_baseline();
    app.jump_review_source(&ctx, &request).unwrap();
    assert!(app.review_return_available());
    frame(&ctx, &mut app, egui::vec2(1200.0, 800.0));
    app.manuscript.review_navigation.back = true;
    app.finish_review_navigation(&ctx);
    frame(&ctx, &mut app, egui::vec2(1200.0, 800.0));
    let after = app.manuscript_session();
    assert_eq!(after.preview_scoped, before.preview_scoped);
    assert_eq!(after.selected_id, before.selected_id);
    assert_eq!(
        after.navigation.unwrap().status,
        before.navigation.unwrap().status
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.manuscript_delivery_is_current());
    assert!(Arc::ptr_eq(
        &report,
        app.manuscript
            .preview_cache
            .delivery
            .reviewed
            .as_ref()
            .unwrap()
    ));
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source(SOURCE.replace("甲😀", "再次改稿😀"));
    assert!(!app.manuscript_delivery_is_current());
    assert!(app.jump_review_source(&ctx, &request).is_err());
    frame(&ctx, &mut app, egui::vec2(1200.0, 800.0));
    generate(&mut app);
    assert!(app
        .manuscript
        .preview_cache
        .delivery
        .reviewed
        .as_ref()
        .unwrap()
        .markdown()
        .unwrap()
        .contains("再次改稿😀"));
}

#[test]
fn changed_query_ime_and_project_edits_block_delivery_without_clearing_inputs() {
    let (ctx, mut app) = fixture();
    generate(&mut app);
    app.manuscript.preview_cache.delivery.confirmed = true;
    app.ime_composing = true;
    assert!(app
        .checked_manuscript_markdown(&ctx)
        .unwrap_err()
        .contains("输入法"));
    app.ime_composing = false;
    app.manuscript.navigation.session.text = "changed".into();
    assert!(!app.manuscript_delivery_is_current());
    assert!(app.checked_manuscript_markdown(&ctx).is_err());
    app.manuscript.navigation.session.text.clear();
    let path = app.project.entry.clone();
    app.project
        .set_text(&path, SOURCE.replace("甲😀", "已应用改稿"))
        .unwrap();
    assert!(app.checked_manuscript_markdown(&ctx).is_err());
    assert!(app.manuscript.preview_cache.delivery.reviewed.is_some());
}

#[test]
fn changed_arrangement_still_blocks_the_same_report_before_and_after_query_refresh() {
    let (ctx, mut app) = fixture();
    generate(&mut app);
    app.manuscript.preview_cache.delivery.confirmed = true;
    let report = app
        .manuscript
        .preview_cache
        .delivery
        .reviewed
        .clone()
        .unwrap();
    let book = app.manuscript.books.get_mut("book").unwrap();
    book.draft.entries.reverse();
    book.changed = true;
    assert!(app.checked_manuscript_markdown(&ctx).is_err());
    frame(&ctx, &mut app, egui::vec2(1200.0, 800.0));
    assert!(!app.manuscript_delivery_is_current());
    assert!(app.checked_manuscript_markdown(&ctx).is_err());
    assert!(Arc::ptr_eq(
        &report,
        app.manuscript
            .preview_cache
            .delivery
            .reviewed
            .as_ref()
            .unwrap()
    ));
}

#[test]
fn native_background_repeat_start_and_cancel_have_one_owner_and_no_new_artifact() {
    let (ctx, mut app) = fixture();
    app.begin_manuscript_delivery(&ctx);
    assert!(app.manuscript.preview_cache.delivery.job.is_some());
    app.begin_manuscript_delivery(&ctx);
    app.cancel_manuscript_delivery();
    assert!(app.manuscript.preview_cache.delivery.job.is_none());
    assert!(app.manuscript.preview_cache.delivery.reviewed.is_none());
    app.poll_manuscript_delivery(&ctx);
    assert!(app.manuscript.preview_cache.delivery.reviewed.is_none());
}

#[test]
fn scope_markdown_small_windows_and_large_text_leave_actions_in_scrollable_surface() {
    let (ctx, mut app) = fixture();
    generate(&mut app);
    app.manuscript.reader_open = true;
    app.manuscript.narrow_preview = true;
    app.manuscript.preview_cache.delivery.markdown_open = true;
    for size in [egui::vec2(800.0, 600.0), egui::vec2(400.0, 300.0)] {
        let output = frame(&ctx, &mut app, size);
        assert!(!output.shapes.is_empty());
        assert!(app.manuscript.preview_cache.delivery.reviewed.is_some());
    }
}

#[test]
fn range_pages_use_real_core_sizes_and_keep_every_occurrence_and_error() {
    let (ctx, mut app) = fixture();
    let path = app.project.entry.clone();
    let long = format!(
        "event start\n{}  -> END\nevent second\n  第二章\n  -> END\n",
        "  有界正文😀\n".repeat(1700)
    );
    app.project.set_text(&path, long).unwrap();
    app.recompile();
    let book_path = app.project.root.join(".world/manuscripts/book.json");
    let entries: Vec<_> = (0..17).map(|position| serde_json::json!({"id":format!("chapter_{position}"),"kind":"chapter","title":"重复源","target_ref":{"kind":"event","id":if position == 16 {"missing"} else {"start"}}})).collect();
    let raw = serde_json::to_vec(
        &serde_json::json!({"schema_version":1,"id":"book","title":"书","entries":entries}),
    )
    .unwrap();
    app.project.set_authoring_document(&book_path, raw).unwrap();
    app.recompile();
    // 直接从真实当前Project生成，坏章仍有自己的出现位置。
    let query = Arc::new(app.project.manuscript_query_snapshot(&[], &[]).unwrap());
    let request = worldline_core::manuscript::ManuscriptDeliveryRequest::new(
        worldline_core::manuscript::ManuscriptQueryRequest {
            manuscript_id: "book".into(),
            ..Default::default()
        },
    );
    let report = generate_manuscript_delivery(
        ManuscriptDeliverySnapshot::new(query, &request).unwrap(),
        &mut |_| true,
    )
    .unwrap();
    assert!(
        report.chapters()[0].review.is_some(),
        "{:?}",
        report.chapters()[0].error
    );
    let pages = super::delivery_view::bounded_pages(&report);
    assert_eq!(pages.iter().map(|page| page.len()).sum::<usize>(), 17);
    assert!(pages.iter().any(|page| page.len() < 8));
    for (number, page) in pages.iter().enumerate() {
        if number > 0 {
            assert_eq!(pages[number - 1].end, page.start);
        }
        let mut budget = PageBudget::default();
        for chapter in &report.chapters()[page.clone()] {
            assert!(budget.admit(
                chapter
                    .review
                    .as_ref()
                    .map_or(0, |review| review.node_count),
                chapter.review_bytes
            ));
        }
    }
    assert!(report.chapters()[16].error.is_some());
    assert_eq!(pages.last().unwrap().end, 17);
    assert!(!report.complete());
    let _ = ctx;
}

#[test]
fn frozen_review_view_restores_after_edit_without_reusing_old_writing_anchor() {
    let (ctx, mut app) = fixture();
    generate(&mut app);
    app.manuscript.review_scroll_y = 143.0;
    let saved = app.manuscript_session();
    assert!(app
        .manuscript
        .preview_cache
        .can_restore_delivery_view(&saved));
    let path = app.project.entry.clone();
    app.manuscript
        .writing_buffer_mut(&path)
        .unwrap()
        .replace_source(SOURCE.replace("甲😀", "来源处已改写😀"));
    assert!(!app
        .manuscript
        .validate_session(&app.project, &saved)
        .unwrap());
    assert!(app
        .manuscript
        .preview_cache
        .can_restore_delivery_view(&saved));
    assert!(!app.manuscript_delivery_is_current());
    app.manuscript.preview_cache.delivery.confirmed = true;
    assert!(app.checked_manuscript_markdown(&ctx).is_err());
    frame(&ctx, &mut app, egui::vec2(1200.0, 800.0));
    generate(&mut app);
    assert!(!app
        .manuscript
        .preview_cache
        .can_restore_delivery_view(&saved));
}
