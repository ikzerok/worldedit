use super::*;

fn source_and_reading(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1700.0, 1400.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            app.source_tab(ctx);
            app.reading_window(ctx);
        },
    )
}

fn pointer_click(ctx: &egui::Context, app: &mut WorldeditApp, point: egui::Pos2) {
    for pressed in [true, false] {
        source_and_reading(
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
        );
    }
}

#[test]
fn opening_source_wiki_releases_hidden_input_but_explicit_return_restores_cursor() {
    for keyboard in [false, true] {
        let (ctx, mut app) = app();
        let entry = app.active_file.clone();
        let source = "entity a kind place as \"同名\"\nevent start\n  看见[[entity:a|同名]]，继续\n"
            .to_string()
            + &"// spacing\n".repeat(50) + "// VISIBLE_SOURCE";
        app.project.set_text(&entry, source.clone()).unwrap();
        app.recompile();
        let baseline = app.project.content_baseline();
        let history = app.history.len();
        for _ in 0..4 {
            source_and_reading(&ctx, &mut app, vec![]);
        }
        let output = source_and_reading(&ctx, &mut app, vec![]);
        let link_point = output
            .shapes
            .iter()
            .find_map(|shape| source_text_position(&shape.shape, &source, "[[entity:a|同名]]"))
            .unwrap();
        let id = egui::Id::new(("source", &entry));
        if keyboard {
            let offset = source[..source.find("[[entity:a|").unwrap()]
                .chars()
                .count()
                + 5;
            let mut state = egui::TextEdit::load_state(&ctx, id).unwrap();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(offset),
                )));
            egui::TextEdit::store_state(&ctx, id, state);
            ctx.memory_mut(|memory| memory.request_focus(id));
            source_and_reading(
                &ctx,
                &mut app,
                vec![Event::Key {
                    key: egui::Key::Enter,
                    physical_key: Some(egui::Key::Enter),
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                }],
            );
        } else {
            pointer_click(&ctx, &mut app, link_point);
        }
        assert_eq!(app.reading_target, Some(TargetRef::new("entity", "a")));
        let cursor = app.reading_return.as_ref().unwrap().1;
        source_and_reading(&ctx, &mut app, vec![Event::Text(" @same".into())]);
        assert_eq!(
            app.project.document(&entry).unwrap(),
            source,
            "打开Wiki后不能继续写入被遮挡的源码"
        );
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(app.history.len(), history);
        assert!(!ctx.memory(|memory| memory.has_focus(id)));
        // 旁查仍非模态：明确点击窗口外的正文可以继续创作。
        let output = source_and_reading(&ctx, &mut app, vec![]);
        let visible_source = output
            .shapes
            .iter()
            .find_map(|shape| source_text_position(&shape.shape, &source, "VISIBLE_SOURCE"))
            .unwrap();
        pointer_click(&ctx, &mut app, visible_source);
        assert!(ctx.memory(|memory| memory.has_focus(id)));
        let range = egui::TextEdit::load_state(&ctx, id)
            .unwrap()
            .cursor
            .char_range()
            .unwrap();
        let start = range.primary.index.min(range.secondary.index);
        let end = range.primary.index.max(range.secondary.index);
        let byte_at = |index| {
            source
                .char_indices()
                .nth(index)
                .map_or(source.len(), |(byte, _)| byte)
        };
        source_and_reading(&ctx, &mut app, vec![Event::Text("y".into())]);
        let mut expected = source.clone();
        expected.replace_range(byte_at(start)..byte_at(end), "y");
        assert_eq!(app.project.document(&entry).unwrap(), expected);
        assert!(app.reading_target.is_some());
        let output = source_and_reading(&ctx, &mut app, vec![]);
        let back = visible_text_position(&output, "返回源码编辑").unwrap();
        pointer_click(&ctx, &mut app, back);
        assert!(app.reading_target.is_none());
        assert!(ctx.memory(|memory| memory.has_focus(id)));
        assert_eq!(
            egui::TextEdit::load_state(&ctx, id)
                .unwrap()
                .cursor
                .char_range()
                .unwrap()
                .primary
                .index,
            cursor
        );
        source_and_reading(&ctx, &mut app, vec![Event::Text("x".into())]);
        let byte = source
            .char_indices()
            .nth(cursor)
            .map_or(source.len(), |(byte, _)| byte);
        expected.insert(byte, 'x');
        assert_eq!(app.project.document(&entry).unwrap(), expected);
    }
}

#[test]
fn opening_wiki_keeps_pending_composition_draft_and_its_source_baseline() {
    let (ctx, mut app) = app();
    let entry = app.active_file.clone();
    let source =
        "entity a kind place as \"同名\"\nevent start\n  看见[[entity:a|同名]]".to_string();
    app.project.set_text(&entry, source.clone()).unwrap();
    app.recompile();
    // 合成一个已由源码输入建立的未提交组合草稿；不冒充物理 IME 验收。
    let draft = source.clone() + "待提交中文";
    app.ime_source_baseline = Some((entry.clone(), source.clone()));
    app.ime_source_draft = Some((entry.clone(), draft.clone(), source.clone()));
    app.ime_composing = true;
    let baseline = app.project.content_baseline();
    let history = app.history.len();
    for _ in 0..4 {
        source_and_reading(&ctx, &mut app, vec![]);
    }
    let output = source_and_reading(&ctx, &mut app, vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| source_text_position(&shape.shape, &draft, "[[entity:a|同名]]"))
        .unwrap();
    pointer_click(&ctx, &mut app, point);
    assert!(app.reading_target.is_some());
    source_and_reading(
        &ctx,
        &mut app,
        vec![
            Event::Text("unfocused".into()),
            Event::Ime(egui::ImeEvent::Disabled),
        ],
    );
    assert_eq!(app.project.document(&entry).unwrap(), source);
    assert_eq!(
        app.ime_source_draft,
        Some((entry.clone(), draft, source.clone()))
    );
    // Disabled结束活动组合态；恢复用基线仍保存在上面已核对的draft三元组中。
    assert!(app.ime_source_baseline.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.history.len(), history);
    assert!(app.has_open_authoring_form());
    let output = source_and_reading(&ctx, &mut app, vec![]);
    let back = visible_text_position(&output, "返回源码编辑").unwrap();
    pointer_click(&ctx, &mut app, back);
    assert!(ctx.memory(|memory| memory.has_focus(egui::Id::new(("source", &entry)))));
    assert!(app.ime_source_draft.is_some());
}
