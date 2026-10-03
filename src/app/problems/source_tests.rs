//! 主/关联来源复用core坐标与作者Back，不按同名文件替换。
use super::tests::{app, frame};
use crate::app::Tab;
use std::path::Path;

#[test]
fn same_basename_primary_related_and_back_preserve_exact_source_identity() {
    let (ctx, mut app) = app();
    app.project
        .set_text(
            &app.active_file.clone(),
            "world glass_tide as \"玻璃潮\"\n".into(),
        )
        .unwrap();
    for (path, display) in [
        ("north/钟楼.wl", "北钟楼 🧭"),
        ("south/钟楼.wl", "南钟楼 🧭"),
    ] {
        let file = app.project.add_file(Path::new(path)).unwrap();
        app.project
            .set_text(&file, format!("character keeper as \"{display}\"\r\n"))
            .unwrap();
    }
    app.recompile();
    let report = app.project.problems_report(&Default::default()).unwrap();
    let entry = report
        .entries
        .iter()
        .find(|entry| entry.code == "A104" && entry.related_count > 0)
        .unwrap()
        .clone();
    let primary = app
        .project
        .problem_location(&report, &entry.id, None)
        .unwrap();
    let related = app
        .project
        .problem_location(&report, &entry.id, Some(0))
        .unwrap();
    assert_ne!(primary.path, related.path);
    assert_eq!(
        Path::new(primary.path.as_ref().unwrap()).file_name(),
        Path::new(related.path.as_ref().unwrap()).file_name()
    );
    let baseline = app.project.content_baseline();
    app.problems.observation = Some(report.source_observation.clone());
    app.problems.install(report, app.version);
    app.problems.select(entry.id.clone());
    let original = app.active_file.clone();
    app.locate_problem(&ctx, None);
    assert_eq!(
        app.active_file,
        app.project.root.join(primary.path.as_ref().unwrap())
    );
    for _ in 0..3 {
        frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    }
    let selected = egui::TextEdit::load_state(&ctx, egui::Id::new(("source", &app.active_file)))
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    let expected = primary.char_range.as_ref().unwrap();
    assert_eq!(
        selected.primary.index.min(selected.secondary.index),
        expected.start
    );
    assert_eq!(
        selected.primary.index.max(selected.secondary.index),
        expected.end
    );
    app.locate_problem(&ctx, Some(0));
    assert_eq!(
        app.active_file,
        app.project.root.join(related.path.as_ref().unwrap())
    );
    frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    app.author_back(&ctx);
    assert_eq!(
        app.active_file,
        app.project.root.join(primary.path.unwrap())
    );
    app.author_back(&ctx);
    assert_eq!(app.active_file, original);
    assert_eq!(app.tab, Tab::Manuscript);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.problems.selected, Some(entry.id));
}

#[test]
fn asset_delete_restore_requires_recheck_even_when_the_observation_key_returns() {
    let (ctx, mut app) = app();
    let root = app.project.root.clone();
    std::fs::create_dir_all(&root).unwrap();
    let asset = root.join("fixture-asset.bin");
    std::fs::write(&asset, b"initial").unwrap();
    let report = app.project.problems_report(&Default::default()).unwrap();
    let first_observation = report.source_observation.clone();
    app.problems.observation = Some(first_observation.clone());
    app.problems.install(report, app.version);
    let baseline = app.project.content_baseline();
    let version = app.version;
    std::fs::remove_file(&asset).unwrap();
    let _ = ctx.run(
        egui::RawInput {
            time: Some(2.),
            ..Default::default()
        },
        |ctx| app.poll_problems(ctx),
    );
    assert_eq!(app.version, version);
    assert_eq!(app.project.content_baseline(), baseline);
    assert_ne!(app.problems.observation.as_ref(), Some(&first_observation));
    assert!(app.problems.stale(version));
    std::fs::write(&asset, b"restored").unwrap();
    let _ = ctx.run(
        egui::RawInput {
            time: Some(3.1),
            ..Default::default()
        },
        |ctx| app.poll_problems(ctx),
    );
    assert_eq!(app.problems.observation.as_ref(), Some(&first_observation));
    assert!(
        app.problems.stale(version),
        "恢复同名附件不能复活旧报告的新鲜状态"
    );
    let report = app.project.problems_report(&Default::default()).unwrap();
    app.problems.install(report, version);
    assert!(!app.problems.stale(version));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn document_only_problem_opens_json_without_fake_range_and_back_restores_json_selection() {
    use worldline_core::problems::{ProblemDomain, ProblemPrecision};
    let (ctx, mut app) = app();
    let root = app.project.root.clone();
    app.project.create_authoring_document(&root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.13","required_features":["presentation.maps.v1"],"maps":{"broken":".world/maps.json"}}"#.to_vec()).unwrap();
    let json_path = root.join(".world/maps.json");
    let raw = format!("{{\n{}", "  未完成的中文 JSON 🧭\n".repeat(120)).into_bytes();
    app.project
        .create_authoring_document(&json_path, raw.clone())
        .unwrap();
    app.recompile();
    let report = app.project.problems_report(&Default::default()).unwrap();
    let document = report
        .entries
        .iter()
        .find(|entry| {
            entry.domain == ProblemDomain::Maps
                && entry.primary.precision == ProblemPrecision::Document
        })
        .unwrap()
        .clone();
    let source = report
        .entries
        .iter()
        .find(|entry| {
            entry.domain == ProblemDomain::Content
                && entry.primary.precision == ProblemPrecision::Span
        })
        .unwrap()
        .clone();
    assert!(document.primary.span.is_none() && document.primary.byte_range.is_none());
    app.problems.observation = Some(report.source_observation.clone());
    app.problems.install(report, app.version);
    let id = egui::Id::new(("authoring-source", &json_path));
    let mut state = egui::TextEdit::load_state(&ctx, id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(2),
            egui::text::CCursor::new(5),
        )));
    state.store(&ctx, id);
    app.problems.select(document.id);
    app.locate_problem(&ctx, None);
    assert_eq!(app.active_file, json_path);
    assert!(app.jump.is_none());
    let collapsed = egui::TextEdit::load_state(&ctx, id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!(
        collapsed.primary.index, collapsed.secondary.index,
        "文档级不能呈现假精确选区"
    );
    for _ in 0..3 {
        frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    }
    let mut state = egui::TextEdit::load_state(&ctx, id).unwrap();
    state.cursor.set_char_range(Some(egui::text::CCursorRange {
        primary: egui::text::CCursor::new(20),
        secondary: egui::text::CCursor::new(2),
        h_pos: None,
    }));
    state.store(&ctx, id);
    app.personal.source_scroll = [0., 120.];
    app.problems.select(source.id);
    app.locate_problem(&ctx, None);
    assert_ne!(app.active_file, json_path);
    app.author_back(&ctx);
    assert_eq!(app.active_file, json_path);
    let restored = egui::TextEdit::load_state(&ctx, id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    assert_eq!((restored.primary.index, restored.secondary.index), (20, 2));
    assert_eq!(app.personal.source_scroll, [0., 120.]);
    assert!(app.personal.restore_source);
    frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    assert!(!app.personal.restore_source);
    assert!(app.personal.source_scroll[1] > 100.);
    assert_eq!(
        app.project.authoring_document(&json_path).unwrap().bytes(),
        raw
    );
}
