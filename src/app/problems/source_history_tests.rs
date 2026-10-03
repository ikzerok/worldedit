//! 页游标与过滤分别恢复；原稿基线失效时不复活独立来源装饰。
use super::tests::{app, frame};
use worldline_core::problems::ProblemPrecision;

#[test]
fn crossing_page_200_to_201_and_back_restores_core_page_cursor_and_selected_source() {
    for change_filter in [false, true] {
        let (ctx, mut app) = app();
        let path = app.active_file.clone();
        let source = format!(
            "event start\n{}",
            (0..205)
                .map(|i| format!("  -> missing_{i:03}\n"))
                .collect::<String>()
        );
        app.project.set_text(&path, source.clone()).unwrap();
        app.recompile();
        let report = app.project.problems_report(&Default::default()).unwrap();
        assert_eq!(report.entries.len(), 205);
        app.problems.observation = Some(report.source_observation.clone());
        app.problems.install(report, app.version);
        let id = app.problems.page.as_ref().unwrap().entries[199].id.clone();
        app.problems.select(id.clone());
        app.locate_problem(&ctx, None);
        frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
        let saved = app.capture_problem_source().unwrap();
        let range = app.problem_source_range(&path).unwrap();
        let undo = app.history.len();
        let version = app.version;
        app.step_problem(&ctx, false, true);
        frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
        assert_eq!(app.problems.cursor.as_ref().unwrap().offset, 200);
        if change_filter {
            app.problems.query.text = "missing_204".into();
            app.problems.change_filter();
        }
        app.author_back(&ctx);
        frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
        if change_filter {
            assert_eq!(app.problems.query.text, "missing_204");
            assert!(app.capture_problem_source().is_none());
            assert!(app.problem_source_range(&path).is_none());
        } else {
            assert!(app.problems.cursor.is_none());
            assert!(app.problems.previous_pages.is_empty());
            assert_eq!(app.problems.page.as_ref().unwrap().entries[199].id, id);
            assert_eq!(app.problems.selected.as_ref(), Some(&id));
            assert!(app.capture_problem_source().as_ref() == Some(&saved));
            assert_eq!(app.problem_source_range(&path), Some(range));
        }
        assert_eq!(app.project.document(&path).unwrap(), source);
        assert_eq!(app.history.len(), undo);
        assert_eq!(app.version, version);
    }
}

#[test]
fn back_that_rejects_changed_source_cursor_also_rejects_its_problem_decoration() {
    let (ctx, mut app) = app();
    let id = app
        .problems
        .report
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .find(|entry| entry.primary.precision == ProblemPrecision::Span)
        .unwrap()
        .id
        .clone();
    app.problems.select(id);
    app.locate_problem(&ctx, None);
    frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    let path = app.active_file.clone();
    let location = app.author_location(Some(&ctx));
    assert!(location.source_problem.is_some());
    app.remember_author_location(location);
    // 直接改变已加载缓冲而不递增UI version，确保Back使用真实来源基线。
    app.project
        .set_text(&path, "event new_source\n  新稿🧭\n".into())
        .unwrap();
    app.author_back(&ctx);
    assert!(app.capture_problem_source().is_none());
    assert!(app.problem_source_range(&path).is_none());
    assert!(app.message.as_ref().unwrap().contains("未恢复旧选区和滚动"));
}
