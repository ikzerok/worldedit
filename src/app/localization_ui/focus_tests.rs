use super::workbench_tests::{fixture, frame, stage};
use super::*;
use egui::{Event, Key, Modifiers};

pub(super) fn draw(
    ctx: &egui::Context,
    project: &mut Project,
    state: &mut LocalizationUiState,
    events: Vec<Event>,
) -> egui::FullOutput {
    frame(ctx, project, state, egui::vec2(1680.0, 1300.0), events).1
}

pub(super) fn click_label(
    ctx: &egui::Context,
    project: &mut Project,
    state: &mut LocalizationUiState,
    label: &str,
) {
    for _ in 0..2 {
        draw(ctx, project, state, vec![]);
    }
    let output = draw(ctx, project, state, vec![]);
    let point = output
        .shapes
        .iter()
        .rev()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let rect = text
                    .galley
                    .rect
                    .translate(text.pos.to_vec2())
                    .intersect(shape.clip_rect);
                rect.is_positive().then(|| rect.center())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("visible control {label}"));
    for pressed in [true, false] {
        draw(
            ctx,
            project,
            state,
            vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ],
        );
    }
}

pub(super) fn ctrl_a(ctx: &egui::Context, project: &mut Project, state: &mut LocalizationUiState) {
    for pressed in [true, false] {
        draw(
            ctx,
            project,
            state,
            vec![Event::Key {
                key: Key::A,
                physical_key: Some(Key::A),
                pressed,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    command: true,
                    ..Modifiers::NONE
                },
            }],
        );
    }
}

#[test]
fn localization_typed_characters_keep_focus_when_first_draft_and_async_headers_appear() {
    let (mut project, mut state) = fixture(1);
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    stage(&mut state, "Original translation");
    plans::preview_edits(&project, &mut state);
    jobs::settle(&project, &mut state, 1);
    assert!(super::workbench_tests::click(
        &egui::Context::default(),
        &mut project,
        &mut state,
        "应用到工程（可撤销）"
    ));
    state = LocalizationUiState {
        source_locale: "en".into(),
        target_locale: "zh-Hant".into(),
        ..Default::default()
    };
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    assert_eq!(
        state.workbench.page.as_ref().unwrap().entries[0].status,
        worldline_core::localization::LocalizationStatus::Translated
    );
    assert!(state.status.is_none());
    assert!(state.workbench.drafts.is_empty());
    let ctx = egui::Context::default();
    click_label(&ctx, &mut project, &mut state, "Original translation");
    let key = editing::draft_key("zh-Hant", "line0");
    let id = egui::Id::new(("localization-part-text", &project.root, &key, 0usize));
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    ctrl_a(&ctx, &mut project, &mut state);
    let mut expected = String::new();
    for (index, character) in "HELLO中文😀".chars().enumerate() {
        if index == 2 {
            state.jobs.notice = Some("异步查询状态已变化".into());
        }
        if index == 3 {
            state.workbench.invalidate();
            catalog::refresh(&project, &mut state, 1);
            assert!(state.jobs.catalog_pending());
            assert!(catalog::can_edit_retained_page(&project, &state, 1));
        }
        if index == 4 {
            state.status = Some(Ok("旧操作提示".into()));
        }
        expected.push(character);
        draw(
            &ctx,
            &mut project,
            &mut state,
            vec![Event::Text(character.to_string())],
        );
        if index == 3 {
            jobs::settle(&project, &mut state, 1);
        }
        draw(&ctx, &mut project, &mut state, vec![]);
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(id),
            "after {expected}"
        );
        assert_eq!(
            state.workbench.drafts[&key].edit.translation_parts,
            [LocalizationPart::Text {
                text: expected.clone()
            }]
        );
    }
    assert!(state.has_unsubmitted_work());
    click_label(&ctx, &mut project, &mut state, "en");
    let locale_id = egui::Id::new(("localization-source-locale", &project.root, false));
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(locale_id));
    draw(&ctx, &mut project, &mut state, vec![]);
    assert_eq!(
        ctx.memory(|memory| memory.focused()),
        Some(locale_id),
        "never reclaim a deliberate focus transfer"
    );
}

#[test]
fn localization_pending_refresh_reuses_only_the_same_accepted_workspace_version_and_locale() {
    let (project, mut state) = fixture(1);
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    stage(&mut state, "保留原 revision 输入");
    let key = editing::draft_key("zh-Hant", "line0");
    let revision = state.workbench.drafts[&key].edit.source_revision.clone();
    state.workbench.invalidate();
    catalog::refresh(&project, &mut state, 1);
    assert!(state.jobs.catalog_pending());
    assert!(catalog::can_edit_retained_page(&project, &state, 1));
    state.target_locale = "fr".into();
    assert!(!catalog::can_edit_retained_page(&project, &state, 1));
    state.target_locale = "zh-Hant".into();
    assert!(!catalog::can_edit_retained_page(&project, &state, 2));
    let (other, _) = fixture(1);
    assert!(!catalog::can_edit_retained_page(&other, &state, 1));
    assert_eq!(state.workbench.drafts[&key].edit.source_revision, revision);
    assert!(state.has_unsubmitted_work());
    state.jobs.cancel();
}

#[test]
fn localization_query_typing_keeps_focus_across_real_async_results_and_status_headers() {
    let (mut project, mut state) = fixture(3);
    catalog::refresh(&project, &mut state, 1);
    jobs::settle(&project, &mut state, 1);
    let ctx = egui::Context::default();
    click_label(&ctx, &mut project, &mut state, "搜索源文、译文或 ID");
    let id = egui::Id::new(("localization-search", &project.root));
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    let mut expected = String::new();
    for character in "源文 2😀".chars() {
        expected.push(character);
        draw(
            &ctx,
            &mut project,
            &mut state,
            vec![Event::Text(character.to_string())],
        );
        draw(&ctx, &mut project, &mut state, vec![]);
        jobs::settle(&project, &mut state, 1);
        draw(&ctx, &mut project, &mut state, vec![]);
        assert_eq!(
            ctx.memory(|memory| memory.focused()),
            Some(id),
            "after {expected}"
        );
        assert_eq!(state.workbench.search, expected);
    }
    assert_eq!(state.workbench.page.as_ref().unwrap().total, 0);
    assert!(!state.has_unsubmitted_work());
}
