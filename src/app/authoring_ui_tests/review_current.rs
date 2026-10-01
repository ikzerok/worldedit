use super::*;
use crate::app::collaboration_ui::comment_lifecycle::ReviewAction;
use worldline_core::collaboration::{
    self, CommentAnchor, CommentCommand, CommentDraft, CommentResolutionFilter, CommentReviewFilter,
};

fn add_notes(app: &mut WorldeditApp, count: usize) {
    for at in 0..count {
        let expected_revision = app.map_revision;
        let expected_baseline = app.project.content_baseline();
        collaboration::write_comment(
            &mut app.project,
            &mut app.map_revision,
            CommentCommand {
                expected_revision,
                expected_baseline,
                original: None,
                draft: CommentDraft {
                    id: format!("revision_{at:03}"),
                    author: "作者".into(),
                    body: format!("第{at}条问题"),
                    anchor: CommentAnchor::Object {
                        target: TargetRef::new("entity", "a"),
                    },
                    resolved: at % 3 == 0,
                },
            },
        )
        .unwrap();
    }
    app.recompile();
}
fn guarded_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
    size: egui::Vec2,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), size)),
            events,
            ..Default::default()
        },
        |ctx| {
            app.review_navigation_guard(ctx);
            app.review_tab(ctx);
            app.draft_exit_dialog(ctx);
        },
    )
}
fn guarded_click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let size = vec2(1280.0, 800.0);
    for _ in 0..3 {
        guarded_frame(ctx, app, vec![], size);
    }
    let output = guarded_frame(ctx, app, vec![], size);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| text_position_contains(&shape.shape, label))
        .unwrap_or_else(|| panic!("未见 {label}"));
    for pressed in [true, false] {
        guarded_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            size,
        );
    }
}
#[test]
fn dirty_comment_same_other_new_filter_close_and_navigation_never_silently_replace() {
    let (ctx, mut app) = app();
    add_notes(&mut app, 3);
    app.edit_comment("revision_001");
    let initial = app.review.comment_editor.as_ref().unwrap().draft.clone();
    app.review
        .comment_editor
        .as_mut()
        .unwrap()
        .draft
        .body
        .push_str(" 未应用输入");
    let dirty = app.review.comment_editor.as_ref().unwrap().draft.clone();
    let before = app.project.content_baseline();
    let actions = vec![
        ReviewAction::Edit("revision_001".into()),
        ReviewAction::Edit("revision_002".into()),
        ReviewAction::New(initial.anchor.clone()),
        ReviewAction::Filter(CommentReviewFilter {
            resolution: CommentResolutionFilter::All,
            ..Default::default()
        }),
        ReviewAction::Close,
        ReviewAction::Navigate(Tab::Edit),
    ];
    for action in actions {
        app.request_review_action(action);
        assert!(app.review.pending_comment_action.is_some());
        assert_eq!(app.review.comment_editor.as_ref().unwrap().draft, dirty);
        guarded_click(&ctx, &mut app, "取消，保留批注输入");
        assert!(app.review.pending_comment_action.is_none());
        assert_eq!(app.review.comment_editor.as_ref().unwrap().draft, dirty);
        assert_eq!(app.project.content_baseline(), before);
    }
    app.edit_comment("revision_002");
    guarded_click(&ctx, &mut app, "明确放弃此批注输入并继续");
    assert_eq!(
        app.review.comment_editor.as_ref().unwrap().draft.id,
        "revision_002"
    );
    assert_eq!(app.project.content_baseline(), before);
}
#[test]
fn navigation_guard_preserves_draft_and_global_return_routes_back_to_review() {
    let (ctx, mut app) = app();
    add_notes(&mut app, 1);
    app.edit_comment("revision_000");
    app.review
        .comment_editor
        .as_mut()
        .unwrap()
        .draft
        .body
        .push_str(" 暂存");
    app.review_navigation_guard(&ctx);
    app.tab = Tab::Edit;
    app.review_navigation_guard(&ctx);
    assert_eq!(app.tab, Tab::Review);
    assert!(app.review.pending_comment_action.is_some());
    assert!(app
        .review
        .comment_editor
        .as_ref()
        .unwrap()
        .draft
        .body
        .ends_with("暂存"));
}
#[test]
fn selection_comment_preview_is_current_source_and_never_applies_unapplied_manuscript() {
    let (ctx, mut app) = manuscript_app();
    app.tab = Tab::Edit;
    let path = app.active_file.clone();
    let source = app.project.document(&path).unwrap().to_owned();
    let byte = source.find("甲乙").unwrap();
    let start = source[..byte].chars().count();
    crate::app::search::record_editor_selection(
        &ctx,
        egui::Id::new("source-test"),
        &path,
        None,
        &source,
        0,
        &source,
        egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(start + 2),
        ),
    );
    app.comment_current_selection(&ctx);
    let anchor = app
        .review
        .comment_editor
        .as_ref()
        .unwrap()
        .draft
        .anchor
        .clone();
    assert!(
        matches!(anchor,CommentAnchor::TextRange {ref quote,..} if quote.contains("甲乙") && quote.contains("[[character:"))
    );
    app.review.comment_editor = None;
    app.tab = Tab::Edit;
    let draft = source.replace("甲乙", "未应用");
    let before = app.project.content_baseline();
    crate::app::search::record_editor_selection(
        &ctx,
        egui::Id::new("source-test"),
        &path,
        None,
        &draft,
        0,
        &draft,
        egui::text::CCursorRange::two(
            egui::text::CCursor::new(start),
            egui::text::CCursor::new(start + 3),
        ),
    );
    app.comment_current_selection(&ctx);
    assert!(app.review.comment_editor.is_none());
    assert!(app.io_error.as_ref().unwrap().contains("未应用"));
    assert_eq!(app.project.content_baseline(), before);
}
#[test]
fn review_37_and_300_rows_search_tail_and_keyboard_scroll_selection_visible() {
    let (ctx, mut app) = app();
    add_notes(&mut app, 300);
    app.tab = Tab::Review;
    app.review.filter.resolution = CommentResolutionFilter::All;
    for size in [
        vec2(1040.0, 660.0),
        vec2(1280.0, 800.0),
        vec2(1600.0, 1000.0),
    ] {
        for dark in [true, false] {
            ctx.set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            app.review.filter.text = "revision_299".into();
            let output = guarded_frame(&ctx, &mut app, vec![], size);
            let mut text = String::new();
            for shape in &output.shapes {
                collect_text(&shape.shape, &mut text);
            }
            assert!(text.contains("命中 1 / 全部 300 · 未解决 200"), "{text}");
            assert!(text.contains("revision_299"), "{text}");
            let visible_position = |needle: &str| {
                output.shapes.iter().find_map(|clipped| {
                    text_position_contains(&clipped.shape, needle)
                        .filter(|point| clipped.clip_rect.contains(*point))
                })
            };
            let identity = visible_position("revision_299").expect("列表身份必须实际可见");
            let source = visible_position("entity:a").expect("来源必须独占实际可见一行");
            let summary = visible_position("第299条问题").expect("摘要必须实际可见而非只有悬停");
            assert!(
                identity.y < source.y && source.y < summary.y,
                "三条独立信息行"
            );
        }
    }
    app.review.filter.text.clear();
    app.review.selected_comment = Some("revision_298".into());
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("review-comment-list")));
    guarded_frame(&ctx, &mut app, vec![], vec2(1280.0, 800.0));
    let down = Event::Key {
        key: egui::Key::ArrowDown,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    guarded_frame(&ctx, &mut app, vec![down], vec2(1280.0, 800.0));
    assert_eq!(app.review.selected_comment.as_deref(), Some("revision_299"));
    for _ in 0..3 {
        guarded_frame(&ctx, &mut app, vec![], vec2(1280.0, 800.0));
    }
    let output = guarded_frame(&ctx, &mut app, vec![], vec2(1280.0, 800.0));
    let visible = output.shapes.iter().any(|clipped| {
        text_position_contains(&clipped.shape, "revision_299")
            .is_some_and(|point| clipped.clip_rect.contains(point))
    });
    assert!(visible, "键盘尾项必须进入真实裁剪区");
    app.review.filter.text = "not-found".into();
    app.review.selected_comment = Some("revision_299".into());
    let enter = Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    guarded_frame(&ctx, &mut app, vec![enter], vec2(1280.0, 800.0));
    assert!(app.review.comment_editor.is_none());
    // 37条规模也以同一投影而非首屏裁切返回末项。
    app.review.filter.text.clear();
    let mut projection = app.snapshot.as_ref().unwrap().comment_index.clone();
    projection
        .comments
        .retain(|id, _| id.as_str() <= "revision_036");
    assert_eq!(
        projection.review_projection(&app.review.filter).items.len(),
        37
    );
}

#[test]
fn comment_save_undo_redo_and_external_staleness_keep_inputs_and_source_independent() {
    let (ctx, mut app) = app();
    add_notes(&mut app, 1);
    app.project.save().unwrap();
    app.recompile();
    app.edit_comment("revision_000");
    let original = app
        .review
        .comment_editor
        .as_ref()
        .unwrap()
        .draft
        .body
        .clone();
    let sources = app.project.sources();
    let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
    app.review.comment_editor.as_mut().unwrap().draft.body = "修改批注而非正文".into();
    click(&ctx, &mut app, 7, "保存批注");
    assert!(app.review.comment_editor.is_none(), "{:?}", app.io_error);
    assert!(!app.comment_draft_dirty());
    assert!(!app.dirty_draft_names().contains(&"审阅批注"));
    app.project.save().unwrap();
    app.recompile();
    assert!(!app.project.is_dirty());
    assert!(!app.comment_draft_dirty());
    let saved_frame = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 800.0))),
            ..Default::default()
        },
        |ctx| {
            app.status_bar(ctx);
            app.review_tab(ctx);
        },
    );
    let mut saved_text = String::new();
    for shape in &saved_frame.shapes {
        collect_text(&shape.shape, &mut saved_text);
    }
    assert!(saved_text.contains("全部文件已保存"), "{saved_text}");
    assert!(!saved_text.contains("有未应用输入"), "{saved_text}");
    app.undo(false);
    assert_eq!(
        app.snapshot.as_ref().unwrap().comment_index.comments["revision_000"]
            .draft
            .body,
        original
    );
    app.undo(true);
    assert_eq!(
        app.snapshot.as_ref().unwrap().comment_index.comments["revision_000"]
            .draft
            .body,
        "修改批注而非正文"
    );
    assert_eq!(app.project.sources(), sources);
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
    app.edit_comment("revision_000");
    app.review
        .comment_editor
        .as_mut()
        .unwrap()
        .draft
        .body
        .push_str(" 过期仍保留");
    let entry = app.active_file.clone();
    let source = app.project.document(&entry).unwrap().to_owned() + "\n# 外部等价刷新\n";
    app.project.set_text(&entry, source).unwrap();
    app.recompile();
    let output = frame(&ctx, &mut app, vec![], 7);
    let mut text = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut text);
    }
    assert!(text.contains("旧批注表单不能覆盖当前稿"), "{text}");
    assert!(app
        .review
        .comment_editor
        .as_ref()
        .unwrap()
        .draft
        .body
        .ends_with("过期仍保留"));
}

#[test]
fn manual_range_and_stale_applied_selection_also_refuse_a_different_unapplied_file_buffer() {
    let (ctx, mut app) = manuscript_app();
    let path = app.active_file.clone();
    let source = app.project.document(&path).unwrap().to_owned();
    let mut buffer = app.project.open_source_writing_buffer(&path).unwrap();
    buffer.replace_source(source.replace("甲乙", "尚未应用的版本"));
    app.manuscript.restore_writing_buffers(&[buffer]);
    app.tab = Tab::Edit;
    crate::app::search::record_editor_selection(
        &ctx,
        egui::Id::new("source-test"),
        &path,
        None,
        &source,
        0,
        &source,
        egui::text::CCursorRange::two(egui::text::CCursor::new(0), egui::text::CCursor::new(2)),
    );
    let baseline = app.project.content_baseline();
    app.comment_current_selection(&ctx);
    assert!(app.review.comment_editor.is_none());
    assert!(app.io_error.as_ref().unwrap().contains("未应用"));
    app.io_error = None;
    app.tab = Tab::Review;
    click(&ctx, &mut app, 7, "锚定当前正文范围");
    assert!(app.review.comment_editor.is_none());
    assert!(app.io_error.as_ref().unwrap().contains("未应用"));
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app
        .manuscript
        .writing_buffers()
        .iter()
        .any(|buffer| buffer.path() == path && buffer.source().contains("尚未应用的版本")));
}

#[test]
fn review_keyboard_focus_survives_tail_scroll_and_manual_scroll_does_not_execute_hidden_row() {
    let (ctx, mut app) = app();
    add_notes(&mut app, 37);
    app.tab = Tab::Review;
    app.review.filter.resolution = CommentResolutionFilter::All;
    app.review.selected_comment = Some("revision_036".into());
    app.review.scroll_selection = true;
    let size = vec2(1280.0, 800.0);
    for _ in 0..3 {
        guarded_frame(&ctx, &mut app, vec![], size);
    }
    guarded_click(&ctx, &mut app, "进入批注列表（键盘）");
    // egui 的公开契约要求先在一个完整帧拥有焦点，set_focus_lock_filter 才生效。
    guarded_frame(&ctx, &mut app, vec![], size);
    let list_id = egui::Id::new("review-comment-list");
    assert!(ctx.memory(|memory| memory.has_focus(list_id) && memory.had_focus_last_frame(list_id)));
    let key = |key| Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    guarded_frame(&ctx, &mut app, vec![key(egui::Key::ArrowUp)], size);
    assert_eq!(app.review.selected_comment.as_deref(), Some("revision_035"));
    guarded_frame(&ctx, &mut app, vec![key(egui::Key::ArrowDown)], size);
    assert_eq!(app.review.selected_comment.as_deref(), Some("revision_036"));
    let output = guarded_frame(&ctx, &mut app, vec![], size);
    let point = output
        .shapes
        .iter()
        .find_map(|c| {
            text_position_contains(&c.shape, "revision_036").filter(|p| c.clip_rect.contains(*p))
        })
        .expect("末项可见");
    guarded_frame(
        &ctx,
        &mut app,
        vec![
            Event::PointerMoved(point),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, 10000.0),
                modifiers: egui::Modifiers::NONE,
            },
        ],
        size,
    );
    for _ in 0..24 {
        guarded_frame(&ctx, &mut app, vec![], size);
    }
    let output = guarded_frame(&ctx, &mut app, vec![], size);
    assert!(
        !output
            .shapes
            .iter()
            .any(|c| text_position_contains(&c.shape, "revision_036")
                .is_some_and(|p| c.clip_rect.contains(p))),
        "手动滚动不得每帧被拉回"
    );
    guarded_frame(&ctx, &mut app, vec![key(egui::Key::Enter)], size);
    assert!(
        app.review.comment_editor.is_none(),
        "隐藏条目不能被Enter执行"
    );
    for _ in 0..3 {
        guarded_frame(&ctx, &mut app, vec![], size);
    }
    guarded_frame(&ctx, &mut app, vec![key(egui::Key::Enter)], size);
    assert_eq!(
        app.review.comment_editor.as_ref().unwrap().draft.id,
        "revision_036"
    );
}

#[test]
fn pending_new_anchor_is_rechecked_against_current_source_before_it_can_be_saved() {
    let (ctx, mut app) = app();
    add_notes(&mut app, 1);
    app.edit_comment("revision_000");
    app.review
        .comment_editor
        .as_mut()
        .unwrap()
        .draft
        .body
        .push_str(" 原批注未应用");
    let path = app.active_file.clone();
    let anchor = collaboration::capture_text_anchor(&app.project, &path, 1, 1).unwrap();
    app.new_comment_for_anchor(anchor);
    let updated = app
        .project
        .document(&path)
        .unwrap()
        .replace("同名", "外部修改");
    app.project.set_text(&path, updated).unwrap();
    app.recompile();
    guarded_click(&ctx, &mut app, "明确放弃此批注输入并继续");
    app.review.comment_editor.as_mut().unwrap().draft.body = "新的批注问题".into();
    let baseline = app.project.content_baseline();
    let output = frame(&ctx, &mut app, vec![], 7);
    let mut text = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut text);
    }
    assert!(
        text.contains("失锚"),
        "原文已变不能伪称新锚仍attached: {text}"
    );
    click(&ctx, &mut app, 7, "保存批注");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.review.comment_editor.is_some());
}

#[path = "review_persistence.rs"]
mod persistence;
