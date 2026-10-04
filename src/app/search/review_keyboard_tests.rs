use super::*;

fn draw(
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
        |ctx| eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest()),
    )
}

fn press(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    key: egui::Key,
) -> Vec<String> {
    let mut labels = Vec::new();
    for pressed in [true, false] {
        let output = draw(
            ctx,
            app,
            size,
            vec![egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        for event in output.platform_output.events {
            if let egui::output::OutputEvent::FocusGained(info) = event {
                if let Some(label) = info.label {
                    labels.push(label);
                }
            }
        }
    }
    labels
}

fn focus_named(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2, target: &str) {
    let mut seen = Vec::new();
    for _ in 0..100 {
        let labels = press(ctx, app, size, egui::Key::Tab);
        if labels.iter().any(|label| label == target) {
            return;
        }
        seen.extend(labels);
    }
    panic!("Tab could not reach {target}: {seen:?}");
}

fn visible_rect(output: &egui::FullOutput, label: &str) -> Option<egui::Rect> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(text) if text.galley.text() == label => {
            let rect = text.galley.rect.translate(text.pos.to_vec2());
            shape.clip_rect.contains_rect(rect).then_some(rect)
        }
        _ => None,
    })
}

#[test]
fn keyboard_checkbox_space_and_action_enter_are_not_stolen_by_search_navigation() {
    let size = egui::vec2(1024.0, 768.0);
    let (ctx, mut app) = app();
    app.open_search(&ctx, false, true);
    app.project_query = "needle".into();
    app.search_state.replacement = "newword".into();
    for _ in 0..4 {
        draw(&ctx, &mut app, size, vec![]);
    }
    focus_named(&ctx, &mut app, size, "待改");
    let current = app.search_state.selected;
    press(&ctx, &mut app, size, egui::Key::Space);
    assert_eq!(app.search_state.chosen.len(), 1);
    assert_eq!(app.search_state.selected, current);
    assert!(app.search_state.located.is_none());
    focus_named(&ctx, &mut app, size, "待改");
    press(&ctx, &mut app, size, egui::Key::Space);
    assert_eq!(app.search_state.chosen.len(), 2);
    focus_named(&ctx, &mut app, size, "预览已选 2 处");
    let id = ctx.memory(|m| m.focused()).unwrap();
    assert!(!app.search_navigation_has_focus(&ctx));
    let output = draw(&ctx, &mut app, size, vec![]);
    assert!(visible_rect(&output, "预览已选 2 处").is_some());
    assert!(ctx.read_response(id).unwrap().has_focus());
    press(&ctx, &mut app, size, egui::Key::Enter);
    assert!(app.search_state.plan.is_some());
    assert!(app.search_state.located.is_none());
    focus_named(&ctx, &mut app, size, "取消预览");
    let baseline = app.project.content_baseline();
    press(&ctx, &mut app, size, egui::Key::Enter);
    assert!(app.search_state.plan.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.search_state.chosen.len(), 2);
    // Let egui retire the vanished Cancel response, as when the author pauses to review.
    for _ in 0..8 {
        draw(&ctx, &mut app, size, vec![]);
    }
    assert_review_tab_focus(&ctx, &mut app, size);
    // A disappearing cancel button must not strand focus in the modal review.
    for _ in 0..2 {
        for pressed in [true, false] {
            draw(
                &ctx,
                &mut app,
                size,
                vec![egui::Event::Key {
                    key: egui::Key::Tab,
                    physical_key: Some(egui::Key::Tab),
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::SHIFT,
                }],
            );
        }
    }
    press(&ctx, &mut app, size, egui::Key::Tab);
    focus_named(&ctx, &mut app, size, "预览已选 2 处");
    press(&ctx, &mut app, size, egui::Key::Enter);
    assert_eq!(app.search_state.plan.as_ref().unwrap().hits.len(), 2);
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn query_enter_navigates_but_replacement_enter_never_navigates_or_applies() {
    let size = egui::vec2(1024.0, 768.0);
    let (ctx, mut app) = app();
    app.open_search(&ctx, false, true);
    app.project_query = "needle".into();
    for _ in 0..4 {
        draw(&ctx, &mut app, size, vec![]);
    }
    press(&ctx, &mut app, size, egui::Key::Enter);
    assert_eq!(app.search_state.selected, 0);
    assert!(app.search_state.located.is_some());
    press(&ctx, &mut app, size, egui::Key::Enter);
    assert_eq!(
        app.search_state.selected, 1,
        "query Enter must stay usable after first navigation"
    );
    let located = app.search_state.located.clone();
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("author-search-replacement")));
    let before = app.project.content_baseline();
    press(&ctx, &mut app, size, egui::Key::Enter);
    assert_eq!(app.search_state.located, located);
    assert_eq!(app.project.content_baseline(), before);
    assert!(app.search_state.plan.is_none());
}

#[test]
fn long_line_highlight_and_stable_actions_fit_small_window_large_type_and_both_themes() {
    for theme in [
        crate::theme::ThemeMode::Dark,
        crate::theme::ThemeMode::Light,
    ] {
        for (size, font) in [
            (egui::vec2(1024.0, 768.0), 24.0),
            (egui::vec2(1188.0, 848.0), 16.0),
        ] {
            let (ctx, mut app) = app();
            app.personal.settings.theme = theme;
            app.personal.settings.body_size = font;
            app.project
                .set_text(
                    &app.active_file.clone(),
                    format!(
                        "event start\n  {}needle 尾声\n  第二处 needle\n  第三处 needle\n",
                        "长".repeat(254)
                    ),
                )
                .unwrap();
            app.open_search(&ctx, false, true);
            app.project_query = "needle".into();
            app.search_state.replacement = "修订".into();
            app.choose_all_search_hits();
            for _ in 0..5 {
                draw(&ctx, &mut app, size, vec![]);
            }
            let output = draw(&ctx, &mut app, size, vec![]);
            let window = ctx
                .memory(|m| m.area_rect(egui::Id::new("author-search-window")))
                .unwrap();
            assert!(
                egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(window),
                "search window left viewport: {window:?}"
            );
            assert!(visible_rect(&output, "预览已选 3 处").is_some());
            assert!(visible_rect(&output, "关闭查找 · Esc").is_some());
            let hit = app.current_search_hits().unwrap().remove(0);
            assert_eq!(hit.column, 257);
            let context = hit.context.as_ref().unwrap();
            assert_eq!(&context.text[context.highlight.clone()], "needle");
            let ui = egui::Ui::new(
                ctx.clone(),
                egui::Id::new("context-format-test"),
                egui::UiBuilder::new(),
            );
            let job = review_view::context_job(&ui, context, &app.personal.settings);
            assert!(job
                .sections
                .iter()
                .any(|section| section.byte_range == context.highlight
                    && section.format.underline.width > 0.0));
            app.preview_selected_search_replacement();
            for _ in 0..3 {
                draw(&ctx, &mut app, size, vec![]);
            }
            let output = draw(&ctx, &mut app, size, vec![]);
            assert!(visible_rect(&output, "确认应用 3 处").is_some());
            assert!(visible_rect(&output, "取消预览").is_some());
            assert!(visible_rect(&output, "关闭查找 · Esc").is_some());
        }
    }
}

#[test]
fn successful_count_expires_on_undo_and_later_source_edit_not_the_apply_frame() {
    let (ctx, mut app) = app();
    app.open_search(&ctx, false, true);
    app.project_query = "needle".into();
    app.search_state.replacement = "changed".into();
    app.choose_all_search_hits();
    app.preview_selected_search_replacement();
    app.apply_search_replacement();
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.search_state.applied_count, Some(2));
    app.edit_undo(false);
    assert!(app.search_state.applied_count.is_none());
    app.choose_all_search_hits();
    app.preview_selected_search_replacement();
    app.apply_search_replacement();
    assert_eq!(app.search_state.applied_count, Some(2));
    app.project
        .set_text(
            &app.active_file.clone(),
            "event start\n  later input\n".into(),
        )
        .unwrap();
    frame(&ctx, &mut app, vec![]);
    assert!(app.search_state.applied_count.is_none());
}

#[test]
fn keyboard_source_view_return_retains_selection_and_preview() {
    let size = egui::vec2(1024.0, 768.0);
    let (ctx, mut app) = app();
    app.open_search(&ctx, false, true);
    app.project_query = "needle".into();
    app.search_state.replacement = "newword".into();
    app.choose_all_search_hits();
    app.preview_selected_search_replacement();
    for _ in 0..4 {
        draw(&ctx, &mut app, size, vec![]);
    }
    focus_named(&ctx, &mut app, size, "查看当前原文");
    press(&ctx, &mut app, size, egui::Key::Enter);
    assert!(app.search_state.source_view);
    assert_eq!(app.search_state.chosen.len(), 2);
    let plan = app.search_state.plan.clone();
    // The editor owns Tab while viewing source; its visible return shortcut restores review.
    draw(
        &ctx,
        &mut app,
        size,
        vec![key(egui::Key::ArrowLeft, egui::Modifiers::ALT)],
    );
    assert!(!app.search_state.source_view);
    assert_eq!(app.search_state.chosen.len(), 2);
    assert_eq!(app.search_state.plan, plan);
}

#[test]
fn short_and_long_result_galleys_share_the_same_left_reading_edge() {
    let size = egui::vec2(1188.0, 848.0);
    let (ctx, mut app) = app();
    app.project
        .set_text(
            &app.active_file.clone(),
            format!(
                "event start\n  {} needle 长行结尾\n  短行 needle\n  -> END\n",
                "较长上下文需要沿同一条阅读边界对齐。".repeat(3)
            ),
        )
        .unwrap();
    app.open_search(&ctx, false, true);
    app.project_query = "needle".into();
    for _ in 0..5 {
        draw(&ctx, &mut app, size, vec![]);
    }
    let output = draw(&ctx, &mut app, size, vec![]);
    let left = |line| {
        output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text)
                    if text.galley.text().contains(&format!("world.wl:{line}:")) =>
                {
                    assert!(shape.clip_rect.contains(text.pos), "result must be visible");
                    Some(text.pos.x + text.galley.rect.left())
                }
                _ => None,
            })
            .expect("result context and metadata must have a real galley")
    };
    assert!(
        (left(2) - left(3)).abs() < 0.1,
        "long and short result text drifted horizontally: {} vs {}",
        left(2),
        left(3)
    );
}

fn assert_review_tab_focus(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2) {
    for _ in 0..8 {
        draw(ctx, app, size, vec![]);
    }
    let output = draw(ctx, app, size, vec![]);
    let label = visible_rect(&output, "命中与勾选").expect("stable review tab must stay visible");
    let id = ctx
        .memory(|memory| memory.focused())
        .expect("review action must hand back visible focus");
    let response = ctx.read_response(id).unwrap();
    assert!(response.has_focus());
    assert!(
        response.rect.contains_rect(label),
        "focus must belong to the review tab"
    );
    assert!(
        !app.search_navigation_has_focus(ctx),
        "undo must not target query text"
    );
}

#[test]
fn clear_and_apply_restore_visible_review_focus_for_undo_and_another_preview() {
    let size = egui::vec2(1024.0, 768.0);
    let (ctx, mut app) = app();
    app.open_search(&ctx, false, true);
    app.project_query = "needle".into();
    app.search_state.replacement = "newword".into();
    app.choose_all_search_hits();
    app.preview_selected_search_replacement();
    for _ in 0..4 {
        draw(&ctx, &mut app, size, vec![]);
    }
    let baseline = app.project.content_baseline();
    focus_named(&ctx, &mut app, size, "清空选择");
    press(&ctx, &mut app, size, egui::Key::Enter);
    assert!(app.search_state.chosen.is_empty());
    assert!(app.search_state.plan.is_none());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_review_tab_focus(&ctx, &mut app, size);
    focus_named(&ctx, &mut app, size, "全选本范围可替换 2 处");
    press(&ctx, &mut app, size, egui::Key::Enter);
    focus_named(&ctx, &mut app, size, "预览已选 2 处");
    press(&ctx, &mut app, size, egui::Key::Enter);
    assert_eq!(app.search_state.plan.as_ref().unwrap().hits.len(), 2);
    focus_named(&ctx, &mut app, size, "确认应用 2 处");
    press(&ctx, &mut app, size, egui::Key::Enter);
    assert_eq!(app.search_state.applied_count, Some(2));
    assert!(app.search_state.plan.is_none());
    assert_review_tab_focus(&ctx, &mut app, size);
    for pressed in [true, false] {
        draw(
            &ctx,
            &mut app,
            size,
            vec![egui::Event::Key {
                key: egui::Key::Z,
                physical_key: Some(egui::Key::Z),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            }],
        );
    }
    assert_eq!(
        app.project.content_baseline(),
        baseline,
        "one Ctrl+Z restores the transaction"
    );
    assert_eq!(
        app.project_query, "needle",
        "transaction undo must not undo the query"
    );
    focus_named(&ctx, &mut app, size, "全选本范围可替换 2 处");
    press(&ctx, &mut app, size, egui::Key::Enter);
    focus_named(&ctx, &mut app, size, "预览已选 2 处");
    press(&ctx, &mut app, size, egui::Key::Enter);
    assert_eq!(app.search_state.plan.as_ref().unwrap().hits.len(), 2);
}
