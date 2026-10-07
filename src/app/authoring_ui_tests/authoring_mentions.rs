use super::*;

#[test]
fn source_mention_lists_ambiguous_targets_and_explicit_choice_inserts_one_stable_link() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let source = "entity a kind place as \"同名\"\nentity b kind organization as \"同名\"\nevent start\n  开始：";
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;

    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(source.chars().count()),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let _ = frame(&ctx, &mut app, vec![Event::Text("@同名".into())], 11);

    let output = frame(&ctx, &mut app, Vec::new(), 11);
    assert!(
        output
            .shapes
            .iter()
            .any(|shape| text_position(&shape.shape, "同名 · 实体:a\nworld.wl:1").is_some()),
        "候选必须显示 kind 和 ID，避免同名时静默选择"
    );
    assert!(output.shapes.iter().any(|shape| text_position(
        &shape.shape,
        "同名 · 实体:b\nworld.wl:2"
    )
    .is_some()));

    let before_commit = app.project.content_baseline();
    let history_before = app.history.len();
    click(&ctx, &mut app, 11, "同名 · 实体:b\nworld.wl:2");
    assert!(
        app.project
            .document(&entry)
            .unwrap()
            .contains("[[entity:b|同名]]"),
        "source={:?}, error={:?}",
        app.project.document(&entry).unwrap(),
        app.io_error
    );
    let links = &app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .text_links;
    assert!(links.iter().any(|link| {
        link.target == worldline_core::TargetRef::new("entity", "b") && link.label == "同名"
    }));
    assert_eq!(app.history.len(), history_before + 1);
    app.undo(false);
    assert_eq!(app.project.content_baseline(), before_commit);
}

#[test]
fn source_mention_candidates_can_be_selected_and_committed_without_a_mouse() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let source = "entity a kind place as \"同名\"\nentity b kind organization as \"同名\"\nevent start\n  开始：";
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;

    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(source.chars().count()),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let _ = frame(&ctx, &mut app, vec![Event::Text("@同名".into())], 11);
    let candidates = frame(&ctx, &mut app, Vec::new(), 11);
    assert!(candidates.shapes.iter().any(|shape| text_position(
        &shape.shape,
        "同名 · 实体:a\nworld.wl:1"
    )
    .is_some()));

    let key = |key| Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = frame(&ctx, &mut app, vec![key(egui::Key::ArrowDown)], 11);
    let _ = frame(&ctx, &mut app, vec![key(egui::Key::Enter)], 11);

    assert!(app
        .project
        .document(&entry)
        .unwrap()
        .contains("[[entity:b|同名]]"));
    assert_eq!(app.history.len(), 2);
}

#[test]
fn source_mention_keyboard_commit_accepts_web_rooted_active_source() {
    let (ctx, mut app) = app();
    let original_entry = app.project.entry.clone();
    let original_root = app.project.root.clone();
    let source = "entity a kind place as \"同名\"\nentity b kind organization as \"同名\"\nevent start\n  开始：";
    app.project
        .set_text(&original_entry, source.into())
        .unwrap();
    app.recompile();

    let root = std::path::PathBuf::from("/world").join(format!(
        "worldedit-web-source-{}-{}",
        std::process::id(),
        NEXT_TEST_ROOT.fetch_add(1, Ordering::Relaxed)
    ));
    let entry = root.join("world.wl");
    let relocate = |path: std::path::PathBuf| match path.strip_prefix(&original_root) {
        Ok(relative) => {
            let path = root.join(relative);
            std::path::absolute(&path).unwrap_or(path)
        }
        Err(_) => path,
    };
    app.project.documents = std::mem::take(&mut app.project.documents)
        .into_iter()
        .map(|(path, document)| (relocate(path), document))
        .collect();
    app.project.authoring_documents = std::mem::take(&mut app.project.authoring_documents)
        .into_iter()
        .map(|(path, document)| (relocate(path), document))
        .collect();
    app.project.root = root.clone();
    app.project.entry = entry.clone();
    app.active_file = entry.clone();
    app.tab = super::Tab::Edit;
    app.recompile();

    let _ = frame(&ctx, &mut app, Vec::new(), 11);
    let editor = egui::Id::new(("source", &app.active_file));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(source.chars().count()),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let _ = frame(&ctx, &mut app, vec![Event::Text("@同名".into())], 11);

    let key = |key| Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = frame(&ctx, &mut app, vec![key(egui::Key::ArrowDown)], 11);
    let _ = frame(&ctx, &mut app, vec![key(egui::Key::Enter)], 11);

    assert!(
        app.project
            .document(&entry)
            .unwrap()
            .contains("[[entity:b|同名]]"),
        "source={:?}, error={:?}",
        app.project.document(&entry).unwrap(),
        app.io_error
    );
    assert!(app.io_error.is_none(), "error={:?}", app.io_error);
}

#[test]
fn source_mention_esc_hides_candidates_without_changing_source() {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let source = "entity a kind place as \"同名\"\nevent start\n  开始：";
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;
    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(source.chars().count()),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let _ = frame(&ctx, &mut app, vec![Event::Text("@同名".into())], 11);
    let shown = frame(&ctx, &mut app, Vec::new(), 11);
    assert!(shown
        .shapes
        .iter()
        .any(|shape| { text_position(&shape.shape, "同名 · 实体:a\nworld.wl:1").is_some() }));

    let baseline = app.project.content_baseline();
    let output = frame(
        &ctx,
        &mut app,
        vec![Event::Key {
            key: egui::Key::Escape,
            physical_key: Some(egui::Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
        11,
    );
    assert!(!output
        .shapes
        .iter()
        .any(|shape| { text_position(&shape.shape, "同名 · 实体:a\nworld.wl:1").is_some() }));
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.project.document(&entry).unwrap().contains("@同名"));
}

#[test]
fn source_mention_waits_for_ime_commit_before_showing_candidates() {
    source_mention_ime_commit(false);
}

#[test]
fn source_mention_commit_navigation_keys_do_not_insert_cancel_or_add_newlines() {
    source_mention_ime_commit(true);
}

fn source_mention_ime_commit(with_navigation_keys: bool) {
    let (ctx, mut app) = app();
    let entry = app.project.entry.clone();
    let source = "entity a kind place as \"同名\"\nevent start\n  开始：";
    app.project.set_text(&entry, source.into()).unwrap();
    app.recompile();
    app.tab = super::Tab::Edit;
    let editor = egui::Id::new(("source", &entry));
    let mut state = egui::TextEdit::load_state(&ctx, editor).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(source.chars().count()),
        )));
    egui::TextEdit::store_state(&ctx, editor, state);
    ctx.memory_mut(|memory| memory.request_focus(editor));
    let _ = frame(
        &ctx,
        &mut app,
        vec![
            Event::Ime(egui::ImeEvent::Enabled),
            Event::Ime(egui::ImeEvent::Preedit("@同名".into())),
        ],
        11,
    );
    let preedit = frame(&ctx, &mut app, Vec::new(), 11);
    assert!(!preedit
        .shapes
        .iter()
        .any(|shape| { text_position(&shape.shape, "同名 · 实体:a\nworld.wl:1").is_some() }));
    assert!(preedit.shapes.iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text.contains("@同名"))
    }));
    assert_eq!(app.project.document(&entry).unwrap(), source);

    let mut commit = vec![Event::Ime(egui::ImeEvent::Commit("@同名".into()))];
    if with_navigation_keys {
        commit.extend(
            [
                egui::Key::Enter,
                egui::Key::Escape,
                egui::Key::ArrowDown,
                egui::Key::PageDown,
            ]
            .into_iter()
            .map(|key| Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }),
        );
    }
    let _ = frame(&ctx, &mut app, commit, 11);
    let committed = frame(&ctx, &mut app, Vec::new(), 11);
    assert!(
        committed.shapes.iter().any(|shape| {
            text_position(&shape.shape, "同名 · 实体:a\nworld.wl:1").is_some()
        }),
        "source={:?}, focus={:?}, composing={:?}, suppressed={:?}",
        app.project.document(&entry),
        ctx.memory(|m| m.focused()),
        app.ime_composing,
        app.mention_suppression
    );
    assert_eq!(app.history.len(), 1);
    assert_eq!(
        app.project.document(&entry).unwrap(),
        format!("{source}@同名")
    );
    assert!(app.mention_suppression.is_none());
}
