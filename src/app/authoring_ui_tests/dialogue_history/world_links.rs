//! H08：真实编辑器选词/关联控件与 typed 对白、译文共用四条历史边。
use super::*;
use worldline_core::localization::{LocalizationCatalogQuery, LocalizationStatus};

fn locale_status(app: &WorldeditApp) -> LocalizationStatus {
    let page = app
        .project
        .query_localization_catalog(&LocalizationCatalogQuery {
            target_locale: Some("fr".into()),
            string_ids: vec!["spoken".into()],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(page.entries.len(), 1);
    page.entries[0].status
}

fn press(ctx: &egui::Context, app: &mut WorldeditApp, key: egui::Key, shift: bool) {
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers: if shift {
                    egui::Modifiers::SHIFT
                } else {
                    egui::Modifiers::NONE
                },
            }],
            13,
        );
    }
}

fn target_is_fully_visible(output: &egui::FullOutput) -> bool {
    fn matches(shape: &egui::Shape, clip: egui::Rect) -> bool {
        match shape {
            egui::Shape::Text(text) => {
                text.galley.text() == "稳定目标 character:traveler"
                    && clip.contains_rect(text.galley.rect.translate(text.pos.to_vec2()))
            }
            egui::Shape::Vec(children) => children.iter().any(|child| matches(child, clip)),
            _ => false,
        }
    }
    output
        .shapes
        .iter()
        .any(|shape| matches(&shape.shape, shape.clip_rect))
}

fn insert_existing_reference(ctx: &egui::Context, app: &mut WorldeditApp) {
    click(ctx, app, 13, "港口");
    click(ctx, app, 13, "源码");
    let source = buffer(app, &app.project.entry).source().to_owned();
    let output = frame(ctx, app, vec![], 13);
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| {
            source_text_position(&shape.shape, &source, "港口旁白")
                .filter(|pos| shape.clip_rect.contains(*pos))
        })
        .expect("scene's current full-source TextEdit must be visible");
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            13,
        );
    }
    // Synthetic pointer/key events select actual source text. No EditorSelection is injected.
    press(ctx, app, egui::Key::End, false);
    for _ in 0.."港口旁白。".chars().count() {
        press(ctx, app, egui::Key::ArrowLeft, false);
    }
    for _ in 0.."港口".chars().count() {
        press(ctx, app, egui::Key::ArrowRight, true);
    }
    let selected = crate::app::search::editor_selection(ctx).unwrap();
    assert_eq!(&selected.source[selected.range], "港口");
    assert_eq!(
        selected.target,
        Some(TargetRef::new("scene", "arrival.harbor"))
    );
    assert_eq!(selected.path, app.project.entry);
    assert_eq!(buffer(app, &app.project.entry).source(), source);
    click(ctx, app, 13, "选词工具");
    click(ctx, app, 13, "关联世界资料…");
    assert!(app.manuscript.world_links_open(), "{:?}", app.io_error);
    for _ in 0..3 {
        frame(ctx, app, vec![], 13);
    }
    let query = egui::Id::new("world-link-query");
    let response = ctx
        .read_response(query)
        .expect("existing reference query TextEdit");
    assert!(response.interact_rect.contains_rect(response.rect));
    let pos = response.rect.center();
    for pressed in [true, false] {
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            13,
        );
    }
    assert_eq!(ctx.memory(|m| m.focused()), Some(query));
    focus_replace(ctx, query, "港口");
    frame(ctx, app, vec![Event::Text("traveler".into())], 13);
    let object = app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .object(&TargetRef::new("character", "traveler"))
        .unwrap();
    let caption = crate::app::object_picker::candidate_caption(object, Some(&app.project.root));
    click_body_output(ctx, app, &caption);
    let rendered = labels(&frame(ctx, app, vec![], 13));
    assert!(rendered.contains("已选择 character:traveler"), "{rendered}");
    click_body_output(ctx, app, "预览关联计划");
    let mut result = frame(ctx, app, vec![], 13);
    #[cfg(not(target_arch = "wasm32"))]
    let unchanged = app.h08_world_link_preview_state();
    let owner = ctx.memory(|memory| memory.focused());
    if !target_is_fully_visible(&result) {
        // ScrollArea commits result-reveal offsets after drawing its children.
        // Honor exactly its requested host paint; never poll until an assertion passes.
        assert_eq!(
            result.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
            std::time::Duration::ZERO
        );
        result = frame(ctx, app, vec![], 13);
    }
    #[cfg(not(target_arch = "wasm32"))]
    assert_eq!(app.h08_world_link_preview_state(), unchanged);
    assert_eq!(ctx.memory(|memory| memory.focused()), owner);
    let rendered = labels(&result);
    assert!(
        rendered.contains("稳定目标 character:traveler"),
        "{rendered}"
    );
    assert!(target_is_fully_visible(&result), "{rendered}");
    click_body_output(ctx, app, "插入引用到正文草稿");
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert!(!app.manuscript.world_links_open());
    assert!(buffer(app, &app.project.entry)
        .source()
        .contains("[[character:traveler|港口]]旁白。"));
    click(ctx, app, 13, "抵达");
    click(ctx, app, 13, "写作");
}

#[test]
fn dialogue_history_typed_reference_locale_apply_travels_four_shared_edges() {
    let (ctx, mut app, _cleanup) = fixture();
    let path = app.project.entry.clone();
    let original = project_files(&app.project);
    let disk_before = disk(&app);
    let clean = buffer(&app, &path);
    let remote = app.project.root.join("remote.wl");
    let mut other = app.project.open_source_writing_buffer(&remote).unwrap();
    other.replace_source(format!("{}// H08 独立未应用稿😀\n", other.source()));
    app.manuscript.restore_writing_buffers(&[other.clone()]);

    stage(&ctx, &mut app, "原来的正式对白", "对白与引用都保留😀"); // T
    let typed = buffer(&app, &path);
    assert_eq!(counts(&app), (0, 0, 1, 0));
    insert_existing_reference(&ctx, &mut app); // W: actual menu, picker, preview and confirmation.
    let linked = buffer(&app, &path);
    assert_eq!(counts(&app), (0, 0, 2, 0));
    assert!(linked.generation() > typed.generation());
    assert_eq!(
        linked.source(),
        typed
            .source()
            .replace("港口旁白。", "[[character:traveler|港口]]旁白。")
    );
    assert_eq!(project_files(&app.project), original);
    assert_eq!(disk(&app), disk_before);
    assert_buffer(&app, &other, true);

    typed_locale(&ctx, &mut app); // L: UI-owned before Project → remember → recompile.
    let localized = project_files(&app.project);
    assert_eq!(counts(&app), (1, 0, 2, 0));
    assert_buffer(&app, &linked, true);
    assert_buffer(&app, &other, true);
    assert_translation(&app);
    assert_eq!(locale_status(&app), LocalizationStatus::Translated);
    apply_body(&ctx, &mut app); // A: visible Apply, no direct Project write.
    let applied = project_files(&app.project);
    assert_eq!(counts(&app), (2, 0, 2, 0));
    assert_eq!(app.project.document(&path).unwrap(), linked.source());
    // L honestly binds the applied source; applying the newer T makes this locale stale.
    assert_eq!(locale_status(&app), LocalizationStatus::StaleSource);
    assert_eq!(disk(&app), disk_before);

    // A, L, W, T. Each operation restores its exact source/generation and consumes one edge.
    for (expected, files, history) in [
        (&linked, &localized, (1, 1, 2, 0)),
        (&linked, &original, (0, 2, 2, 0)),
        (&typed, &original, (0, 2, 1, 1)),
        (&clean, &original, (0, 2, 0, 2)),
    ] {
        app.edit_undo(false);
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        assert_eq!(counts(&app), history);
        assert_eq!(project_files(&app.project), *files);
        assert_buffer(&app, expected, true);
        assert_buffer(&app, &other, true);
        assert_eq!(disk(&app), disk_before);
    }
    assert!(app
        .project
        .authoring_document(&sidecar(&app))
        .unwrap()
        .is_deleted());
    assert!(!sidecar(&app).exists());
    assert!(!app.manuscript.has_dialogue_input());

    // T, W, L, A. No draft-only edge can obscure either Project edge.
    for (expected, files, history) in [
        (&typed, &original, (0, 2, 1, 1)),
        (&linked, &original, (0, 2, 2, 0)),
        (&linked, &localized, (1, 1, 2, 0)),
    ] {
        app.edit_undo(true);
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        assert_eq!(counts(&app), history);
        assert_eq!(project_files(&app.project), *files);
        assert_buffer(&app, expected, true);
        assert_buffer(&app, &other, true);
        assert_eq!(disk(&app), disk_before);
    }
    app.edit_undo(true);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(counts(&app), (2, 0, 2, 0));
    assert_eq!(project_files(&app.project), applied);
    assert!(!app
        .manuscript
        .writing_buffers()
        .iter()
        .any(|b| b.path() == path && b.is_changed()));
    assert_buffer(&app, &other, true);
    assert_translation(&app);
    assert_eq!(locale_status(&app), LocalizationStatus::StaleSource);
    assert_eq!(disk(&app), disk_before);
    save_reopen(&mut app);
    assert_buffer(&app, &other, true);
}
