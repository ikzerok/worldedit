use super::*;

#[test]
fn object_reading_keyboard_dirty_first_focus_scope_escape_and_input() {
    let (mut h, field_owner) = dirty();
    let before = preserved(&h);
    let opener = open_details(&mut h);
    let close = assert_first(&mut h);
    for modifiers in [Modifiers::SHIFT, Modifiers::CTRL, Modifiers::ALT] {
        tap(&mut h, Key::Escape, modifiers);
        assert_eq!(
            h.app.reading_target,
            Some(TargetRef::new("character", "traveler")),
            "modified Escape is not local Close"
        );
        assert_eq!(focused(&h).id, close);
        assert_eq!(preserved(&h), before);
    }
    for reverse in [false, true] {
        let mut owners = HashSet::new();
        let mut repeated = false;
        for _ in 0..80 {
            let out = tap(
                &mut h,
                Key::Tab,
                if reverse {
                    Modifiers::SHIFT
                } else {
                    Modifiers::NONE
                },
            );
            let r = focused(&h);
            assert_eq!(
                r.layer_id,
                layer(),
                "transient window must not Tab into the editable author form"
            );
            assert_ne!(r.id, field_owner);
            assert_eq!(preserved(&h), before);
            if !owners.insert(r.id) {
                repeated = true;
                break;
            }
            assert!(!out.shapes.is_empty());
        }
        assert!(repeated, "the actual local focus cycle must repeat within the bound; reverse={reverse}, owners={owners:?}");
    }
    // A normal higher-level command dialog has priority over this transient
    // reader. Esc closes only that higher layer and leaves the dirty F/window.
    tap(&mut h, Key::P, Modifiers::COMMAND);
    assert!(h.app.command_palette.open);
    tap(&mut h, Key::Escape, Modifiers::NONE);
    assert!(!h.app.command_palette.open);
    assert_eq!(
        h.app.reading_target,
        Some(TargetRef::new("character", "traveler"))
    );
    assert_eq!(preserved(&h), before);
    seek(&mut h, ALIAS, true, false);
    let alias = focused(&h).id;
    h.frame(vec![Event::Text("harbor".into())]);
    tap(&mut h, Key::ArrowLeft, Modifiers::NONE);
    let cursor = egui::TextEdit::load_state(&h.ctx, alias)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!(
        cursor.primary.index, 5,
        "arrows belong to the actual alias TextEdit"
    );
    h.frame(vec![
        Event::Ime(egui::ImeEvent::Enabled),
        Event::Ime(egui::ImeEvent::Preedit("候选".into())),
    ]);
    h.frame(vec![Event::Ime(egui::ImeEvent::Commit("字".into()))]);
    h.frame(vec![Event::Ime(egui::ImeEvent::Disabled)]);
    assert_eq!(h.app.alias_input, "harbo字r");
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(alias));
    assert_eq!(preserved(&h), before);
    let reader = seek(&mut h, READER, false, false);
    let out = h.frame(vec![]);
    let (prior, domain) = local_reading_content(&h, &out);
    // A continued IME session must disable the reader even when this Arrow
    // frame contains no Ime event. No direct IME/focus state injection.
    h.frame(vec![
        Event::Ime(egui::ImeEvent::Enabled),
        Event::Ime(egui::ImeEvent::Preedit("候选".into())),
    ]);
    assert!(h.app.auxiliary_ime_active(&h.ctx));
    let ime_out = h.frame(vec![Event::Key {
        key: Key::ArrowDown,
        physical_key: Some(Key::ArrowDown),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    assert!(h.app.auxiliary_ime_active(&h.ctx));
    assert!(
        h.ctx
            .input(|input| input.events.iter().any(|event| matches!(
                event,
                Event::Key {
                    key: Key::ArrowDown,
                    pressed: true,
                    ..
                }
            ))),
        "inactive reader must not consume the continued composition's Arrow"
    );
    assert!(texts(&ime_out).iter().any(|(text, rect, clip)| text
        == "按当前工程内容汇总；表单修改应用后会更新此页。"
        && *rect == prior
        && *clip == domain));
    assert_eq!(preserved(&h), before);
    h.frame(vec![Event::Key {
        key: Key::ArrowDown,
        physical_key: Some(Key::ArrowDown),
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    h.frame(vec![Event::Ime(egui::ImeEvent::Disabled)]);
    assert_eq!(seek(&mut h, READER, false, false), reader);
    let out = h.frame(vec![]);
    let (prior, domain) = local_reading_content(&h, &out);
    let mut moved = false;
    for _ in 0..8 {
        let out = tap(&mut h, Key::ArrowDown, Modifiers::NONE);
        assert_eq!(
            focused(&h).id,
            reader,
            "only the named reader owns these arrows"
        );
        moved |= texts(&out).iter().any(|(text, rect, clip)| {
            text == "按当前工程内容汇总；表单修改应用后会更新此页。"
                && *clip == domain
                && clip.intersects(*rect)
                && rect.min.y < prior.min.y
        });
    }
    assert!(
        moved,
        "the exact object-content row must move within the same local paint clip"
    );
    assert_eq!(preserved(&h), before);
    tap(&mut h, Key::Escape, Modifiers::NONE);
    returned(&mut h, opener, &before);
    open_details(&mut h);
    assert_eq!(assert_first(&mut h), close);
    tap(&mut h, Key::Enter, Modifiers::NONE);
    returned(&mut h, opener, &before);
}

#[test]
fn object_reading_keyboard_dirty_edit_keeps_guard_and_restores_author() {
    let (mut h, _) = dirty();
    let before = preserved(&h);
    let opener = open_details(&mut h);
    // Independent of the new first-focus assertion: the old reachable action and
    // original dirty-input refusal are still exercised when run as RED.
    seek(&mut h, "编辑此对象", false, false);
    tap(&mut h, Key::Enter, Modifiers::NONE);
    assert_eq!(h.app.tab, Tab::Manuscript, "after actual EditObject Enter");
    assert!(h.app.character_editor.is_none());
    assert!(h
        .app
        .message
        .as_deref()
        .is_some_and(|s| s.contains("当前还有未应用输入")));
    assert_eq!(preserved(&h), before);
    assert!(
        h.app.reading_target.is_none(),
        "EditObject refusal closes only the transient reader"
    );
    assert!(
        h.ctx.memory(|m| m.focused()).is_some(),
        "after EditObject refusal, before returned-owner assertion"
    );
    returned(&mut h, opener, &before);
}

#[test]
fn object_reading_keyboard_shared_row_link_and_production_entries() {
    for entry in ["speaker", "link", "production"] {
        let mut h =
            Harness::normal("  say traveler \"Look [[character:traveler|traveler link]]\"\n");
        h.toolbar("逐句对白");
        if entry == "production" {
            h.toolbar("角色台本");
            h.click("生成当前稿台本");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
            loop {
                let out = h.frame(vec![]);
                if visible_label(&out, "第 1–1 / 1 行").is_some() {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "real production job must show its exact one-row result: {}",
                    labels(&out)
                );
                std::thread::yield_now();
            }
        }
        let before = preserved(&h);
        let opener = seek(
            &mut h,
            if entry == "link" {
                "traveler link"
            } else {
                "旅人"
            },
            false,
            false,
        );
        tap(&mut h, Key::Enter, Modifiers::NONE);
        assert_eq!(
            h.app.reading_target,
            Some(TargetRef::new("character", "traveler"))
        );
        assert_first(&mut h);
        tap(&mut h, Key::Escape, Modifiers::NONE);
        returned(&mut h, opener, &before);
        if entry == "production" {
            assert!(h.app.manuscript.production.open);
        }
    }
}

#[test]
fn object_reading_keyboard_clean_source_preserves_existing_return_cursor() {
    let mut h = Harness::normal("  say traveler \"Look [[character:traveler|traveler link]]\"\n");
    h.app.tab = Tab::Edit; // Fixture's starting page, not the navigation under test.
    let source = h
        .app
        .project
        .document(&h.app.active_file)
        .unwrap()
        .to_owned();
    let out = h.settle();
    let text = texts(&out)
        .into_iter()
        .find(|(s, _, _)| s == &source)
        .expect("real source TextEdit galley");
    let point = text.1.min + egui::vec2(2.0, 4.0);
    assert!(text.2.contains(point));
    click_point(&mut h, point);
    let source_owner = focused(&h).id;
    assert!(egui::TextEdit::load_state(&h.ctx, source_owner).is_some());
    tap(&mut h, Key::Home, Modifiers::COMMAND);
    let at = source[..source.find("[[character:traveler").unwrap()]
        .chars()
        .count()
        + 5;
    for _ in 0..at {
        tap(&mut h, Key::ArrowRight, Modifiers::NONE);
    }
    let cursor = egui::TextEdit::load_state(&h.ctx, source_owner)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!(cursor.primary.index, at);
    let before = preserved(&h);
    tap(&mut h, Key::Enter, Modifiers::COMMAND);
    assert_eq!(
        h.app.reading_target,
        Some(TargetRef::new("character", "traveler"))
    );
    assert_eq!(h.app.reading_return, Some((h.app.active_file.clone(), at)));
    tap(&mut h, Key::Escape, Modifiers::NONE);
    assert!(h.app.reading_target.is_none() && h.app.reading_return.is_none());
    assert_eq!(h.app.tab, Tab::Edit);
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(source_owner));
    assert_eq!(
        egui::TextEdit::load_state(&h.ctx, source_owner)
            .unwrap()
            .cursor
            .char_range()
            .unwrap(),
        cursor
    );
    assert_eq!(preserved(&h), before);
}

#[test]
fn object_reading_keyboard_create_character_wide_narrow_keeps_typed_form() {
    for width in [1188.0, 900.0] {
        let (mut h, _) = dirty();
        h.size = egui::vec2(width, 848.0);
        h.settle();
        let before = preserved(&h);
        seek(&mut h, "新建人物…", false, true);
        tap(&mut h, Key::Enter, Modifiers::NONE);
        assert_eq!(h.app.tab, Tab::Characters);
        let out = h.frame(vec![]);
        let r = focused(&h);
        assert_eq!(r.id, egui::Id::new("character-name-input"));
        assert!(egui::TextEdit::load_state(&h.ctx, r.id).is_some());
        assert_eq!(visible_owner(&h, &out, "新人物", true), r.id);
        assert_eq!(preserved(&h), before);
        tap(&mut h, Key::ArrowLeft, Modifiers::ALT);
        assert_eq!(h.app.tab, Tab::Manuscript);
        assert_eq!(preserved(&h), before);
    }
}
