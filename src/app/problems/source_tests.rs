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
