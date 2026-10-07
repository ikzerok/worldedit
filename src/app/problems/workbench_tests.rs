//! 当前来源的真实egui回归；不代替原生、系统IME或读屏验收。
use super::tests::{app, frame, key};
use crate::{app::Tab, theme};
use worldline_core::problems::ProblemPrecision;

fn texts(output: &egui::FullOutput) -> String {
    fn add(shape: &egui::Shape, result: &mut String) {
        match shape {
            egui::Shape::Text(text) => {
                result.push_str(text.galley.text());
                result.push('\n');
            }
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    add(shape, result);
                }
            }
            _ => {}
        }
    }
    let mut result = String::new();
    for shape in &output.shapes {
        add(&shape.shape, &mut result);
    }
    result
}

fn located() -> (egui::Context, crate::app::WorldeditApp, String) {
    let (ctx, mut app) = app();
    let entry = app
        .problems
        .report
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .find(|entry| entry.primary.precision == ProblemPrecision::Span)
        .unwrap()
        .clone();
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
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(0),
        )));
    state.store(&ctx, id);
    for size in [
        egui::vec2(1040., 660.),
        egui::vec2(1280., 800.),
        egui::vec2(1600., 1000.),
    ] {
        for font in [16., 28.] {
            for wrap in [false, true] {
                for mode in [theme::ThemeMode::Dark, theme::ThemeMode::Light] {
                    app.personal.settings.body_size = font;
                    app.personal.settings.source_wrap = wrap;
                    let _theme = theme::configure(&ctx, mode);
                    let before = egui::TextEdit::load_state(&ctx, id)
                        .unwrap()
                        .cursor
                        .char_range();
                    let output = frame(&ctx, &mut app, size, vec![]);
                    assert!(texts(&output).contains("当前问题"));
                    assert!(!texts(&output).contains("从选中文本建档"));
                    assert_eq!(app.problem_source_range(&path), Some(range.clone()));
                    assert_eq!(
                        egui::TextEdit::load_state(&ctx, id)
                            .unwrap()
                            .cursor
                            .char_range(),
                        before
                    );
                    assert_eq!(app.project.content_baseline(), baseline);
                    assert_eq!(app.version, version);
                }
            }
        }
    }
    ctx.memory_mut(|memory| memory.request_focus(id));
    frame(
        &ctx,
        &mut app,
        egui::vec2(1280., 800.),
        vec![egui::Event::Text("//改稿\n".into())],
    );
    assert!(app.problem_source_range(&path).is_none());
    let output = frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    assert!(texts(&output).contains("位置强调已暂停"));
    let report = app.project.problems_report(&Default::default()).unwrap();
    app.problems.observation = Some(report.source_observation.clone());
    app.problems.install(report, app.version);
    app.restore_problem_source(Some(captured));
    assert!(
        app.capture_problem_source().is_none(),
        "新报告不能复活旧来源身份"
    );
}

#[test]
fn summary_keyboard_return_consumes_enter_and_focuses_existing_details() {
    let (ctx, mut app, problem_id) = located();
    let baseline = app.project.content_baseline();
    let scope = egui::Id::new((
        "problem-source-summary",
        &app.problems.report.as_ref().unwrap().report_version,
        &problem_id,
    ));
    ctx.memory_mut(|memory| memory.request_focus(scope.with("details")));
    frame(
        &ctx,
        &mut app,
        egui::vec2(1040., 660.),
        vec![key(egui::Key::Enter), egui::Event::Text("\n".into())],
    );
    frame(&ctx, &mut app, egui::vec2(1040., 660.), vec![]);
    assert!(app.personal.settings.diagnostics && app.problems.narrow_detail);
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        Some(egui::Id::new(("problem-detail", &problem_id)).with("copy-problem"))
    );
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

#[test]
fn replaced_report_with_same_hash_never_revives_a_historical_source_decoration() {
    let (_, mut app, _) = located();
    let old = app.capture_problem_source().unwrap();
    let report = (**app.problems.report.as_ref().unwrap()).clone();
    app.problems.invalidated = true;
    assert!(app.problem_source_range(&app.active_file).is_none());
    app.problems.install(report, app.version);
    app.restore_problem_source(Some(old));
    assert!(app.capture_problem_source().is_none());
    assert!(app.problem_source_range(&app.active_file).is_none());
}

#[test]
fn old_context_missing_report_stays_readable_but_cannot_navigate_or_decorate() {
    let (ctx, mut app) = app();
    let mut report = (**app.problems.report.as_ref().unwrap()).clone();
    let id = report
        .entries
        .iter()
        .find(|entry| entry.primary.precision == ProblemPrecision::Span)
        .unwrap()
        .id
        .clone();
    for entry in &mut report.entries {
        entry.primary.context = None;
    }
    for locations in report.related.values_mut() {
        for location in locations {
            location.context = None;
        }
    }
    app.problems.install(report, app.version);
    app.problems.select(id.clone());
    app.open_problems(&ctx);
    app.problems.narrow_detail = true;
    app.problems.focus_detail = true;
    ctx.style_mut(|style| style.scroll_animation = egui::style::ScrollAnimation::none());
    frame(&ctx, &mut app, egui::vec2(1040., 660.), vec![]);
    let copy = egui::Id::new(("problem-detail", &id))
        .with("primary")
        .with("copy-excerpt");
    // 必须经真实Tab获得焦点。帧外request_focus会被egui当成“上帧已有焦点”，
    // gained_focus永远false，也就不会触发focus_action的滚入。
    for _ in 0..40 {
        frame(
            &ctx,
            &mut app,
            egui::vec2(1040., 660.),
            vec![key(egui::Key::Tab)],
        );
        let mut released = key(egui::Key::Tab);
        if let egui::Event::Key { pressed, .. } = &mut released {
            *pressed = false;
        }
        frame(&ctx, &mut app, egui::vec2(1040., 660.), vec![released]);
        if ctx.memory(|memory| memory.focused()) == Some(copy) {
            break;
        }
    }
    let output = frame(&ctx, &mut app, egui::vec2(1040., 660.), vec![]);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(copy));
    assert!(texts(&output).contains("旧版或未知上下文仅供阅读"));
    assert!(
        output.shapes.iter().any(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text().contains("旧版或未知上下文仅供阅读") =>
                shape
                    .clip_rect
                    .expand(1.)
                    .contains_rect(text.galley.rect.translate(text.pos.to_vec2())),
            _ => false,
        }),
        "只读说明必须在最终帧完整可见，不能只存在于离屏布局"
    );
    let before = app.author_location(Some(&ctx));
    app.locate_problem(&ctx, None);
    assert!(app
        .problems
        .error
        .as_ref()
        .unwrap()
        .contains("STALE_REPORT"));
    assert_eq!(app.active_file, before.file);
    assert_eq!(Some(app.tab), before.tab);
    assert!(app.problems.source.is_none());
}

#[test]
fn next_problem_captures_previous_source_identity_before_changing_selection() {
    let (ctx, mut app) = app();
    let path = app.active_file.clone();
    app.project
        .set_text(
            &path,
            "event start\n  -> missing_one\n  -> missing_two\n".into(),
        )
        .unwrap();
    app.recompile();
    let report = app.project.problems_report(&Default::default()).unwrap();
    let spans: Vec<_> = report
        .entries
        .iter()
        .filter(|entry| entry.primary.precision == ProblemPrecision::Span)
        .collect();
    assert!(spans.len() >= 2);
    let first = spans[0].id.clone();
    app.problems.observation = Some(report.source_observation.clone());
    app.problems.install(report, app.version);
    app.problems.select(first.clone());
    app.locate_problem(&ctx, None);
    frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    let before = app.capture_problem_source().unwrap();
    let selected = egui::TextEdit::load_state(&ctx, egui::Id::new(("source", &path)))
        .unwrap()
        .cursor
        .char_range();
    let baseline = app.project.content_baseline();
    let undo = app.history.len();
    app.step_problem(&ctx, false, true);
    frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    assert_ne!(app.problems.selected.as_ref(), Some(&first));
    app.author_back(&ctx);
    let output = frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
    assert!(app.capture_problem_source().as_ref() == Some(&before));
    assert_eq!(app.problems.selected.as_ref(), Some(&first));
    assert_eq!(
        egui::TextEdit::load_state(&ctx, egui::Id::new(("source", &path)))
            .unwrap()
            .cursor
            .char_range(),
        selected
    );
    assert!(!texts(&output).contains("从选中文本建档"));
    assert_eq!(app.history.len(), undo);
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn list_enter_consumes_corresponding_text_before_source_focus_and_space_is_not_activation() {
    for activation in [egui::Key::Enter, egui::Key::Space] {
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
        app.open_problems(&ctx);
        for _ in 0..3 {
            frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
        }
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(egui::Id::new("problems-list"))
        );
        let sources = app.project.sources();
        let dirty = app.project.is_dirty();
        let version = app.version;
        let undo = app.history.len();
        let tab = app.tab;
        frame(
            &ctx,
            &mut app,
            egui::vec2(1280., 800.),
            vec![
                key(activation),
                egui::Event::Text(
                    if activation == egui::Key::Enter {
                        "\n"
                    } else {
                        " "
                    }
                    .into(),
                ),
            ],
        );
        assert_eq!(
            app.project.sources(),
            sources,
            "列表动作的Text事件不能进入新来源编辑器"
        );
        assert_eq!(app.project.is_dirty(), dirty);
        assert_eq!(app.version, version);
        assert_eq!(app.history.len(), undo);
        if activation == egui::Key::Enter {
            assert_eq!(app.tab, Tab::Edit);
            assert!(app.capture_problem_source().is_some());
        } else {
            assert_eq!(app.tab, tab, "列表协议只用Enter定位，Space不隐式定位");
            assert!(app.capture_problem_source().is_none());
            assert_eq!(
                ctx.memory(|memory| memory.focused()),
                Some(egui::Id::new("problems-list"))
            );
        }
    }
}

#[test]
fn activation_cleanup_preserves_unrelated_text_and_ime_composition_events() {
    for activation in [egui::Key::Enter, egui::Key::Space] {
        let ctx = egui::Context::default();
        let typed = if activation == egui::Key::Enter {
            "\n"
        } else {
            " "
        };
        let _ = ctx.run(egui::RawInput { events: vec![key(activation),
            egui::Event::Text(typed.into()), egui::Event::Text("普通文本".into()),
            egui::Event::Ime(egui::ImeEvent::Preedit("组合".into()))], ..Default::default() }, |ctx| {
            ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, activation));
            super::view::consume_activation(ctx, activation);
            let events = ctx.input(|input| input.events.clone());
            assert!(events.iter().any(|event| matches!(event, egui::Event::Text(text) if text == "普通文本")));
            assert!(events.iter().any(|event| matches!(event, egui::Event::Ime(egui::ImeEvent::Preedit(text)) if text == "组合")));
            assert!(!events.iter().any(|event| matches!(event, egui::Event::Text(text) if text == typed)));
        });
    }
}

#[test]
fn known_external_source_change_or_delete_immediately_invalidates_old_emphasis() {
    for deleted in [false, true] {
        let (ctx, mut app) = app();
        let path = app.active_file.clone();
        let root = app.project.root.clone();
        let original = app.project.document(&path).unwrap().to_owned();
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&path, &original).unwrap();
        app.project = worldline_core::project::Project::open(&path).unwrap();
        app.recompile();
        let report = app.project.problems_report(&Default::default()).unwrap();
        let id = report
            .entries
            .iter()
            .find(|entry| entry.primary.precision == ProblemPrecision::Span)
            .unwrap()
            .id
            .clone();
        app.problems.observation = Some(report.source_observation.clone());
        app.problems.install(report, app.version);
        app.problems.select(id);
        app.locate_problem(&ctx, None);
        frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
        assert!(app.problem_source_range(&path).is_some());
        let dirty = app.project.is_dirty();
        let version = app.version;
        let undo = app.history.len();
        let history = app.personal.history.clone();
        if deleted {
            std::fs::remove_file(&path).unwrap();
        } else {
            std::fs::write(&path, "event replaced\n  外部新稿\n").unwrap();
        }
        // 不安装新报告、不轮询自动刷新，直接重试同一条来源定位。
        app.locate_problem(&ctx, None);
        assert!(
            app.problem_source_range(&path).is_none(),
            "已知验证失败当帧必须撤旧强调"
        );
        assert!(app.problems.invalidated || app.problems.error.is_some());
        let output = frame(&ctx, &mut app, egui::vec2(1280., 800.), vec![]);
        assert!(texts(&output).contains("上次问题（待重检）"));
        assert!(texts(&output).contains("位置强调已暂停"));
        assert_eq!(app.project.document(&path).unwrap(), original);
        assert_eq!(app.project.is_dirty(), dirty);
        assert_eq!(app.version, version);
        assert_eq!(app.history.len(), undo);
        assert!(app.personal.history == history);
        std::fs::remove_dir_all(root).unwrap();
    }
}
