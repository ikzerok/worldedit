//! Real locale Story output and egui controls; this does not replace native-window acceptance.
use super::*;
use std::path::Path;

const ROOT: &str = concat!(
    "let n = 0\ncharacter speaker as \"林舟\"\nevent start\n",
    "  Root🌙 {rnd(1, 1000)} #wl-localization:root_body\n",
    "  say speaker \"Root spoken\" #wl-localization:root_spoken\n",
    "include \"chapters/body.wl\"\n",
    "  choice \"Root go {rnd(1, 1000)}\" #wl-localization:root_go\n",
    "    set n = 1\n    -> END\n",
);
const INCLUDED: &str = concat!(
    "\n\n\n\n\n\n\n\n\n\n",
    "  Included🌦️ {rnd(1, 1000)} #wl-localization:body\n",
    "  say speaker \"Included spoken\" #wl-localization:spoken\n",
    "  choice \"Included go {rnd(1, 1000)}\" #wl-localization:go\n",
    "    set n = 2\n    -> END\n",
);

pub(super) fn included_fixture(crlf: bool) -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = fixture();
    ctx.style_mut(|style| style.animation_time = 0.0);
    // Explicit 1.11 is required for actual Say statements; 1.10 preserves them as text.
    let manifest = app.project.root.join(".world/project.json");
    let mut config: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&manifest).unwrap().bytes()).unwrap();
    config["language_version"] = "1.11".into();
    app.project
        .set_authoring_document(&manifest, serde_json::to_vec(&config).unwrap())
        .unwrap();
    assert_eq!(app.project.language_version(), "1.11");
    let child = app.project.add_file(Path::new("chapters/body.wl")).unwrap();
    for (path, text) in [(app.project.entry.clone(), ROOT), (child, INCLUDED)] {
        app.project
            .set_text(
                &path,
                if crlf {
                    text.replace('\n', "\r\n")
                } else {
                    text.into()
                },
            )
            .unwrap();
    }
    let selection = LocalizationSelection {
        schema_version: 1,
        source_locale: "en".into(),
        target_locale: "zh-Hant".into(),
        string_ids: [
            "root_body",
            "root_spoken",
            "root_go",
            "body",
            "spoken",
            "go",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    };
    let mut exchange = app
        .project
        .preview_localization_export(&selection)
        .unwrap()
        .exchange;
    for entry in &mut exchange.entries {
        let mut parts = vec![LocalizationPart::Text {
            text: "译文 · ".into(),
        }];
        parts.extend(entry.source_parts.clone());
        entry.translation_parts = Some(parts);
    }
    let plan = app
        .project
        .preview_localization_import_candidate(&selection, &exchange)
        .unwrap();
    assert!(plan.can_apply, "{:?}", plan.diagnostics);
    app.project
        .apply_localization_import_candidate(&selection, &exchange, &plan.plan_digest)
        .unwrap();
    app.project.save().unwrap();
    app.reset_views();
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.snapshot.as_ref().unwrap().result.diagnostics
    );
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.program.events[0]
            .body
            .iter()
            .filter(|statement| matches!(statement, worldline_core::ast::Stmt::Say(_)))
            .count(),
        2,
        "root and included speech must be real Say AST nodes"
    );
    app.active_file = app.project.entry.clone();
    app.tab = Tab::Play;
    app.start_play();
    frame(&ctx, &mut app);
    assert!(app.play.as_ref().unwrap().error.is_none());
    assert_eq!(app.play.as_ref().unwrap().localized_outputs.len(), 4);
    assert_eq!(observed(&app).len(), 6);
    for id in ["root_spoken", "spoken"] {
        let output = app
            .play
            .as_ref()
            .unwrap()
            .localized_outputs
            .iter()
            .find(|output| output.localization.as_ref().unwrap().id.as_deref() == Some(id))
            .unwrap();
        assert_eq!(output.speaker.as_deref(), Some("林舟"));
        assert_eq!(output.localization.as_ref().unwrap().source.kind, "say");
    }
    (ctx, app)
}

pub(super) fn observed(app: &WorldeditApp) -> Vec<LocalizedPresentation> {
    let play = app.play.as_ref().unwrap();
    play.localized_outputs
        .iter()
        .filter_map(|output| output.localization.clone())
        .chain(
            play.story
                .as_ref()
                .unwrap()
                .choice_presentations()
                .iter()
                .filter_map(|choice| choice.localization.clone()),
        )
        .collect()
}

pub(super) fn unchanged_state(app: &WorldeditApp) -> serde_json::Value {
    let play = app.play.as_ref().unwrap();
    let story = play.story.as_ref().unwrap();
    serde_json::json!({
        "save": story.save().unwrap(),
        "trace": story.replay_trace(),
        "locale": story.presentation_identity(),
        "transcript": play.transcript,
        "outputs": play.localized_outputs,
        "version": app.version,
        "baseline": app.project.content_baseline(),
        "dirty": app.project.is_dirty(),
        "undo": app.history.len(),
        "redo": app.redo.len(),
    })
}

pub(super) fn navigation_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 1200.0),
            )),
            modifiers: if events
                .iter()
                .any(|event| matches!(event, egui::Event::Key { modifiers, .. } if modifiers.alt))
            {
                egui::Modifiers::ALT
            } else {
                egui::Modifiers::NONE
            },
            events,
            ..Default::default()
        },
        |ctx| {
            app.author_shortcuts(ctx);
            match app.tab {
                Tab::Edit => app.source_tab(ctx),
                Tab::Manuscript => app.manuscript_tab(ctx),
                _ => app.play_tab(ctx),
            }
            app.draft_rehearsal_dialog(ctx);
        },
    )
}

pub(super) fn click_label(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    navigation_frame(ctx, app, vec![]);
    let output = navigation_frame(ctx, app, vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let rect = egui::Rect::from_min_size(text.pos, text.galley.size());
                shape
                    .clip_rect
                    .contains(rect.center())
                    .then_some(rect.center())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing visible {label}: {}", rendered(&output)));
    navigation_frame(
        ctx,
        app,
        vec![
            egui::Event::PointerMoved(point),
            egui::Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    navigation_frame(
        ctx,
        app,
        vec![egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
}

pub(super) fn back(ctx: &egui::Context, app: &mut WorldeditApp) {
    navigation_frame(
        ctx,
        app,
        vec![egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: Some(egui::Key::ArrowLeft),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::ALT,
        }],
    );
    navigation_frame(
        ctx,
        app,
        vec![egui::Event::Key {
            key: egui::Key::ArrowLeft,
            physical_key: Some(egui::Key::ArrowLeft),
            pressed: false,
            repeat: false,
            modifiers: egui::Modifiers::ALT,
        }],
    );
    assert_eq!(app.tab, Tab::Play);
}

pub(super) fn assert_source_selection(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    metadata: &LocalizedPresentation,
) {
    assert_eq!(app.tab, Tab::Edit, "{:?}", app.message);
    let path = app.project.root.join(&metadata.source.file);
    assert_eq!(app.active_file, path);
    assert!(
        app.jump.is_none(),
        "the full range must own source selection"
    );
    for _ in 0..3 {
        navigation_frame(ctx, app, vec![]);
    }
    let source = app.project.document(&path).unwrap();
    let expected = source
        .lines()
        .nth(metadata.source.line as usize - 1)
        .unwrap()
        .trim();
    assert!(expected.contains(metadata.id.as_deref().unwrap()));
    let range = egui::TextEdit::load_state(ctx, egui::Id::new(("source", &path)))
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    let low = range.primary.index.min(range.secondary.index);
    let high = range.primary.index.max(range.secondary.index);
    let selected = source
        .chars()
        .skip(low)
        .take(high - low)
        .collect::<String>();
    assert_eq!(selected, expected);
    let start = source.find(expected).unwrap();
    assert_eq!(low, source[..start].chars().count());
    assert_eq!(high, source[..start + expected.len()].chars().count());
    assert!(app.message.as_deref().unwrap().contains("Alt+Left"));
}

#[test]
fn editor_locale_source_handler_resolves_root_and_include_text_say_choice_with_exact_return() {
    for crlf in [false, true] {
        let (ctx, mut app) = included_fixture(crlf);
        let actual = observed(&app);
        let before = unchanged_state(&app);
        let files = app.project.snapshot_files().unwrap();
        let history = app.personal.history.len();
        for metadata in actual {
            let expected = match metadata.id.as_deref().unwrap() {
                "root_body" => ("world.wl", 4, "text"),
                "root_spoken" => ("world.wl", 5, "say"),
                "root_go" => ("world.wl", 7, "choice"),
                "body" => ("chapters/body.wl", 11, "text"),
                "spoken" => ("chapters/body.wl", 12, "say"),
                "go" => ("chapters/body.wl", 13, "choice"),
                id => panic!("unexpected output {id}"),
            };
            assert_eq!(
                (
                    metadata.source.file.as_str(),
                    metadata.source.line,
                    metadata.source.kind.as_str()
                ),
                expected
            );
            app.open_localized_item(
                &ctx,
                Navigation::Entry {
                    presentation: metadata.clone(),
                    translation: false,
                },
                "zh-Hant",
                false,
            );
            assert_eq!(app.personal.history.len(), history + 1);
            assert_source_selection(&ctx, &mut app, &metadata);
            back(&ctx, &mut app);
            assert_eq!(app.personal.history.len(), history);
            assert_eq!(unchanged_state(&app), before);
            assert_eq!(app.project.snapshot_files().unwrap(), files);
        }
        std::fs::remove_dir_all(&app.project.root).unwrap();
    }
}

#[test]
fn editor_locale_source_egui_button_repeated_click_and_alt_left_preserve_actual_session() {
    let (ctx, mut app) = included_fixture(false);
    let metadata = app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .choice_presentations()[0]
        .localization
        .clone()
        .unwrap();
    let before = unchanged_state(&app);
    let history = app.personal.history.len();
    // The selector configures a future run, never the locale of the current source action.
    app.replay_debugger.locale.enabled = false;
    app.replay_debugger.locale.locale = "fr".into();
    for _ in 0..2 {
        click_label(&ctx, &mut app, "查看此源文");
        assert_source_selection(&ctx, &mut app, &metadata);
        back(&ctx, &mut app);
        assert_eq!(app.personal.history.len(), history);
        assert_eq!(unchanged_state(&app), before);
    }
    std::fs::remove_dir_all(&app.project.root).unwrap();
}
