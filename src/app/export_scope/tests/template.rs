use super::*;

#[test]
fn clearing_loaded_template_to_empty_still_requires_export_scope_confirmation() {
    let (ctx, mut app) = app();
    assert!(!app
        .unapplied_export_inputs()
        .iter()
        .any(|i| i.kind == "工程模板草稿"));
    let manifest_path = app.project.root.join(".world/project.json");
    let mut manifest: serde_json::Value = serde_json::from_slice(
        app.project
            .authoring_document(&manifest_path)
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["required_features"]
        .as_array_mut()
        .unwrap()
        .push("content.templates.v1".into());
    manifest["templates"] = serde_json::json!({"project:typed":".world/templates/typed.json"});
    app.project
        .set_authoring_document(&manifest_path, serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    let template_path = app.project.root.join(".world/templates/typed.json");
    let original = br#"{"schema_version":1,"id":"project:typed","title":"Typed fields","applies_to":{"kind":"entity","entity_type":"place"},"fields":[{"id":"memo","key":"memo","label":"Memo","type":"text","required":false}]}"#;
    app.project
        .create_authoring_document(&template_path, original.to_vec())
        .unwrap();
    app.recompile();
    let template_frame = |app: &mut WorldeditApp, events| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1500.0, 1200.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| app.template_manager_tab(ctx),
        )
    };
    for _ in 0..3 {
        template_frame(&mut app, vec![]);
    }
    let output = template_frame(&mut app, vec![]);
    let pos = output
        .shapes
        .iter()
        .find_map(|shape| position(&shape.shape, "Typed fields · project:typed"))
        .unwrap();
    for pressed in [true, false] {
        template_frame(
            &mut app,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert!(!app
        .unapplied_export_inputs()
        .iter()
        .any(|i| i.kind == "工程模板草稿"));
    // 0.31选择工程模板先进入可视模式；通过真实按钮进入仍保留的高级JSON。
    let output = template_frame(&mut app, vec![]);
    let json_mode = output
        .shapes
        .iter()
        .find_map(|shape| position(&shape.shape, "高级 JSON"))
        .unwrap();
    for pressed in [true, false] {
        template_frame(
            &mut app,
            vec![
                egui::Event::PointerMoved(json_mode),
                egui::Event::PointerButton {
                    pos: json_mode,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    template_frame(&mut app, vec![]);
    assert_eq!(app.template_manager.editor.as_bytes(), original);
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("template-manager-json")));
    for (key, modifiers) in [
        (egui::Key::A, egui::Modifiers::COMMAND),
        (egui::Key::Backspace, egui::Modifiers::NONE),
    ] {
        template_frame(
            &mut app,
            vec![egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed: true,
                repeat: false,
                modifiers,
            }],
        );
    }
    assert!(
        app.template_manager.editor.is_empty(),
        "必须实际清空高级JSON输入"
    );
    assert!(app
        .unapplied_export_inputs()
        .iter()
        .any(|i| i.kind == "工程模板草稿" && i.source.contains("project:typed")));
    assert_eq!(
        app.project
            .authoring_document(&template_path)
            .unwrap()
            .bytes(),
        original
    );
    let target = destination(&app, false, "empty-template");
    app.request_strict_export(target.clone());
    assert!(app.export_confirmation.is_some());
    assert!(!target.path().exists());
    click(&ctx, &mut app, "取消导出");
    assert!(app
        .unapplied_export_inputs()
        .iter()
        .any(|i| i.kind == "工程模板草稿"));
    let _ = fs::remove_dir_all(app.project.root);
}
