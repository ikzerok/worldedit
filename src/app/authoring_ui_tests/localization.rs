use super::*;
use crate::app::localization_ui;
use std::fs;
use worldline_core::localization::{LocalizationExchange, LocalizationPart, LocalizationSelection};

const SOURCE: &str = concat!(
    "let traveler = \"Ari\"\n",
    "event arrival\n",
    "  Welcome, {traveler}! #wl-localization:welcome\n",
    "  choice \"Continue {traveler}\" if true #wl-localization:reply\n",
    "    -> END\n",
    "  -> END\n",
);

fn localization_app() -> (egui::Context, WorldeditApp, std::path::PathBuf) {
    let (ctx, mut app) = app();
    let root = app.project.root.clone();
    let entry = app.project.entry.clone();
    app.project.set_text(&entry, SOURCE.into()).unwrap();
    let manifest_path = root.join(".world/project.json");
    let mut manifest: serde_json::Value = serde_json::from_slice(
        app.project
            .authoring_document(&manifest_path)
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["required_features"] = serde_json::json!([
        "content.entities.v1",
        "content.relations.v1",
        "content.localization.v1"
    ]);
    app.project
        .set_authoring_document(&manifest_path, serde_json::to_vec(&manifest).unwrap())
        .unwrap();
    app.project.save().unwrap();
    app.recompile();
    app.localization_ui.source_locale = "en".into();
    app.localization_ui.target_locale = "zh-Hant".into();
    app.localization_ui.string_ids = "welcome\nreply".into();
    assert!(app.localization_ui.has_unsubmitted_work());
    (ctx, app, root)
}

fn selection(ids: &[&str]) -> LocalizationSelection {
    LocalizationSelection {
        schema_version: 1,
        source_locale: "en".into(),
        target_locale: "zh-Hant".into(),
        string_ids: ids.iter().map(|id| (*id).into()).collect(),
    }
}

fn panel_frame(
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
                applied = localization_ui::show(ui, &mut app.project, &mut app.localization_ui);
            });
        },
    );
    (applied, output)
}

fn click_panel(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) -> bool {
    let mut found = None;
    let mut rendered = String::new();
    for _ in 0..30 {
        let (_, output) = panel_frame(ctx, app, Vec::new());
        rendered = text(&output);
        if let Some(point) = output
            .shapes
            .iter()
            .find_map(|shape| super::text_position(&shape.shape, label))
        {
            found = Some(point);
            break;
        }
        let _ = panel_frame(
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
        applied = panel_frame(
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
        )
        .0;
    }
    applied
}

fn text(output: &egui::FullOutput) -> String {
    let mut rendered = String::new();
    for shape in &output.shapes {
        super::collect_text(&shape.shape, &mut rendered);
    }
    rendered
}

#[test]
fn panel_previews_and_core_exports_file_then_reviews_cancel_and_confirms_import() {
    let (ctx, mut app, root) = localization_app();
    click_panel(&ctx, &mut app, "预览导出");
    let preview = app.localization_ui.export_plan.as_ref().unwrap();
    assert!(preview.can_export, "{:?}", preview.diagnostics);
    let plan_digest = preview.plan_digest.clone();
    let (_, output) = panel_frame(&ctx, &mut app, Vec::new());
    let rendered = text(&output);
    assert!(rendered.contains("welcome"), "{rendered}");
    assert!(rendered.contains(".wl:3"), "{rendered}");

    let destination = root.with_extension("localization-export.json");
    app.project
        .export_localization(
            &selection(&["welcome", "reply"]),
            &plan_digest,
            &destination,
        )
        .unwrap();
    let bytes = fs::read(&destination).unwrap();
    let mut exchange = LocalizationExchange::from_json_bytes(&bytes).unwrap();
    assert_eq!(exchange.string_ids, ["reply", "welcome"]);
    let welcome = exchange
        .entries
        .iter()
        .find(|entry| entry.id == "welcome")
        .unwrap();
    assert_eq!(welcome.source.line, 3);
    assert!(welcome.source.file.ends_with(".wl"));
    for entry in &mut exchange.entries {
        entry.translation_parts = Some(entry.source_parts.clone());
    }

    app.localization_ui.exchange_json = serde_json::to_string(&exchange).unwrap();
    let baseline = app.project.content_baseline();
    click_panel(&ctx, &mut app, "预览导入");
    assert!(app.localization_ui.import_plan.as_ref().unwrap().can_apply);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.localization_ui.has_unsubmitted_work());
    app.localization_ui
        .set_import_failure("替换文件不是 UTF-8".into());
    assert!(app.localization_ui.import_plan.is_none());
    assert_eq!(
        app.localization_ui.exchange_json,
        serde_json::to_string(&exchange).unwrap()
    );
    assert_eq!(app.project.content_baseline(), baseline);
    click_panel(&ctx, &mut app, "预览导入");
    assert!(app.localization_ui.import_plan.as_ref().unwrap().can_apply);

    click_panel(&ctx, &mut app, "复核通过 · 确认导入…");
    click_panel(&ctx, &mut app, "取消导入");
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.localization_ui.has_unsubmitted_work());
    assert_eq!(
        app.localization_ui.exchange_json,
        serde_json::to_string(&exchange).unwrap()
    );

    click_panel(&ctx, &mut app, "复核通过 · 确认导入…");
    assert!(click_panel(&ctx, &mut app, "确认并原子导入"));
    assert_ne!(app.project.content_baseline(), baseline);
    assert!(!app.localization_ui.has_unsubmitted_work());
    assert_eq!(
        app.localization_ui.exchange_json,
        serde_json::to_string(&exchange).unwrap()
    );
    let sidecar = fs::read(root.join(".world/localization/zh-Hant.json")).unwrap();
    let persisted: serde_json::Value = serde_json::from_slice(&sidecar).unwrap();
    assert!(persisted["entries"]["welcome"]["translation_parts"].is_array());

    drop(app);
    fs::remove_file(destination).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn panel_shows_core_missing_stale_and_placeholder_diagnostics_without_writing() {
    let (ctx, mut app, root) = localization_app();
    let original = app
        .project
        .preview_localization_export(&selection(&["welcome"]))
        .unwrap()
        .exchange;

    app.localization_ui.string_ids = "welcome".into();
    app.localization_ui.exchange_json = serde_json::to_string(&original).unwrap();
    click_panel(&ctx, &mut app, "预览导入");
    let plan = app.localization_ui.import_plan.as_ref().unwrap();
    assert!(!plan.can_apply);
    assert!(plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "MISSING_TRANSLATION"));
    let (_, output) = panel_frame(&ctx, &mut app, Vec::new());
    assert!(text(&output).contains("MISSING_TRANSLATION"));

    let mut invalid = original;
    invalid.entries[0].translation_parts = Some(vec![LocalizationPart::Text {
        text: "你好 {not-the-protected-token}".into(),
    }]);
    let changed = SOURCE.replace("Welcome", "Greetings");
    app.project
        .set_text(&app.project.entry.clone(), changed)
        .unwrap();
    app.project.save().unwrap();
    app.recompile();
    let baseline = app.project.content_baseline();
    app.localization_ui.exchange_json = serde_json::to_string(&invalid).unwrap();
    click_panel(&ctx, &mut app, "预览导入");
    let plan = app.localization_ui.import_plan.as_ref().unwrap();
    assert!(!plan.can_apply);
    assert!(plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "STALE_SOURCE"));
    assert!(plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "INVALID_TOKEN"));
    let (_, output) = panel_frame(&ctx, &mut app, Vec::new());
    let rendered = text(&output);
    assert!(rendered.contains("STALE_SOURCE"), "{rendered}");
    assert!(rendered.contains("INVALID_TOKEN"), "{rendered}");
    let source = app
        .localization_ui
        .import_plan
        .as_ref()
        .unwrap()
        .diagnostics
        .iter()
        .find_map(|diagnostic| diagnostic.source.clone())
        .unwrap();
    click_panel(
        &ctx,
        &mut app,
        &format!("定位诊断 {}:{} · {}", source.file, source.line, source.kind),
    );
    assert_eq!(app.localization_ui.take_navigation(), Some(source));
    assert!(
        rendered.contains(".wl:"),
        "source location missing: {rendered}"
    );
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.localization_ui.has_unsubmitted_work());

    drop(app);
    fs::remove_dir_all(root).unwrap();
}
