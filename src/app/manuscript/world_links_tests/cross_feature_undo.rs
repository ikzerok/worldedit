//! C 与首次 locale 文档创建共用历史时，撤销仍须找回被纳入的完整作者草稿。
use super::*;
use crate::app::localization_ui;
use worldline_core::localization::LocalizationSelection;

pub(super) fn localization_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> (bool, egui::FullOutput) {
    let mut applied = false;
    let output = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1700.0, 1400.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                applied = localization_ui::show(
                    ui,
                    &mut app.project,
                    &mut app.localization_ui,
                    app.version,
                );
                // 与 Tab::Localization 的宿主一致：使用 UI 返回的真实事务前快照。
                if applied {
                    let before = app.localization_ui.take_applied_before().unwrap();
                    app.remember(before);
                    app.recompile();
                }
            });
        },
    );
    (applied, output)
}

pub(super) fn click_localization(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) -> bool {
    let mut found = None;
    let mut rendered = String::new();
    for _ in 0..30 {
        let (_, output) = localization_frame(ctx, app, Vec::new());
        rendered = labels(&output);
        found = output.shapes.iter().find_map(|shape| {
            let mut texts = Vec::new();
            text_shapes(&shape.shape, &mut texts);
            texts
                .into_iter()
                .filter(|text| text.galley.text() == label)
                .map(|text| text.pos + text.galley.rect.center().to_vec2())
                .find(|point| shape.clip_rect.contains(*point))
        });
        if found.is_some() {
            break;
        }
        localization_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos2(850.0, 700.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -450.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    let point = found.unwrap_or_else(|| panic!("未显示控件 {label}：{rendered}"));
    let mut applied = false;
    for pressed in [true, false] {
        applied |= localization_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        )
        .0;
    }
    app.localization_ui
        .settle_pending_for_test(&app.project, app.version);
    applied
}

#[test]
fn c_then_first_locale_apply_two_undos_restore_original_full_drafts() {
    let (ctx, mut app) = fixture();
    let root = app.project.root.clone();
    let path = app.project.entry.clone();
    let destination = root.join("资料.wl");
    let manifest = root.join(".world/project.json");
    // 准备可翻译的已有作品；历史在真实 C 操作前为空。
    let source = TEXT.replace("走向灯塔。", "走向灯塔。 #wl-localization:body");
    app.project.set_text(&path, source.clone()).unwrap();
    let mut manifest_value: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&manifest).unwrap().bytes()).unwrap();
    manifest_value["required_features"] =
        serde_json::json!(["presentation.manuscripts.v1", "content.localization.v1"]);
    app.project
        .set_authoring_document(&manifest, serde_json::to_vec(&manifest_value).unwrap())
        .unwrap();
    app.project.save().unwrap();
    app.reset_views();
    app.recompile();
    app.tab = Tab::Manuscript;
    for _ in 0..3 {
        frame(&ctx, &mut app, Vec::new());
    }
    let disk_source = fs::read(&path).unwrap();
    let disk_destination = fs::read(&destination).unwrap();
    let disk_manifest = fs::read(&manifest).unwrap();
    app.manuscript
        .writing_buffers
        .get_mut(&path)
        .unwrap()
        .replace_source(source.replace("走向", "慢慢走向"));
    let mut destination_draft = app
        .project
        .open_source_writing_buffer(&destination)
        .unwrap();
    destination_draft.replace_source("// 资料文件原本尚未应用的整稿输入😀\n".into());
    app.manuscript
        .restore_writing_buffers(std::slice::from_ref(&destination_draft));
    frame(&ctx, &mut app, Vec::new());
    select_name(&ctx, &mut app);
    let source_draft = app.manuscript.writing_buffers[&path].clone();
    assert!(source_draft.is_changed() && destination_draft.is_changed());
    assert!(app.history.is_empty());

    app.begin_manuscript_world_links(&ctx);
    new_character(&mut app);
    let mut state = app.manuscript.world_links.take().unwrap();
    app.preview_manuscript_world_link(&mut state);
    let plan = state
        .plan
        .as_ref()
        .unwrap_or_else(|| panic!("{:?}", state.error));
    assert_eq!(plan.included_buffers.len(), 2);
    assert!(plan.included_buffers.iter().all(|buffer| buffer.changed));
    assert!(app.apply_manuscript_world_link(&ctx, &mut state));
    assert_eq!(app.history.len(), 1);
    let compound_sources = app.project.sources();
    assert!(app.project.document(&path).unwrap().contains("慢慢走向"));
    assert!(app
        .project
        .document(&destination)
        .unwrap()
        .ends_with(destination_draft.source()));

    // 使用真实 A1 高级交换表单的预览、确认、内存 Apply 和宿主 history 调用链。
    let selection = LocalizationSelection {
        schema_version: 1,
        source_locale: "en".into(),
        target_locale: "zh-Hant".into(),
        string_ids: vec!["body".into()],
    };
    let export = app.project.preview_localization_export(&selection).unwrap();
    assert!(export.can_export, "{:?}", export.diagnostics);
    let mut exchange = export.exchange;
    for entry in &mut exchange.entries {
        entry.translation_parts = Some(entry.source_parts.clone());
    }
    app.tab = Tab::Localization;
    app.localization_ui.advanced = true;
    app.localization_ui.source_locale = selection.source_locale;
    app.localization_ui.target_locale = selection.target_locale;
    app.localization_ui.string_ids = "body".into();
    app.localization_ui.exchange_json = serde_json::to_string(&exchange).unwrap();
    assert!(!click_localization(&ctx, &mut app, "预览导入"));
    assert!(app.localization_ui.import_plan.as_ref().unwrap().can_apply);
    assert!(!click_localization(&ctx, &mut app, "复核通过 · 确认导入…"));
    assert!(click_localization(&ctx, &mut app, "确认并原子导入"));
    assert_eq!(app.history.len(), 2);
    let sidecar = root.join(".world/localization/zh-Hant.json");
    assert!(!app
        .project
        .authoring_document(&sidecar)
        .unwrap()
        .is_deleted());
    assert!(!sidecar.exists());

    app.edit_undo(false);
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.project.sources(), compound_sources);
    assert!(app
        .project
        .authoring_document(&sidecar)
        .unwrap()
        .is_deleted());
    app.edit_undo(false);
    assert!(app.history.is_empty());
    assert_eq!(app.project.document(&path).unwrap().as_bytes(), disk_source);
    assert_eq!(
        app.project.document(&destination).unwrap().as_bytes(),
        disk_destination
    );

    for expected in [&source_draft, &destination_draft] {
        let restored = app
            .manuscript
            .writing_buffers
            .get(expected.path())
            .unwrap_or_else(|| {
                panic!(
                    "撤销 A1 后再撤销 C 丢失了原未应用全文草稿：{}",
                    expected.path().display()
                )
            });
        assert_eq!(restored.source(), expected.source());
        assert_eq!(restored.generation(), expected.generation());
        assert!(restored.is_changed());
        assert_eq!(restored.baseline(), app.project.content_baseline());
        // 公共 core 守卫会逐字节核对 original 与当前已应用文件；仅检查副本。
        let mut original_probe = restored.clone();
        original_probe
            .rebase_unchanged_source(&app.project)
            .expect("恢复草稿的 original 必须仍是已验证的事务前完整原文");
    }
    for _ in 0..2 {
        app.edit_undo(true);
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        assert_eq!(app.project.sources(), compound_sources);
        app.edit_undo(true);
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        assert!(!app
            .project
            .authoring_document(&sidecar)
            .unwrap()
            .is_deleted());
        app.edit_undo(false);
        app.edit_undo(false);
        for expected in [&source_draft, &destination_draft] {
            let restored = &app.manuscript.writing_buffers[expected.path()];
            assert_eq!(restored.source(), expected.source());
            assert_eq!(restored.generation(), expected.generation());
            assert_eq!(restored.baseline(), app.project.content_baseline());
        }
    }
    assert_eq!(fs::read(&path).unwrap(), disk_source);
    assert_eq!(fs::read(&destination).unwrap(), disk_destination);
    assert_eq!(fs::read(&manifest).unwrap(), disk_manifest);
    assert!(!sidecar.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn existing_reference_then_first_locale_undo_restores_draft_without_false_conflict() {
    let (ctx, mut app) = fixture();
    let root = app.project.root.clone();
    let path = app.project.entry.clone();
    let manifest = root.join(".world/project.json");
    let source = TEXT.replace("走向灯塔。", "走向灯塔。 #wl-localization:body");
    app.project.set_text(&path, source.clone()).unwrap();
    let mut manifest_value: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&manifest).unwrap().bytes()).unwrap();
    manifest_value["required_features"] =
        serde_json::json!(["presentation.manuscripts.v1", "content.localization.v1"]);
    app.project
        .set_authoring_document(&manifest, serde_json::to_vec(&manifest_value).unwrap())
        .unwrap();
    app.project.save().unwrap();
    app.reset_views();
    app.recompile();
    app.tab = Tab::Manuscript;
    for _ in 0..3 {
        frame(&ctx, &mut app, Vec::new());
    }
    app.manuscript
        .writing_buffers
        .get_mut(&path)
        .unwrap()
        .replace_source(source.replace("走向", "慢慢走向"));
    frame(&ctx, &mut app, Vec::new());
    select_name(&ctx, &mut app);
    let original_draft = app.manuscript.writing_buffers[&path].clone();
    let applied_sources = app.project.sources();
    assert!(original_draft.is_changed());
    app.begin_manuscript_world_links(&ctx);
    let mut state = app.manuscript.world_links.take().unwrap();
    state.chosen = Some(TargetRef::new("character", "lin"));
    app.preview_manuscript_world_link(&mut state);
    assert!(app.apply_manuscript_world_link(&ctx, &mut state));
    let linked_draft = app.manuscript.writing_buffers[&path].clone();
    assert!(linked_draft.source().contains("[[character:lin|林芜😀]]"));
    assert_eq!(app.project.sources(), applied_sources);
    assert!(app.history.is_empty());

    let selection = LocalizationSelection {
        schema_version: 1,
        source_locale: "en".into(),
        target_locale: "zh-Hant".into(),
        string_ids: vec!["body".into()],
    };
    let export = app.project.preview_localization_export(&selection).unwrap();
    assert!(export.can_export, "{:?}", export.diagnostics);
    let mut exchange = export.exchange;
    for entry in &mut exchange.entries {
        entry.translation_parts = Some(entry.source_parts.clone());
    }
    app.tab = Tab::Localization;
    app.localization_ui.advanced = true;
    app.localization_ui.source_locale = selection.source_locale;
    app.localization_ui.target_locale = selection.target_locale;
    app.localization_ui.string_ids = "body".into();
    app.localization_ui.exchange_json = serde_json::to_string(&exchange).unwrap();
    assert!(!click_localization(&ctx, &mut app, "预览导入"));
    assert!(app.localization_ui.import_plan.as_ref().unwrap().can_apply);
    assert!(!click_localization(&ctx, &mut app, "复核通过 · 确认导入…"));
    assert!(click_localization(&ctx, &mut app, "确认并原子导入"));
    assert_eq!(app.history.len(), 1);
    let sidecar = root.join(".world/localization/zh-Hant.json");
    assert!(!app
        .project
        .authoring_document(&sidecar)
        .unwrap()
        .is_deleted());
    assert!(!sidecar.exists());
    app.edit_undo(false);
    assert!(app.history.is_empty());
    assert!(app
        .project
        .authoring_document(&sidecar)
        .unwrap()
        .is_deleted());
    assert_eq!(
        app.manuscript.writing_buffers[&path].source(),
        linked_draft.source()
    );
    assert_eq!(app.project.sources(), applied_sources);

    app.edit_undo(false);
    assert!(
        app.io_error.is_none(),
        "无后续正文输入不应发生撤销冲突：{:?}",
        app.io_error
    );
    let restored = &app.manuscript.writing_buffers[&path];
    assert_eq!(restored.source(), original_draft.source());
    assert_eq!(restored.generation(), original_draft.generation());
    assert!(restored.is_changed());
    assert_eq!(restored.baseline(), app.project.content_baseline());
    let mut original_probe = restored.clone();
    original_probe
        .rebase_unchanged_source(&app.project)
        .expect("引用撤销后 original 必须仍是已验证的已应用完整原文");
    assert_eq!(app.project.sources(), applied_sources);
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    assert!(!sidecar.exists());
    fs::remove_dir_all(root).unwrap();
}
