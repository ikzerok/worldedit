use super::*;

#[test]
fn newer_search_layer_owns_escape_before_association_window() {
    let (ctx, mut app) = fixture();
    app.begin_manuscript_world_links(&ctx);
    app.sync_edit_layers(&ctx);
    assert!(app.edit_layer_is_top("world-links"));
    app.search_open = true;
    app.sync_edit_layers(&ctx);
    assert!(app.edit_layer_is_top("search"));
    let _ = ctx.run(
        RawInput {
            events: vec![key(egui::Key::Escape)],
            ..Default::default()
        },
        |ctx| {
            app.edit_shortcuts(ctx);
        },
    );
    assert!(!app.search_open);
    assert!(app.manuscript.world_links.as_ref().unwrap().open);
    app.sync_edit_layers(&ctx);
    let _ = ctx.run(
        RawInput {
            events: vec![key(egui::Key::Escape)],
            ..Default::default()
        },
        |ctx| {
            app.edit_shortcuts(ctx);
        },
    );
    assert!(!app.manuscript.world_links.as_ref().unwrap().open);
}

#[test]
fn association_state_does_not_capture_escape_when_another_tab_is_active() {
    let (ctx, mut app) = fixture();
    app.begin_manuscript_world_links(&ctx);
    app.tab = Tab::Edit;
    app.sync_edit_layers(&ctx);
    assert!(!app.edit_layer_is_top("world-links"));
    let _ = ctx.run(
        RawInput {
            events: vec![key(egui::Key::Escape)],
            ..Default::default()
        },
        |ctx| {
            assert!(!app.close_manuscript_world_links_on_escape(ctx));
        },
    );
    assert!(app.manuscript.world_links.as_ref().unwrap().open);
    assert_eq!(app.tab, Tab::Edit);
}

#[test]
fn later_draft_in_affected_file_blocks_project_undo_without_overwriting_input() {
    let (ctx, mut app) = fixture();
    app.begin_manuscript_world_links(&ctx);
    new_character(&mut app);
    let mut state = app.manuscript.world_links.take().unwrap();
    app.preview_manuscript_world_link(&mut state);
    assert!(app.apply_manuscript_world_link(&ctx, &mut state));
    let baseline = app.project.content_baseline();
    let path = app.project.entry.clone();
    let buffer = app.manuscript.writing_buffers.get_mut(&path).unwrap();
    let keep = format!("{}\n// 不能覆盖的新输入\n", buffer.source());
    buffer.replace_source(keep.clone());
    app.edit_undo(false);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.manuscript.writing_buffers[&path].source(), keep);
    assert!(app.io_error.as_ref().unwrap().contains("不能覆盖"));
}

#[test]
fn narrow_scaled_frames_keep_return_and_primary_preview_controls_inside_clip() {
    let (ctx, mut app) = fixture();
    app.begin_manuscript_world_links(&ctx);
    for (size, scale) in [(vec2(540.0, 440.0), 1.0), (vec2(480.0, 380.0), 2.0)] {
        ctx.set_pixels_per_point(scale);
        let mut output = frame_size(&ctx, &mut app, size, vec![]);
        for _ in 0..3 {
            output = frame_size(&ctx, &mut app, size, vec![]);
        }
        for expected in ["返回正文，保留输入", "预览关联计划"] {
            let visible = output.shapes.iter().any(|shape| {
                let mut text = Vec::new();
                text_shapes(&shape.shape, &mut text);
                text.into_iter().any(|text| {
                    text.galley.text() == expected
                        && shape
                            .clip_rect
                            .contains(text.pos + text.galley.rect.center().to_vec2())
                })
            });
            assert!(
                visible,
                "{size:?} scale={scale} missing {expected}: {}",
                labels(&output)
            );
        }
    }
}
