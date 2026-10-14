use super::*;

#[test]
fn production_keyboard_formats_direction_and_confirmation() {
    let _serial = serial();
    let mut failures = vec![];
    for (format, label) in [
        (ProductionFormat::Json, "精确 JSON"),
        (ProductionFormat::Markdown, "阅读 Markdown"),
        (ProductionFormat::Csv, "表格 CSV"),
    ] {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut h = Keyboard::new(3, false, false);
            let disk = h.flow.disk();
            let baseline = h.flow.app.project.content_baseline();
            h.setup_generated();
            h.setup_preview(label, false);
            let author = h.author_state();
            let snapshot = h.flow.snapshot();
            let bytes = h.flow.artifact();
            assert_eq!(
                bytes,
                snapshot
                    .export(&ProductionExportOptions {
                        schema_version: 1,
                        format,
                        include_direction: false
                    })
                    .unwrap()
                    .bytes()
            );
            let text = String::from_utf8(bytes.clone()).unwrap();
            for forbidden in [PRIVATE, "OTHER_ROLE_SECRET", "UNSELECTED_SECRET"] {
                assert!(!text.replace('\\', "").contains(forbidden));
            }
            if format == ProductionFormat::Json {
                assert!(serde_json::from_slice::<Value>(&bytes).unwrap()["rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|r| r.get("direction").is_none()));
            }
            if format == ProductionFormat::Csv {
                assert!(text.ends_with("\r\n"));
                assert!(bytes.starts_with(b"\"'"));
            }
            // First keyboard target is this independently prepared delivery area,
            // rather than replaying the list test's initial keyboard precondition.
            h.enter(CONFIRM);
            assert!(h.flow.app.manuscript.production.confirmed);
            let out = h.enter("复制相同完整材料");
            assert_eq!(copied(&out).unwrap().as_bytes(), bytes);
            h.enter(CONFIRM);
            assert!(!h.flow.app.manuscript.production.confirmed);
            assert!(h.flow.app.checked_production_artifact(&h.flow.ctx).is_err());
            h.enter(DIRECTION);
            assert!(!h.flow.app.manuscript.production.confirmed);
            assert!(h.flow.app.checked_production_artifact(&h.flow.ctx).is_err());
            h.enter("预览完整交付字节");
            assert_eq!(
                h.flow.artifact(),
                snapshot
                    .export(&ProductionExportOptions {
                        schema_version: 1,
                        format,
                        include_direction: true
                    })
                    .unwrap()
                    .bytes()
            );
            assert!(String::from_utf8(h.flow.artifact())
                .unwrap()
                .replace('\\', "")
                .contains(PRIVATE));
            h.enter(CONFIRM);
            let out = h.enter("复制相同完整材料");
            assert_eq!(copied(&out).unwrap().as_bytes(), h.flow.artifact());
            h.enter(DIRECTION);
            assert!(!h.flow.app.manuscript.production.confirmed);
            assert!(h.flow.app.checked_production_artifact(&h.flow.ctx).is_err());
            h.enter("准备私密交付");
            assert!(!h.flow.app.manuscript.production.export_open);
            assert_eq!(h.flow.snapshot().key(), snapshot.key());
            assert_eq!(h.flow.app.project.content_baseline(), baseline);
            assert_eq!(h.flow.disk(), disk);
            assert_eq!(h.author_state(), author);
        }));
        if let Err(error) = result {
            failures.push(format!("{format:?}: {}", panic_text(error)));
        }
    }
    assert!(
        failures.is_empty(),
        "independent format cells failed: {failures:#?}"
    );
}

#[test]
fn production_keyboard_manual_new_path_and_failure_protection() {
    let _serial = serial();
    let mut h = Keyboard::new(3, false, false);
    let disk = h.flow.disk();
    let baseline = h.flow.app.project.content_baseline();
    h.setup_generated();
    h.setup_preview("精确 JSON", true);
    let author = h.author_state();
    let bytes = h.flow.artifact();
    let key = h.flow.snapshot().key().to_owned();
    let existing = h.flow.directory.join("existing.json");
    fs::write(&existing, b"keep existing bytes").unwrap();
    let internal = h.flow.app.project.root.join("inside.json");
    let wrong = h.flow.directory.join("wrong.csv");
    let missing = h.flow.directory.join("missing/new.json");
    for path in [&existing, &internal, &wrong, &missing] {
        h.tab_field(PATH_HINT);
        h.type_value(path.to_str().unwrap());
        assert_eq!(
            h.flow.app.manuscript.production.destination,
            path.to_str().unwrap()
        );
        let field = h.response.as_ref().unwrap().id;
        h.tab_to("写入新文件");
        let out = h.key(Key::Tab, Modifiers::SHIFT);
        let r = h.response.as_ref().unwrap();
        assert_eq!(r.id, field, "reverse Tab returns to the actual path owner");
        assert!(
            r.interact_rect.contains_rect(r.rect) && text_focus_paint(&h.flow.ctx, &out, r),
            "reverse path focus must remain wholly visible; {}",
            h.diagnostic(&out)
        );
        h.enter("写入新文件");
        assert!(
            !h.flow.notice().starts_with("已写入"),
            "{}",
            h.flow.notice()
        );
        assert_eq!(h.flow.artifact(), bytes);
        assert_eq!(h.flow.snapshot().key(), key);
        assert!(h.flow.app.manuscript.production.confirmed);
    }
    assert_eq!(fs::read(&existing).unwrap(), b"keep existing bytes");
    for path in [&internal, &wrong, &missing] {
        assert!(!path.exists());
    }
    h.tab_field(PATH_HINT);
    h.type_value("relative.json");
    h.enter("写入新文件");
    assert!(!h.flow.notice().starts_with("已写入"));
    let fresh = h.flow.directory.join("new-private.json");
    h.tab_field(PATH_HINT);
    h.type_value(fresh.to_str().unwrap());
    let owner = h.response.as_ref().unwrap().id;
    let cursor = egui::TextEdit::load_state(&h.flow.ctx, owner)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    h.key(Key::ArrowLeft, Modifiers::NONE);
    assert_ne!(
        egui::TextEdit::load_state(&h.flow.ctx, owner)
            .unwrap()
            .cursor
            .char_range()
            .unwrap(),
        cursor
    );
    h.frame(vec![
        Event::Key {
            key: Key::Space,
            physical_key: Some(Key::Space),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        },
        Event::Text(" ".into()),
    ]);
    h.frame(vec![Event::Key {
        key: Key::Space,
        physical_key: Some(Key::Space),
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    assert!(h.flow.app.manuscript.production.destination.contains(' '));
    h.frame(vec![Event::Ime(egui::ImeEvent::Preedit("候".into()))]);
    h.key(Key::ArrowDown, Modifiers::NONE);
    h.frame(vec![Event::Ime(egui::ImeEvent::Commit("字".into()))]);
    assert_eq!(h.flow.ctx.memory(|m| m.focused()), Some(owner));
    assert!(h.flow.app.manuscript.production.destination.contains('字'));
    assert!(!fresh.exists());
    h.type_value(fresh.to_str().unwrap());
    h.enter(CONFIRM);
    assert!(!h.flow.app.manuscript.production.confirmed);
    assert!(!fresh.exists());
    assert!(h.flow.app.checked_production_artifact(&h.flow.ctx).is_err());
    h.enter(CONFIRM);
    h.enter("写入新文件");
    assert_eq!(fs::read(&fresh).unwrap(), bytes);
    h.enter("写入新文件");
    assert!(!h.flow.notice().starts_with("已写入"));
    assert_eq!(fs::read(&fresh).unwrap(), bytes);
    // Actual locale edit invalidates the receipt; no final write is authorized.
    h.tab_field(LOCALE_HINT);
    h.type_value("en");
    assert!(!h.flow.app.production_is_current());
    assert!(h.flow.app.checked_production_artifact(&h.flow.ctx).is_err());
    assert_eq!(fs::read(&fresh).unwrap(), bytes);
    assert_eq!(h.flow.app.project.content_baseline(), baseline);
    assert_eq!(h.flow.disk(), disk);
    assert!(!h.flow.app.project.is_dirty());
    assert_eq!(h.author_state(), author);
}
fn panic_text(error: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = error.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = error.downcast_ref::<&str>() {
        (*s).into()
    } else {
        "non-string panic".into()
    }
}
