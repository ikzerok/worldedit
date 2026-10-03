//! 当前来源的真实egui回归；不代替原生、系统IME或读屏验收。
use super::tests::{app, frame, key};
use crate::{app::Tab, theme};
use worldline_core::problems::ProblemPrecision;

fn texts(output: &egui::FullOutput) -> String {
    fn add(shape: &egui::Shape, result: &mut String) {
        match shape {
            egui::Shape::Text(text) => { result.push_str(text.galley.text()); result.push('\n'); }
            egui::Shape::Vec(shapes) => for shape in shapes { add(shape, result); },
            _ => {}
        }
    }
    let mut result = String::new();
    for shape in &output.shapes { add(&shape.shape, &mut result); }
    result
}

fn located() -> (egui::Context, crate::app::WorldeditApp, String) {
    let (ctx, mut app) = app();
    let entry = app.problems.report.as_ref().unwrap().entries.iter()
        .find(|entry| entry.primary.precision == ProblemPrecision::Span).unwrap().clone();
    app.problems.select(entry.id.clone());
    app.locate_problem(&ctx, None);
    frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    (ctx, app, entry.id)
}

#[test]
fn current_problem_identity_survives_cursor_and_appearance_then_expires_on_edit() {
    let (ctx, mut app, _) = located();
    let path = app.active_file.clone();
    let baseline = app.project.content_baseline();
    let version = app.version;
    let range = app.problem_source_range(&path).unwrap();
    let captured = app.capture_problem_source().unwrap();
    let id = egui::Id::new(("source", &path));
    let mut state = egui::TextEdit::load_state(&ctx, id).unwrap();
    state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(0))));
    state.store(&ctx, id);
    for size in [egui::vec2(1040., 660.), egui::vec2(1280., 800.), egui::vec2(1600., 1000.)] {
        for font in [16., 28.] {
            for wrap in [false, true] {
                for mode in [theme::ThemeMode::Dark, theme::ThemeMode::Light] {
                    app.personal.settings.body_size = font;
                    app.personal.settings.source_wrap = wrap;
                    theme::configure(&ctx, mode);
                    let before = egui::TextEdit::load_state(&ctx, id).unwrap().cursor.char_range();
                    let output = frame(&ctx, &mut app, size, vec![]);
                    assert!(texts(&output).contains("当前问题"));
                    assert!(!texts(&output).contains("从选中文本建档"));
                    assert_eq!(app.problem_source_range(&path), Some(range.clone()));
                    assert_eq!(egui::TextEdit::load_state(&ctx, id).unwrap().cursor.char_range(), before);
                    assert_eq!(app.project.content_baseline(), baseline);
                    assert_eq!(app.version, version);
                }
            }
        }
    }
    ctx.memory_mut(|memory| memory.request_focus(id));
    frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![egui::Event::Text("//改稿\n".into())]);
    assert!(app.problem_source_range(&path).is_none());
    let output = frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    assert!(texts(&output).contains("位置强调已暂停"));
    let report = app.project.problems_report(&Default::default()).unwrap();
    app.problems.observation = Some(report.source_observation.clone());
    app.problems.install(report, app.version);
    app.restore_problem_source(Some(captured));
    assert!(app.capture_problem_source().is_none(), "新报告不能复活旧来源身份");
}

#[test]
fn summary_keyboard_return_consumes_enter_and_focuses_existing_details() {
    let (ctx, mut app, problem_id) = located();
    let baseline = app.project.content_baseline();
    let scope = egui::Id::new(("problem-source-summary",
        &app.problems.report.as_ref().unwrap().report_version, &problem_id));
    ctx.memory_mut(|memory| memory.request_focus(scope.with("details")));
    frame(&ctx, &mut app, egui::vec2(1040., 660.), vec![key(egui::Key::Enter), egui::Event::Text("\n".into())]);
    frame(&ctx, &mut app, egui::vec2(1040., 660.), vec![]);
    assert!(app.personal.settings.diagnostics && app.problems.narrow_detail);
    assert_eq!(ctx.memory(|memory| memory.focused()),
        Some(egui::Id::new(("problem-detail", &problem_id)).with("copy-problem")));
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.tab, Tab::Edit);
}

#[test]
fn protected_ime_and_filter_changes_cannot_keep_old_emphasis() {
    let (ctx, mut app, _) = located();
    let path = app.active_file.clone();
    let id = egui::Id::new(("source", &path));
    ctx.memory_mut(|memory| memory.request_focus(id));
    app.ime_composing = true;
    assert!(app.problem_source_range(&path).is_none());
    frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    app.ime_composing = false;
    app.problems.query.text = "不存在的问题筛选".into();
    app.problems.change_filter();
    assert!(app.capture_problem_source().is_none());
    assert!(app.problem_source_range(&path).is_none());
}
