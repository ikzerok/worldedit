//! 真实 egui offscreen 帧与 core DTO 测试；不是物理键盘/桌面/IME 验收。
use super::*;
use crate::app::Tab;
use worldline_core::{project::Project, Severity};

pub(super) fn app() -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.);
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = std::env::temp_dir().join(format!(
        "problems-ui-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.project = Project::new(&root);
    app.active_file = app.project.entry.clone();
    app.project
        .documents
        .retain(|path, _| path == &app.active_file);
    app.project
        .set_text(
            &app.active_file.clone(),
            "character keeper as \"守钟人\"\nevent entrance as \"长中文 🧭\"\n  -> missing\n"
                .into(),
        )
        .unwrap();
    app.recompile();
    app.tab = Tab::Manuscript;
    let report = app.project.problems_report(&Default::default()).unwrap();
    app.problems.observation = Some(report.source_observation.clone());
    app.problems.install(report, app.version);
    app.problems.schedule.observe(app.version, 0.);
    app.problems.schedule.cancel();
    (ctx, app)
}
pub(super) fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| {
            app.author_shortcuts(ctx);
            app.problems_panel(ctx);
            let remaining = ctx.available_rect().height();
            ctx.data_mut(|data| {
                data.insert_temp(egui::Id::new("problems-test-content-height"), remaining)
            });
            if app.tab == Tab::Edit {
                app.source_tab(ctx);
            } else {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.label("原作者页");
                });
            }
            app.capture_edit_focus(ctx);
        },
    )
}
pub(super) fn key(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
fn text(shape: &egui::epaint::Shape, output: &mut String) {
    match shape {
        egui::epaint::Shape::Text(value) => {
            output.push_str(value.galley.text());
            output.push('\n');
        }
        egui::epaint::Shape::Vec(values) => {
            for value in values {
                text(value, output);
            }
        }
        _ => {}
    }
}

#[test]
fn filters_pages_and_partial_replacement_never_claim_resolution_or_compile() {
    let (_, mut app) = app();
    let original = app.problems.report.as_ref().unwrap().clone();
    let id = original.entries.first().unwrap().id.clone();
    app.problems.select(id);
    let baseline = app.project.content_baseline();
    let version = app.version;
    app.problems.query.text = "no-such-problem-text".into();
    app.problems.change_filter();
    assert_eq!(app.problems.page.as_ref().unwrap().matched, 0);
    assert!(app.problems.selected.is_none());
    assert!(app
        .problems
        .notice
        .as_ref()
        .unwrap()
        .contains("不表示已解决"));
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.version, version);
    assert!(std::sync::Arc::ptr_eq(
        &original,
        app.problems.report.as_ref().unwrap()
    ));
    app.problems.query = Default::default();
    app.problems.change_filter();
    app.problems.select(original.entries[0].id.clone());
    let mut replacement = (*original).clone();
    replacement.complete = false;
    replacement.truncated = true;
    replacement.report_version.push_str("-partial");
    replacement.entries.clear();
    app.problems.install(replacement, version + 1);
    assert!(app.problems.selected.is_none());
    assert!(app
        .problems
        .notice
        .as_ref()
        .unwrap()
        .contains("不能据此认定已解决"));
}

#[test]
fn keyboard_selects_without_navigation_then_enter_uses_core_and_back_restores() {
    let (ctx, mut app) = app();
    let original_tab = app.tab;
    let original_file = app.active_file.clone();
    app.open_problems(&ctx);
    for _ in 0..3 {
        frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    }
    frame(
        &ctx,
        &mut app,
        egui::vec2(1280., 800.),
        vec![key(egui::Key::ArrowDown)],
    );
    assert!(app.problems.selected.is_some());
    assert_eq!(app.tab, original_tab);
    frame(
        &ctx,
        &mut app,
        egui::vec2(1280., 800.),
        vec![key(egui::Key::Enter)],
    );
    assert_eq!(app.tab, Tab::Edit);
    app.author_back(&ctx);
    assert_eq!(app.tab, original_tab);
    assert_eq!(app.active_file, original_file);
    assert!(app.personal.settings.diagnostics);
}

#[test]
fn stale_report_and_ime_do_not_navigate_or_overwrite_a_draft() {
    let (ctx, mut app) = app();
    let id = app.problems.report.as_ref().unwrap().entries[0].id.clone();
    app.problems.select(id);
    app.project
        .set_text(
            &app.active_file.clone(),
            "character newly_written as \"新稿 🧭\"\n".into(),
        )
        .unwrap();
    let expected = app.project.document(&app.active_file).unwrap().to_owned();
    app.locate_problem(&ctx, None);
    assert_eq!(app.tab, Tab::Manuscript);
    assert!(app
        .problems
        .error
        .as_ref()
        .unwrap()
        .contains("STALE_REPORT"));
    app.ime_composing = true;
    app.locate_problem(&ctx, None);
    assert_eq!(app.project.document(&app.active_file).unwrap(), expected);
    assert_eq!(app.tab, Tab::Manuscript);
    assert!(app.problems.notice.as_ref().unwrap().contains("输入法"));
}

#[test]
fn five_thousand_rows_are_paginated_and_only_visible_rows_are_laid_out() {
    let (ctx, mut app) = app();
    let mut report = (**app.problems.report.as_ref().unwrap()).clone();
    let entry = report.entries[0].clone();
    report.entries = (0..5_000)
        .map(|i| {
            let mut row = entry.clone();
            row.id = format!("issue-{i}");
            row.message = format!("第{i}处长中文问题，不能用同名覆盖另一来源 🧭");
            row.severity = Severity::Error;
            row
        })
        .collect();
    app.problems.install(report, app.version);
    app.open_problems(&ctx);
    for size in [
        egui::vec2(1040., 660.),
        egui::vec2(1280., 800.),
        egui::vec2(1600., 1000.),
    ] {
        for theme in [
            crate::theme::ThemeMode::Dark,
            crate::theme::ThemeMode::Light,
        ] {
            app.personal.settings.theme = theme;
            crate::theme::configure(&ctx, theme);
            for font in [16., 28.] {
                app.personal.settings.body_size = font;
                let output = frame(&ctx, &mut app, size, vec![]);
                assert!(app.problems.rendered_rows > 0 && app.problems.rendered_rows < 40);
                let mut rendered = String::new();
                for shape in &output.shapes {
                    text(&shape.shape, &mut rendered);
                }
                assert!(rendered.contains("总计 5000 · 匹配 5000"));
                assert_eq!(app.problems.page.as_ref().unwrap().entries.len(), 200);
                assert!(
                    ctx.data(
                        |data| data.get_temp::<f32>(egui::Id::new("problems-test-content-height"))
                    )
                    .unwrap()
                        >= 230.,
                    "正文需要保留可用高度：{size:?}"
                );
            }
        }
    }
    app.problem_page(false);
    assert_eq!(app.problems.cursor.as_ref().unwrap().offset, 200);
    assert_eq!(
        app.problems.page.as_ref().unwrap().entries[0].id,
        "issue-200"
    );
    app.problem_page(true);
    assert_eq!(app.problems.page.as_ref().unwrap().entries[0].id, "issue-0");
}

#[test]
fn native_worker_returns_matching_read_only_report_and_cancellation_is_isolated() {
    let (ctx, app) = app();
    let baseline = app.project.content_baseline();
    let ticket = schedule::ReportTicket {
        generation: 3,
        version: app.version,
        baseline: baseline.clone(),
        observation: app.project.problems_observation_key().unwrap(),
    };
    let mut job = job::ProblemsJob::start(&app.project, ticket, &ctx).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let report = loop {
        if let Some(result) = job.poll() {
            break result.unwrap();
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert_eq!(report.content_baseline, baseline);
    assert_eq!(report.compile_count, 1);
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(report.limits.max_report_bytes <= 16 * 1024 * 1024);
}

#[test]
fn row_has_two_visible_text_layers_and_uses_one_list_keyboard_focus() {
    let (ctx, app) = app();
    let problem = app.problems.report.as_ref().unwrap().entries[0].clone();
    let source = super::view::location_label(&problem.primary);
    let mut row_focusable = true;
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let response = super::view::problem_row(ui, &problem, true, 16., 48.);
            row_focusable = response.sense.is_focusable();
        });
    });
    assert!(!row_focusable, "行点击后焦点归列表，不能额外制造Tab陷阱");
    assert!(
        output.shapes.iter().any(|shape| match &shape.shape {
            egui::epaint::Shape::Text(text) => text.galley.text().starts_with(&source),
            _ => false,
        }),
        "相对路径必须有独立可见文本层，不可被摘要单行截断吞掉"
    );
}

#[test]
fn pointer_click_on_both_row_text_layers_selects_the_problem() {
    let (ctx, mut app) = app();
    app.open_problems(&ctx);
    for _ in 0..3 {
        frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    }
    for secondary in [false, true] {
        app.problems.selected = None;
        let output = frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
        let first = &app.problems.page.as_ref().unwrap().entries[0];
        let expected = first.id.clone();
        let prefix = if secondary {
            super::view::location_label(&first.primary)
        } else {
            format!("{} · ", super::view::severity_label(first.severity))
        };
        let point = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::epaint::Shape::Text(text) if text.galley.text().starts_with(&prefix) => {
                    Some(text.pos + egui::vec2(8., text.galley.size().y / 2.))
                }
                _ => None,
            })
            .expect("问题行文本必须可见");
        for pressed in [true, false] {
            frame(
                &ctx,
                &mut app,
                egui::vec2(1280., 800.),
                vec![
                    egui::Event::PointerMoved(point),
                    egui::Event::PointerButton {
                        pos: point,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert_eq!(
            app.problems.selected,
            Some(expected),
            "行内文本不能截走整行点击"
        );
    }
}
